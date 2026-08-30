//! TCP streams example — open a TCP connection from the Wolfram Language and
//! read and write it with ordinary stream functions.
//!
//! ```wolfram
//! id = tcp::open_connection["example.com", 80];
//!
//! out = OpenWrite[ToString[id], Method -> "TCP"];
//! WriteString[out, "GET / HTTP/1.0\r\nHost: example.com\r\n\r\n"];
//! Close[out];
//!
//! in = OpenRead[ToString[id], Method -> "TCP"];
//! response = ReadString[in];
//! Close[in];
//!
//! tcp::close_connection[id];
//! ```
//!
//! The connection outlives the streams: `connect` returns an id, and each
//! `OpenRead` / `OpenWrite` makes a stream over that same connection, so a
//! request can be written and a response read without reconnecting. `close`
//! ends the connection itself.
//!
//! Reads are non-blocking. The socket has a short read timeout, so a read with
//! nothing waiting reports [`StreamError::WouldBlock`] rather than blocking; the
//! Wolfram Language then calls [`wait_for_input`][InputStream::wait_for_input]
//! and reads again. That is what keeps a `ReadString` on a quiet socket
//! interruptible — see [`TcpInput::wait_for_input`].

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpStream,
    sync::{Mutex, OnceLock},
    time::Duration,
};

use wolfram_library_link::{
    self as wll,
    stream::{
        register_input_stream_method, register_output_stream_method, InputStream,
        InputStreamMethod, OpenRequest, OutputMode, OutputStream, OutputStreamMethod,
        StreamError,
    },
};

/// How long a read waits for data before reporting `WouldBlock`.
///
/// This bounds how long the Wolfram Language can be stuck inside a single read,
/// and so how quickly an abort takes effect.
const READ_TIMEOUT: Duration = Duration::from_millis(50);

//======================================
// Open connections
//======================================

/// The open connections, keyed by the id handed to the Wolfram Language.
///
/// A connection lives here rather than in the stream, so that the same
/// connection can back both an input and an output stream, and outlive both.
fn connections() -> &'static Mutex<HashMap<i64, TcpStream>> {
    static CONNECTIONS: OnceLock<Mutex<HashMap<i64, TcpStream>>> = OnceLock::new();
    CONNECTIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Clone the connection with the given id, for one stream to use.
///
/// [`TcpStream::try_clone`] shares the underlying socket, so an input stream and
/// an output stream over the same id read and write the same connection.
fn clone_connection(id: i64) -> Result<TcpStream, StreamError> {
    let connections = connections()
        .lock()
        .expect("CONNECTIONS mutex was poisoned");

    let Some(connection) = connections.get(&id) else {
        return Err(StreamError::new(format!(
            "There is no open TCP connection with id {id}."
        )));
    };

    Ok(connection.try_clone()?)
}

/// Parse the id out of a stream name, which is the id rendered as a string.
fn id_from_name(request: &OpenRequest) -> Result<i64, StreamError> {
    request.name().trim().parse::<i64>().map_err(|_| {
        StreamError::new(format!(
            "The stream name {:?} is not a TCP connection id.",
            request.name()
        ))
    })
}

//======================================
// Exported functions
//======================================

/// Open a TCP connection, returning the id to open streams with.
///
/// # Naming
///
/// Every `#[export]`ed function becomes a global C symbol in the process, so a
/// function named `connect` or `close` would interpose the C library's own
/// `connect()` / `close()` for *everything* in the Wolfram kernel -- including
/// the `std::net` calls just below. Keep exported names distinct from C library
/// functions.
#[wll::export]
fn open_connection(host: String, port: i64) -> i64 {
    let port = u16::try_from(port).unwrap_or_else(|_| panic!("invalid port: {port}"));

    let connection = TcpStream::connect((host.as_str(), port))
        .unwrap_or_else(|err| panic!("unable to connect to {host}:{port}: {err}"));

    connection
        .set_read_timeout(Some(READ_TIMEOUT))
        .expect("unable to set the connection read timeout");

    let mut connections = connections()
        .lock()
        .expect("CONNECTIONS mutex was poisoned");

    // Ids are handed out sequentially, and never reused within a session.
    let id = connections.keys().copied().max().unwrap_or(0) + 1;
    connections.insert(id, connection);

    id
}

/// Close a TCP connection. Streams already open over it stop working.
#[wll::export]
fn close_connection(id: i64) -> bool {
    let mut connections = connections()
        .lock()
        .expect("CONNECTIONS mutex was poisoned");

    match connections.remove(&id) {
        // Shut both directions down; dropping the `TcpStream` closes it.
        Some(connection) => {
            let _ = connection.shutdown(std::net::Shutdown::Both);
            true
        },
        None => false,
    }
}

