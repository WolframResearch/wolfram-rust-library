//! Test stream methods for `RustLink/Tests/Streams.wlt`.
//!
//! Each method exercises one part of the [`wolfram_library_link::stream`]
//! contract. Several of them record what they saw in a static, which an
//! exported function then hands back to the Wolfram Language, so the test can
//! assert on what the Rust side actually observed.

use std::{
    io::Cursor,
    sync::{Mutex, MutexGuard},
};

use once_cell::sync::Lazy;

use wolfram_library_link::{
    self as wll,
    stream::{
        register_input_stream_method, register_output_stream_method, InputStream,
        InputStream as DeriveInputStream, InputStreamMethod, OpenRequest, OutputMode,
        OutputStream, OutputStream as DeriveOutputStream, OutputStreamMethod,
        ReaderInputStream, SeekableInputStream as DeriveSeekableInputStream, StreamError,
        StreamUnitSize, WriterOutputStream,
    },
};

//======================================
// Shared observations
//======================================

/// What the most recently opened stream observed, for the Wolfram Language to
/// assert on.
struct Observed {
    /// The options expression read off the most recent `OpenRequest`.
    options: Option<String>,
    /// The `OpenRequest` fields seen by the most recent open.
    open_request: Option<String>,
    /// Bytes written to the most recent `"TestCollect"` output stream.
    written: Vec<u8>,
    /// How many times a `"TestBlocking"` stream was asked to wait.
    waits: i64,
    /// The mode the most recent output stream was opened in.
    output_mode: Option<String>,
}

static OBSERVED: Lazy<Mutex<Observed>> = Lazy::new(|| {
    Mutex::new(Observed {
        options: None,
        open_request: None,
        written: Vec::new(),
        waits: 0,
        output_mode: None,
    })
});

fn observed() -> MutexGuard<'static, Observed> {
    OBSERVED.lock().expect("OBSERVED mutex was poisoned")
}

//======================================
// Registration
//======================================

/// Register every test stream method. Called from this library's
/// `#[init]` function in `main.rs`.
pub fn register_test_stream_methods() {
    register_input_stream_method("TestFixed", FixedMethod);

    register_input_stream_method("TestOptions", OptionsMethod);

    register_input_stream_method("TestError", ErrorMethod);

    register_input_stream_method("TestBlocking", BlockingMethod);

    register_input_stream_method("TestSeekable", SeekableMethod);

    register_input_stream_method("TestOpenFail", OpenFailMethod);

    register_input_stream_method("TestPanic", PanicMethod);

    register_input_stream_method("TestProto", ProtoMethod);

    register_output_stream_method("TestCollect", CollectMethod);

    register_output_stream_method("TestShortWrite", ShortWriteMethod);

    register_input_stream_method("TestDerivedRead", DerivedReadMethod);

    register_input_stream_method("TestDerivedSeek", DerivedSeekMethod);

    register_output_stream_method("TestDerivedWrite", DerivedWriteMethod);
}

//======================================
// Exported observation functions
//======================================

/// The options expression the most recent `"TestOptions"` stream saw, in
/// `InputForm`, or `""` if none has been opened.
#[wll::export]
fn test_stream_last_options() -> String {
    observed().options.clone().unwrap_or_default()
}

/// The `OpenRequest` fields the most recent stream saw.
#[wll::export]
fn test_stream_last_open_request() -> String {
    observed().open_request.clone().unwrap_or_default()
}

/// Take everything written to `"TestCollect"` streams so far.
#[wll::export]
fn test_stream_take_written() -> String {
    let mut observed = observed();
    let written = std::mem::take(&mut observed.written);
    String::from_utf8_lossy(&written).into_owned()
}

/// How many times a `"TestBlocking"` stream was asked to wait for input.
#[wll::export]
fn test_stream_wait_count() -> i64 {
    observed().waits
}

/// The mode (`"Truncate"` or `"Append"`) the most recent output stream was
/// opened in.
#[wll::export]
fn test_stream_output_mode() -> String {
    observed().output_mode.clone().unwrap_or_default()
}

/// Reset every recorded observation, so one test cannot see another's leavings.
#[wll::export]
fn test_stream_reset() -> bool {
    let mut observed = observed();
    observed.options = None;
    observed.open_request = None;
    observed.written.clear();
    observed.waits = 0;
    observed.output_mode = None;
    true
}

/// Register a method under a name that is already taken, to check that the
/// failure is reported rather than silently ignored.
///
/// Registration panics on a duplicate name, so catch the unwind rather than
/// letting it escape into the Wolfram Language.
#[wll::export]
fn test_stream_duplicate_registration_panics() -> bool {
    std::panic::catch_unwind(|| register_input_stream_method("TestFixed", FixedMethod))
        .is_err()
}

//======================================
// Helpers
//======================================

/// Record the `OpenRequest` fields and the options expression.
///
/// Reading the options link is the only way to check that
/// [`StreamOptions`][wll::stream::StreamOptions] is wired up correctly, since
/// the Wolfram Language never shows the library's view of it otherwise.
fn record_request(request: &mut OpenRequest) {
    let summary = format!(
        "name: {:?}, user_supplied_name: {:?}, expanded_name: {:?}, \
         is_file_path: {}, message_head: {:?}",
        request.name(),
        request.user_supplied_name(),
        request.expanded_name(),
        request.is_file_path(),
        request.message_head(),
    );

    let options = {
        let mut options = request.options();

        if options.is_null() {
            String::from("<no options link>")
        } else {
            match options.get_expr() {
                Ok(expr) => expr.to_string(),
                Err(err) => format!("<error reading options: {err}>"),
            }
        }
    };

    let mut observed = observed();
    observed.open_request = Some(summary);
    observed.options = Some(options);
}

//======================================
// Input methods
//======================================

const FIXED_CONTENTS: &str = "hello from Rust";

/// Serves a fixed string. The simplest possible input method.
struct FixedMethod;

impl InputStreamMethod for FixedMethod {
    type Stream = ReaderInputStream;

    fn open(&self, request: &mut OpenRequest) -> Result<Self::Stream, StreamError> {
        record_request(request);

        Ok(
            ReaderInputStream::new(Cursor::new(FIXED_CONTENTS.as_bytes()))
                .with_size(FIXED_CONTENTS.len() as i64),
        )
    }
}

/// Serves back the options expression it was opened with, so the Wolfram
/// Language can read the library's view of its own options.
struct OptionsMethod;

impl InputStreamMethod for OptionsMethod {
    type Stream = ReaderInputStream;

    fn open(&self, request: &mut OpenRequest) -> Result<Self::Stream, StreamError> {
        record_request(request);

        let options = observed().options.clone().unwrap_or_default();

        Ok(ReaderInputStream::new(Cursor::new(options.into_bytes())))
    }
}

/// Fails on the first read, to check how a `StreamError` reaches the user.
struct ErrorMethod;

pub const READ_ERROR_MESSAGE: &str = "The test stream failed on purpose.";

impl InputStreamMethod for ErrorMethod {
    type Stream = ErrorStream;

    fn open(&self, request: &mut OpenRequest) -> Result<Self::Stream, StreamError> {
        record_request(request);
        Ok(ErrorStream)
    }
}

struct ErrorStream;

impl InputStream for ErrorStream {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, StreamError> {
        Err(StreamError::new(READ_ERROR_MESSAGE))
    }
}

/// Reports `WouldBlock` twice before yielding data, to check that the kernel
/// waits and retries rather than treating the empty read as end of stream.
struct BlockingMethod;

impl InputStreamMethod for BlockingMethod {
    type Stream = BlockingStream;

    fn open(&self, request: &mut OpenRequest) -> Result<Self::Stream, StreamError> {
        record_request(request);
        observed().waits = 0;

        Ok(BlockingStream {
            blocks_left: 2,
            sent: false,
        })
    }
}

struct BlockingStream {
    blocks_left: u32,
    sent: bool,
}

impl InputStream for BlockingStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, StreamError> {
        if self.blocks_left > 0 {
            self.blocks_left -= 1;
            return Err(StreamError::WouldBlock);
        }

        if self.sent {
            return Ok(0);
        }

        let data = b"unblocked";
        let len = std::cmp::min(buf.len(), data.len());
        buf[..len].copy_from_slice(&data[..len]);
        self.sent = true;

        Ok(len)
    }

    fn wait_for_input(&mut self) {
        observed().waits += 1;
    }
}

/// A seekable stream, to check `SeekableQ` / `Mfseek` / `Mstreamsize`.
struct SeekableMethod;

/// Large enough that the Wolfram Language cannot simply buffer the whole stream
/// and reposition within its own buffer -- it reads in 64 KiB chunks, and only
/// calls `Mfseek` when the target falls outside what it has buffered.
pub const SEEKABLE_LEN: usize = 300_000;

impl InputStreamMethod for SeekableMethod {
    type Stream = ReaderInputStream;

    fn open(&self, request: &mut OpenRequest) -> Result<Self::Stream, StreamError> {
        record_request(request);

        // Byte `i` is `i mod 251`, so the test can assert on content at any
        // position without shipping the data.
        let contents: Vec<u8> = (0..SEEKABLE_LEN).map(|i| (i % 251) as u8).collect();

        Ok(ReaderInputStream::seekable(Cursor::new(contents)))
    }
}

/// Fails to open at all.
struct OpenFailMethod;

pub const OPEN_ERROR_MESSAGE: &str = "The test stream refused to open.";