/// Whether a connection with this id is still open.
#[wll::export]
fn connection_is_open(id: i64) -> bool {
    connections()
        .lock()
        .expect("CONNECTIONS mutex was poisoned")
        .contains_key(&id)
}

//======================================
// Registration
//======================================

#[wll::init]
fn init() {
    register_input_stream_method("TCP", TcpMethod);
    register_output_stream_method("TCP", TcpMethod);
}

/// Backs both `OpenRead[…, Method -> "TCP"]` and
/// `OpenWrite[…, Method -> "TCP"]`. Input and output stream methods are
/// registered separately, so one type can serve both.
struct TcpMethod;

impl InputStreamMethod for TcpMethod {
    type Stream = TcpInput;

    fn open(&self, request: &mut OpenRequest) -> Result<Self::Stream, StreamError> {
        Ok(TcpInput {
            connection: clone_connection(id_from_name(request)?)?,
        })
    }
}

impl OutputStreamMethod for TcpMethod {
    type Stream = TcpOutput;

    fn open(
        &self,
        request: &mut OpenRequest,
        _mode: OutputMode,
    ) -> Result<Self::Stream, StreamError> {
        // A socket has nothing to truncate or append to, so the mode is ignored
        // and `OpenWrite` and `OpenAppend` behave identically.
        Ok(TcpOutput {
            connection: clone_connection(id_from_name(request)?)?,
        })
    }
}

//======================================
// The streams
//======================================

/// Reads from a TCP connection.
///
/// This implements [`InputStream`] by hand rather than wrapping the connection
/// in a `ReaderInputStream`, because a socket read timeout has to be recognized
/// before it reaches [`StreamError`]. See [`TcpInput::read`].
pub struct TcpInput {
    connection: TcpStream,
}

impl InputStream for TcpInput {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, StreamError> {
        match self.connection.read(buf) {
            Ok(read) => Ok(read),
            // A read that hits the timeout set in `open_connection` means the
            // peer has not sent anything yet. That is not an error: reporting
            // `WouldBlock` is what makes the Wolfram Language wait and read
            // again, rather than failing the stream.
            //
            // Which error kind a timed-out read produces is platform-specific:
            // Unix reports `WouldBlock`, but Windows may report `TimedOut`
            // (see `TcpStream::set_read_timeout`). Both must be matched.
            //
            // This is why the connection is not simply wrapped in a
            // `ReaderInputStream`: its `From<std::io::Error>` maps `WouldBlock`
            // to `StreamError::WouldBlock` but treats `TimedOut` as a genuine
            // failure -- correct in general, since a timed-out read can also
            // mean the connection has died, but wrong for this stream, and
            // wrong only on Windows.
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                Err(StreamError::WouldBlock)
            },
            Err(err) => Err(StreamError::new(format!(
                "Unable to read from the TCP connection: {err}."
            ))),
        }
    }

    /// Called after a read reports [`StreamError::WouldBlock`].
    ///
    /// There is nothing to do here: the socket's own read timeout already
    /// bounds how long the next read waits. Returning immediately is correct —
    /// the Wolfram Language simply reads again, and checks for an abort between
    /// tries.
    fn wait_for_input(&mut self) {}
}

/// Writes to a TCP connection.
pub struct TcpOutput {
    connection: TcpStream,
}

impl OutputStream for TcpOutput {
    fn write(&mut self, buf: &[u8]) -> Result<usize, StreamError> {
        self.connection.write(buf).map_err(|err| {
            StreamError::new(format!("Unable to write to the TCP connection: {err}."))
        })
    }

    fn flush(&mut self) -> Result<(), StreamError> {
        self.connection.flush().map_err(|err| {
            StreamError::new(format!("Unable to flush the TCP connection: {err}."))
        })
    }

    fn close(mut self) -> Result<(), StreamError> {
        // Closing the stream must not close the connection -- the id stays
        // usable for further streams until `tcp::close` is called. Flushing is
        // all that is needed; dropping this clone leaves the connection open.
        self.flush()
    }
}

//======================================
// Test support
//======================================

/// Start a background echo server on an unused local port, and return the port.
///
/// This exists so `tests/TcpStreams.wlt` has something to connect to; a real
/// client library would not ship it.
#[wll::export]
fn start_echo_server() -> i64 {
    use std::net::TcpListener;

    let listener =
        TcpListener::bind("127.0.0.1:0").expect("unable to bind the echo server");
    let port = i64::from(
        listener
            .local_addr()
            .expect("unable to read the echo server address")
            .port(),
    );

    std::thread::spawn(move || {
        for connection in listener.incoming() {
            let Ok(mut connection) = connection else {
                continue;
            };

            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                while let Ok(read) = connection.read(&mut buf) {
                    if read == 0 || connection.write_all(&buf[..read]).is_err() {
                        break;
                    }
                }
            });
        }
    });

    port
}