impl InputStreamMethod for OpenFailMethod {
    type Stream = ReaderInputStream;

    fn open(&self, _request: &mut OpenRequest) -> Result<Self::Stream, StreamError> {
        Err(StreamError::new(OPEN_ERROR_MESSAGE))
    }
}

/// Panics on read, to check that a panic is contained and surfaced as an error
/// rather than unwinding into the kernel.
struct PanicMethod;

impl InputStreamMethod for PanicMethod {
    type Stream = PanicStream;

    fn open(&self, _request: &mut OpenRequest) -> Result<Self::Stream, StreamError> {
        Ok(PanicStream)
    }
}

struct PanicStream;

impl InputStream for PanicStream {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, StreamError> {
        panic!("The test stream panicked on purpose.")
    }
}

/// Claims names by pattern, rather than requiring an explicit `Method` option.
struct ProtoMethod;

impl InputStreamMethod for ProtoMethod {
    type Stream = ReaderInputStream;

    const NAME_DISPATCH: bool = true;

    fn open(&self, request: &mut OpenRequest) -> Result<Self::Stream, StreamError> {
        record_request(request);

        Ok(ReaderInputStream::new(Cursor::new(
            b"claimed by name".to_vec(),
        )))
    }

    fn handles_name(&self, name: &str) -> bool {
        name.starts_with("teststream://")
    }
}

//======================================
// Output methods
//======================================

/// Collects everything written to it, for `test_stream_take_written()`.
struct CollectMethod;

impl OutputStreamMethod for CollectMethod {
    type Stream = CollectStream;

    fn open(
        &self,
        request: &mut OpenRequest,
        mode: OutputMode,
    ) -> Result<Self::Stream, StreamError> {
        record_request(request);

        observed().output_mode = Some(
            match mode {
                OutputMode::Truncate => "Truncate",
                OutputMode::Append => "Append",
            }
            .to_owned(),
        );

        Ok(CollectStream)
    }
}

struct CollectStream;

impl OutputStream for CollectStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, StreamError> {
        observed().written.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn unit_size(&self) -> StreamUnitSize {
        StreamUnitSize::Bytes8
    }
}

/// Accepts only one byte per call, to check that a short write is retried
/// rather than silently dropping the remainder.
struct ShortWriteMethod;

impl OutputStreamMethod for ShortWriteMethod {
    type Stream = WriterOutputStream;

    fn open(
        &self,
        request: &mut OpenRequest,
        _mode: OutputMode,
    ) -> Result<Self::Stream, StreamError> {
        record_request(request);
        Ok(WriterOutputStream::new(OneByteAtATime))
    }
}

struct OneByteAtATime;

impl std::io::Write for OneByteAtATime {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        observed().written.push(buf[0]);
        Ok(1)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

//======================================
// Derived streams
//======================================

//
// The `#[derive(..)]` counterparts to `ReaderInputStream` /
// `WriterOutputStream`, for types a library owns and can therefore derive on.
//

/// A reader whose `InputStream` impl comes from `#[derive(InputStream)]`.
#[derive(DeriveInputStream)]
struct DerivedReader(Cursor<Vec<u8>>);

impl std::io::Read for DerivedReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

struct DerivedReadMethod;

impl InputStreamMethod for DerivedReadMethod {
    type Stream = DerivedReader;

    fn open(&self, request: &mut OpenRequest) -> Result<Self::Stream, StreamError> {
        record_request(request);
        Ok(DerivedReader(Cursor::new(b"derived read".to_vec())))
    }
}

/// A seekable reader whose impls come from `#[derive(SeekableInputStream)]`.
#[derive(DeriveSeekableInputStream)]
struct DerivedSeeker(Cursor<Vec<u8>>);

impl std::io::Read for DerivedSeeker {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl std::io::Seek for DerivedSeeker {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

struct DerivedSeekMethod;

impl InputStreamMethod for DerivedSeekMethod {
    type Stream = DerivedSeeker;

    fn open(&self, request: &mut OpenRequest) -> Result<Self::Stream, StreamError> {
        record_request(request);

        let contents: Vec<u8> = (0..SEEKABLE_LEN).map(|i| (i % 251) as u8).collect();
        Ok(DerivedSeeker(Cursor::new(contents)))
    }
}

/// A writer whose `OutputStream` impl comes from `#[derive(OutputStream)]`.
#[derive(DeriveOutputStream)]
struct DerivedWriter;

impl std::io::Write for DerivedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        observed().written.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct DerivedWriteMethod;

impl OutputStreamMethod for DerivedWriteMethod {
    type Stream = DerivedWriter;

    fn open(
        &self,
        request: &mut OpenRequest,
        _mode: OutputMode,
    ) -> Result<Self::Stream, StreamError> {
        record_request(request);
        Ok(DerivedWriter)
    }
}
