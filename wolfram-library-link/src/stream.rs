//! Custom Wolfram Language I/O streams.
//!
//! A library can register named *stream methods* that the Wolfram Language then
//! opens streams with:
//!
//! ```wolfram
//! stream = OpenRead["my-name", Method -> "MyMethod"];
//! ReadString[stream]
//! ```
//!
//! Registering [`InputStreamMethod`] or [`OutputStreamMethod`] makes such a
//! method available. Each time the Wolfram Language opens a stream with that
//! method, the method's `open()` produces an [`InputStream`] or [`OutputStream`]
//! that services reads or writes for the life of that one stream.
//!
//! To adapt a type that already implements [`std::io::Read`] or
//! [`std::io::Write`], use [`ReaderInputStream`] / [`WriterOutputStream`].
//!
//! # Example
//!
//! ```no_run
//! use std::fs::File;
//! use wolfram_library_link::stream::{
//!     register_input_stream_method, InputStreamMethod, OpenRequest,
//!     ReaderInputStream, StreamError,
//! };
//!
//! struct FileMethod;
//!
//! impl InputStreamMethod for FileMethod {
//!     type Stream = ReaderInputStream;
//!
//!     fn open(&self, request: &mut OpenRequest) -> Result<Self::Stream, StreamError> {
//!         Ok(ReaderInputStream::seekable(File::open(request.name())?))
//!     }
//! }
//!
//! #[wolfram_library_link::init]
//! fn init() {
//!     register_input_stream_method("RustFile", FileMethod);
//! }
//! ```
//!
//! # Kernel behavior
//!
//! The C API this module wraps documents almost none of its runtime contract, so
//! the behavior below was determined empirically (Wolfram 15.0.0). It is what
//! shapes this module's design, and is worth knowing when writing a stream by
//! hand:
//!
//! * **Reads.** Returning `Ok(0)` from [`InputStream::read`] means end of
//!   stream. Returning [`StreamError::WouldBlock`] means "nothing yet" — the
//!   kernel then calls [`InputStream::wait_for_input`] and reads again. Any
//!   other `Err` is reported to the user.
//!
//! * **`wait_for_input` should not block indefinitely.** Waiting a short,
//!   bounded time and returning is correct; the kernel simply reads again.
//!
//! * **Input position is the kernel's.** The kernel buffers input in 64 KiB
//!   chunks and tracks the stream position itself, so
//!   [`InputStream::tell`] is not currently consulted. It calls
//!   [`InputStream::seek`] only when a requested position falls outside the
//!   buffered window — and passes *its own* absolute offset, not the one the
//!   user asked for.
//!
//! * **Output errors are barely reportable.** The kernel provides no error
//!   channel for output streams: a failed write produces a generic message from
//!   `BinaryWrite` and *nothing at all* from `WriteString` or `Write`. See
//!   [`OutputStream::write`]. This is a limitation of LibraryLink, not of this
//!   wrapper.
//!
//! * **Nothing retries a partial write.** This module therefore loops on
//!   [`OutputStream::write`] until the whole buffer is consumed, so a short
//!   write can never silently drop bytes.
//!
//! # Related links
//!
//! * [Streams] section of the LibraryLink documentation.
//! * [`OpenRead`][ref/OpenRead]<sub>WL</sub>, [`OpenWrite`][ref/OpenWrite]<sub>WL</sub>
//!   and [`OpenAppend`][ref/OpenAppend]<sub>WL</sub>, which open a stream with a
//!   registered method.
//!
//! [Streams]: https://reference.wolfram.com/language/LibraryLink/tutorial/InteractionWithWolframLanguage.html#509267359
//! [ref/OpenRead]: https://reference.wolfram.com/language/ref/OpenRead.html
//! [ref/OpenWrite]: https://reference.wolfram.com/language/ref/OpenWrite.html
//! [ref/OpenAppend]: https://reference.wolfram.com/language/ref/OpenAppend.html

use std::{
    ffi::{c_void, CStr, CString},
    io::{Read, Seek, SeekFrom, Write},
    marker::PhantomData,
    os::raw::{c_char, c_int},
    panic::{self, AssertUnwindSafe},
    ptr,
};

use crate::{
    rtl,
    sys::{self, mbool, mint, MInputStream, MOutputStream},
};

const TRUE: mbool = 1;
const FALSE: mbool = 0;

/// Stand-in for a NULL stream name.
const EMPTY_CSTR: &CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"\0") };

//======================================
// Errors
//======================================

/// An error raised by a stream operation.
///
/// [`From<std::io::Error>`][StreamError#impl-From<Error>] is implemented, so `?`
/// works directly on [`std::io`] operations inside a stream method.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum StreamError {
    /// The operation failed.
    ///
    /// The message is reported verbatim by the Wolfram Language, substituted
    /// into [`General::strmerr`][ref/message/strmerr] under the tag of whichever
    /// function was reading. A stream named `"my-stream"` failing a
    /// [`Read`][ref/Read]<sub>WL</sub> with `StreamError::new("The connection
    /// was reset.")` produces:
    ///
    /// ```text
    /// Read::strmerr: Error on stream my-stream. Error message: The connection was reset.
    /// ```
    ///
    /// The message is inserted verbatim and unquoted, so write it in sentence
    /// case, with a period after each sentence.
    ///
    /// [ref/Read]: https://reference.wolfram.com/language/ref/Read.html
    /// [ref/message/strmerr]: https://reference.wolfram.com/language/ref/message/General/strmerr.html
    Error(String),

    /// No data is available *yet*. This is not an error and not end of stream.
    ///
    /// Only meaningful for [`InputStream::read`]: the kernel responds by calling
    /// [`InputStream::wait_for_input`] and reading again. Returned from an
    /// output stream it is treated as an ordinary error, because the C API has
    /// no wait-for-output counterpart.
    WouldBlock,

    /// The stream does not implement this operation.
    Unsupported,
}

impl StreamError {
    /// Construct a [`StreamError::Error`] from anything string-like.
    pub fn new(message: impl Into<String>) -> Self {
        StreamError::Error(message.into())
    }

    /// The message the Wolfram Language should report, if any.
    fn message(&self) -> Option<&str> {
        match self {
            StreamError::Error(message) => Some(message.as_str()),
            StreamError::WouldBlock => None,
            StreamError::Unsupported => {
                Some("Operation is not supported by this stream.")
            },
        }
    }
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamError::Error(message) => f.write_str(message),
            StreamError::WouldBlock => f.write_str("No data is available yet."),
            StreamError::Unsupported => {
                f.write_str("Operation is not supported by this stream.")
            },
        }
    }
}

impl std::error::Error for StreamError {}

impl From<std::io::Error> for StreamError {
    fn from(err: std::io::Error) -> StreamError {
        match err.kind() {
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted => {
                StreamError::WouldBlock
            },
            std::io::ErrorKind::Unsupported => StreamError::Unsupported,
            _ => StreamError::Error(err.to_string()),
        }
    }
}

impl From<String> for StreamError {
    fn from(message: String) -> StreamError {
        StreamError::Error(message)
    }
}

impl From<&str> for StreamError {
    fn from(message: &str) -> StreamError {
        StreamError::Error(message.to_owned())
    }
}

//======================================
// Supporting types
//======================================

/// The size of the units a stream deals in.
///
/// *LibraryLink C type:* [`MStream_StreamUnitSize_t`][sys::MStream_StreamUnitSize_t].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamUnitSize {
    /// 8-bit units (`MSTREAM_8BIT`). This is the default.
    Bytes8,
    /// UTF-32 units (`MSTREAM_UTF32`).
    Utf32,
}

impl StreamUnitSize {
    fn to_raw(self) -> sys::MStream_StreamUnitSize_t {
        match self {
            StreamUnitSize::Bytes8 => sys::MStream_StreamUnitSize_t_MSTREAM_8BIT,
            StreamUnitSize::Utf32 => sys::MStream_StreamUnitSize_t_MSTREAM_UTF32,
        }
    }
}

/// Whether an output stream was opened by `OpenWrite` or `OpenAppend`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputMode {
    /// The stream was opened by [`OpenWrite`][ref/OpenWrite]<sub>WL</sub>.
    ///
    /// [ref/OpenWrite]: https://reference.wolfram.com/language/ref/OpenWrite.html
    Truncate,
    /// The stream was opened by [`OpenAppend`][ref/OpenAppend]<sub>WL</sub>.
    ///
    /// [ref/OpenAppend]: https://reference.wolfram.com/language/ref/OpenAppend.html
    Append,
}

/// The options associated with a stream, as a WSTP link.
///
/// The Wolfram Language passes stream options as an expression on a link, both
/// when opening a stream ([`OpenRequest::options`]) and when the options of an
/// open stream are queried or changed
/// ([`InputStream::set_options`] / [`OutputStream::set_options`]).
///
/// The kernel owns the link; it is not closed when this borrow ends.
pub struct StreamOptions<'a> {
    raw: sys::WSLINK,
    _marker: PhantomData<&'a mut ()>,
}

impl<'a> StreamOptions<'a> {
    fn new(raw: *mut c_void) -> Self {
        StreamOptions {
            raw: raw as sys::WSLINK,
            _marker: PhantomData,
        }
    }

    /// Whether the Wolfram Language provided a link at all.
    pub fn is_null(&self) -> bool {
        self.raw.is_null()
    }

    /// The raw link pointer (the C API's `optionsIn` / `optionsLink`).
    pub fn as_raw_link(&self) -> sys::WSLINK {
        self.raw
    }

    /// Read the options as an [`Expr`][crate::expr::Expr].
    ///
    /// This is the normal way to inspect a stream's options. The Wolfram
    /// Language places a single expression on the link — typically a list of
    /// option rules — which this reads off in one go.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use wolfram_library_link::stream::{OpenRequest, StreamError};
    /// # fn open(request: &mut OpenRequest) -> Result<(), StreamError> {
    /// let options = request.options().get_expr()?;
    /// println!("opened with options: {options}");
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "wstp")]
    pub fn get_expr(&mut self) -> Result<crate::expr::Expr, StreamError> {
        use crate::expr::Symbol;

        let Some(link) = self.link() else {
            return Err(StreamError::new(
                "The stream was opened without an options link.",
            ));
        };

        // The Wolfram Language writes bare symbol names like `List` to this
        // link, which `Link::get_expr()` alone rejects for having no context.
        // Resolve them the same way argument lists are resolved.
        link.get_expr_with_resolver(&mut |name| {
            Symbol::try_new(&format!("System`{name}"))
        })
        .map_err(|err| {
            StreamError::new(format!("Unable to read the stream options: {err}."))
        })
    }

    /// Borrow the options as a WSTP [`Link`][wstp::Link].
    ///
    /// This is the escape hatch for reading or writing the link directly;
    /// prefer [`get_expr()`][StreamOptions::get_expr].
    ///
    /// Returns [`None`] if the Wolfram Language provided no link.
    ///
    /// The kernel owns the link: it is not closed when the borrow ends, and the
    /// link must be left balanced — read whole expressions off it, and write
    /// whole expressions to it. Note that the Wolfram Language writes
    /// unqualified symbol names to this link, so
    /// [`Link::get_expr()`][wstp::Link::get_expr] will fail on them; resolve
    /// them into the `` System` `` context, as
    /// [`get_expr()`][StreamOptions::get_expr] does.
    #[cfg(feature = "wstp")]
    pub fn link(&mut self) -> Option<&mut wstp::Link> {
        if self.raw.is_null() {
            return None;
        }

        // `sys::WSLINK` and `wstp::sys::WSLINK` are the same thin pointer type
        // produced by two separate bindgen runs (the crate casts between them
        // with `as` elsewhere). Reborrow the field in place rather than casting
        // a temporary, so the returned `&mut Link` borrows from `self` and
        // cannot dangle.
        let slot: &mut wstp::sys::WSLINK = unsafe {
            &mut *(&mut self.raw as *mut sys::WSLINK as *mut wstp::sys::WSLINK)
        };

        // Safety: the kernel hands this link to exactly one callback at a time,
        // and `&mut self` makes that exclusivity visible to the borrow checker.
        Some(unsafe { wstp::Link::unchecked_ref_cast_mut(slot) })
    }
}

impl<'a> std::fmt::Debug for StreamOptions<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamOptions")
            .field("raw", &self.raw)
            .finish()
    }
}

/// The stream the Wolfram Language is asking a method to open.
///
/// Passed to [`InputStreamMethod::open`] and [`OutputStreamMethod::open`].
pub struct OpenRequest<'a> {
    name: &'a CStr,
    user_supplied_name: Option<&'a CStr>,
    expanded_name: Option<&'a CStr>,
    is_file_path: bool,
    message_head: Option<&'a CStr>,
    options: *mut c_void,
    _marker: PhantomData<&'a mut ()>,
}

impl<'a> OpenRequest<'a> {
    /// The name of the stream to open, as the Wolfram Language resolved it.
    ///
    /// Returns `""` if the name is not valid UTF-8; use [`name_cstr`] to
    /// inspect such a name.
    ///
    /// [`name_cstr`]: OpenRequest::name_cstr
    pub fn name(&self) -> &'a str {
        self.name.to_str().unwrap_or("")
    }

    /// The name of the stream, without assuming it is UTF-8.
    pub fn name_cstr(&self) -> &'a CStr {
        self.name
    }

    /// The name as the user supplied it, before path resolution.
    pub fn user_supplied_name(&self) -> Option<&'a str> {
        self.user_supplied_name.and_then(|s| s.to_str().ok())
    }

    /// The absolute file path, when [`is_file_path()`][OpenRequest::is_file_path]
    /// is `true`.
    pub fn expanded_name(&self) -> Option<&'a str> {
        self.expanded_name.and_then(|s| s.to_str().ok())
    }

    /// Whether [`name()`][OpenRequest::name] and
    /// [`expanded_name()`][OpenRequest::expanded_name] are file paths.
    pub fn is_file_path(&self) -> bool {
        self.is_file_path
    }

    /// The symbol messages should be issued against.
    ///
    /// This is a fully qualified symbol name, such as `` "System`OpenRead" ``.
    pub fn message_head(&self) -> Option<&'a str> {
        self.message_head.and_then(|s| s.to_str().ok())
    }

    /// The options the stream is being opened with.
    pub fn options(&mut self) -> StreamOptions<'_> {
        StreamOptions::new(self.options)
    }

    unsafe fn from_raw(
        name: *mut c_char,
        user_supplied_name: *mut c_char,
        expanded_name: *mut c_char,
        is_file_path: mbool,
        message_head: *const c_char,
        options: *mut c_void,
    ) -> OpenRequest<'a> {
        unsafe fn opt<'b>(ptr: *const c_char) -> Option<&'b CStr> {
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr))
            }
        }

        OpenRequest {
            name: opt(name).unwrap_or(EMPTY_CSTR),
            user_supplied_name: opt(user_supplied_name),
            expanded_name: opt(expanded_name),
            is_file_path: is_file_path != FALSE,
            message_head: opt(message_head),
            options,
            _marker: PhantomData,
        }
    }
}

impl<'a> std::fmt::Debug for OpenRequest<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenRequest")
            .field("name", &self.name)
            .field("user_supplied_name", &self.user_supplied_name)
            .field("expanded_name", &self.expanded_name)
            .field("is_file_path", &self.is_file_path)
            .field("message_head", &self.message_head)
            .finish_non_exhaustive()
    }
}

//======================================
// Stream traits
//======================================

/// A single open input stream.
///
/// Only [`read()`][InputStream::read] must be implemented; every other method
/// has a default corresponding to "this stream does not support that".
pub trait InputStream: Send + 'static {
    /// Read into `buf`, returning the number of bytes read.
    ///
    /// * `Ok(n)` with `n > 0` — that many bytes were placed in `buf`.
    /// * `Ok(0)` — end of stream, as in [`std::io::Read::read`].
    /// * `Err(`[`StreamError::WouldBlock`]`)` — no data *yet*. The kernel calls
    ///   [`wait_for_input()`][InputStream::wait_for_input] and reads again.
    /// * `Err(_)` — an error, reported to the user. Write the message in
    ///   sentence case with a period after each sentence; see
    ///   [`StreamError::Error`].
    ///
    /// *LibraryLink C field:* [`Mfread`][sys::st_MInputStream::Mfread].
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, StreamError>;

    /// Reposition the stream to an absolute `offset`.
    ///
    /// The kernel passes an offset of its own choosing, which is not
    /// necessarily the position the user asked for — it buffers input and seeks
    /// to a buffer boundary, then reads forward. Treat `offset` as an absolute
    /// position that must be honored exactly.
    ///
    /// A successful seek clears the end-of-stream flag.
    ///
    /// *LibraryLink C field:* [`Mfseek`][sys::st_MInputStream::Mfseek].
    fn seek(&mut self, offset: i64) -> Result<(), StreamError> {
        let _ = offset;
        Err(StreamError::Unsupported)
    }

    /// Whether this stream supports [`seek()`][InputStream::seek].
    ///
    /// This is what the Wolfram Language consults to decide whether the stream
    /// can be repositioned, so it must agree with `seek()`.
    ///
    /// *LibraryLink C field:* [`SeekableQ`][sys::st_MInputStream::SeekableQ].
    fn is_seekable(&self) -> bool {
        false
    }

    /// The current position in the stream.
    ///
    /// The kernel tracks input stream positions itself and does not currently
    /// call this; when it is not implemented, this module reports the number of
    /// bytes read so far, adjusted by successful seeks.
    ///
    /// *LibraryLink C field:* [`Mftell`][sys::st_MInputStream::Mftell].
    fn tell(&mut self) -> Result<i64, StreamError> {
        Err(StreamError::Unsupported)
    }

    /// The total size of the stream, if known.
    ///
    /// Reported to the Wolfram Language as "unknown" when unsupported.
    ///
    /// *LibraryLink C field:* [`Mstreamsize`][sys::st_MInputStream::Mstreamsize].
    fn size(&mut self) -> Result<i64, StreamError> {
        Err(StreamError::Unsupported)
    }

    /// Wait for input to become available.
    ///
    /// Called after [`read()`][InputStream::read] returns
    /// [`StreamError::WouldBlock`]. Waiting a short, bounded time and returning
    /// is correct and expected — the kernel simply reads again. An
    /// implementation that blocks indefinitely should poll
    /// [`aborted()`][crate::aborted] and return when it is `true`, or the user
    /// will not be able to interrupt the read.
    ///
    /// *LibraryLink C field:* [`WaitForInput`][sys::st_MInputStream::WaitForInput].
    fn wait_for_input(&mut self) {}

    /// The size of the units this stream deals in.
    ///
    /// *LibraryLink C field:* [`MstreamUnitSize`][sys::st_MInputStream::MstreamUnitSize].
    fn unit_size(&self) -> StreamUnitSize {
        StreamUnitSize::Bytes8
    }

    /// Handle the options of this stream being queried or changed.
    ///
    /// *LibraryLink C field:* [`MoptionChanges`][sys::st_MInputStream::MoptionChanges].
    fn set_options(&mut self, options: StreamOptions<'_>) {
        let _ = options;
    }

    /// Close the stream.
    ///
    /// [`Drop`] runs afterwards, so this is only needed to report a failure
    /// that closing can produce.
    ///
    /// *LibraryLink C field:* [`Mfclose`][sys::st_MInputStream::Mfclose].
    fn close(self) -> Result<(), StreamError>
    where
        Self: Sized,
    {
        Ok(())
    }
}

/// Marker for [`InputStream`]s that support repositioning.
///
/// Implementing this is not required — [`InputStream::is_seekable`] is what the
/// kernel consults — but it documents the capability and can be used as a bound.
/// `#[derive(SeekableInputStream)]` implements it for you.
pub trait SeekableInputStream: InputStream {}

/// A single open output stream.
///
/// Only [`write()`][OutputStream::write] must be implemented.
pub trait OutputStream: Send + 'static {
    /// Write `buf`, returning the number of bytes accepted.
    ///
    /// A short write is fine: this module calls `write()` again with the
    /// remainder until the whole buffer is consumed. (Nothing in the Wolfram
    /// Language retries a partial write, so a short write that escaped to the
    /// kernel would silently lose those bytes.) Repeatedly accepting zero bytes
    /// is treated as an error, to avoid spinning.
    ///
    /// # Reporting errors
    ///
    /// LibraryLink gives output streams no real error channel. Returning `Err`
    /// causes `BinaryWrite` to report a generic write failure, but `WriteString`
    /// and `Write` report *nothing*. If the user must be told what went wrong,
    /// raise a message from
    /// [`report_error()`][OutputStream::report_error].
    ///
    /// [`StreamError::WouldBlock`] is treated as an ordinary error here: the C
    /// API has no wait-for-output counterpart to
    /// [`InputStream::wait_for_input`], so there is nothing to wait on.
    ///
    /// *LibraryLink C field:* [`Mfwrite`][sys::st_MOutputStream::Mfwrite].
    fn write(&mut self, buf: &[u8]) -> Result<usize, StreamError>;

    /// Flush buffered output.
    ///
    /// *LibraryLink C field:* [`Mfflush`][sys::st_MOutputStream::Mfflush].
    fn flush(&mut self) -> Result<(), StreamError> {
        Ok(())
    }

    /// The current position in the stream.
    ///
    /// Unlike input streams, this *is* consulted — it is what
    /// [`StreamPosition`][ref/StreamPosition]<sub>WL</sub> reports. When not
    /// implemented, this module reports the number of bytes written so far,
    /// which is correct except for a stream opened in
    /// [`OutputMode::Append`] over existing content.
    ///
    /// *LibraryLink C field:* [`Mftell`][sys::st_MOutputStream::Mftell].
    ///
    /// [ref/StreamPosition]: https://reference.wolfram.com/language/ref/StreamPosition.html
    fn tell(&mut self) -> Result<i64, StreamError> {
        Err(StreamError::Unsupported)
    }

    /// The size of the units this stream deals in.
    ///
    /// *LibraryLink C field:* [`MstreamUnitSize`][sys::st_MOutputStream::MstreamUnitSize].
    fn unit_size(&self) -> StreamUnitSize {
        StreamUnitSize::Bytes8
    }

    /// Handle the options of this stream being queried or changed.
    ///
    /// *LibraryLink C field:* [`MoptionChanges`][sys::st_MOutputStream::MoptionChanges].
    fn set_options(&mut self, options: StreamOptions<'_>) {
        let _ = options;
    }

    /// Called after an operation on this stream fails.
    ///
    /// This is the hook for making a write failure visible, since the Wolfram
    /// Language will otherwise report little or nothing (see
    /// [`write()`][OutputStream::write]). An implementation can raise a message
    /// by evaluating one with [`evaluate()`][crate::evaluate].
    ///
    /// The default does nothing.
    fn report_error(&mut self, error: &StreamError) {
        let _ = error;
    }

    /// Close the stream.
    ///
    /// *LibraryLink C field:* [`Mfclose`][sys::st_MOutputStream::Mfclose].
    fn close(self) -> Result<(), StreamError>
    where
        Self: Sized,
    {
        Ok(())
    }
}

//======================================
// Method traits
//======================================

/// A registered input stream method: the factory the Wolfram Language calls to
/// open a stream.
pub trait InputStreamMethod: Send + Sync + 'static {
    /// The per-stream state this method produces.
    type Stream: InputStream;

    /// Whether this method should be consulted for streams opened *without* an
    /// explicit `Method` option.
    ///
    /// When `false` (the default), no handler test is registered and
    /// [`handles_name()`][InputStreamMethod::handles_name] is never called: the
    /// method is used only when named explicitly, as in
    /// `OpenRead[name, Method -> "MyMethod"]`.
    ///
    /// Set this to `true` **and** implement `handles_name()` to claim names by
    /// pattern — implementing one without the other has no effect.
    const NAME_DISPATCH: bool = false;

    /// Open a new stream.
    ///
    /// Returning `Err` makes the `OpenRead` fail, reporting the error's message.
    fn open(&self, request: &mut OpenRequest) -> Result<Self::Stream, StreamError>;

    /// Whether this method handles a stream opened under `name`.
    ///
    /// Only called when [`NAME_DISPATCH`][InputStreamMethod::NAME_DISPATCH] is
    /// `true`.
    ///
    /// *LibraryLink C parameter:* `handlerTest` of
    /// [`registerInputStreamMethod`][sys::st_WolframLibraryData::registerInputStreamMethod].
    fn handles_name(&self, name: &str) -> bool {
        let _ = name;
        false
    }
}

/// A registered output stream method.
pub trait OutputStreamMethod: Send + Sync + 'static {
    /// The per-stream state this method produces.
    type Stream: OutputStream;

    /// Whether this method should be consulted for streams opened *without* an
    /// explicit `Method` option. See
    /// [`InputStreamMethod::NAME_DISPATCH`].
    const NAME_DISPATCH: bool = false;

    /// Open a new stream.
    fn open(
        &self,
        request: &mut OpenRequest,
        mode: OutputMode,
    ) -> Result<Self::Stream, StreamError>;

    /// Whether this method handles a stream opened under `name`.
    ///
    /// Only called when [`NAME_DISPATCH`][OutputStreamMethod::NAME_DISPATCH] is
    /// `true`.
    fn handles_name(&self, name: &str) -> bool {
        let _ = name;
        false
    }
}

//======================================
// Registration
//======================================

/// A registered input stream method.
///
/// Dropping this handle does **not** unregister the method — a stream method is
/// normally registered for the life of the library, and unregistering one while
/// streams are still open would leave the kernel calling into freed state. Call
/// [`unregister()`][InputStreamMethodHandle::unregister] explicitly.
#[derive(Debug)]
pub struct InputStreamMethodHandle {
    name: CString,
}

impl InputStreamMethodHandle {
    /// The name this method is registered under.
    pub fn name(&self) -> &str {
        self.name.to_str().unwrap_or("")
    }

    /// Unregister this method.
    ///
    /// *LibraryLink C Function:* [`unregisterInputStreamMethod`][sys::st_WolframLibraryData::unregisterInputStreamMethod].
    pub fn unregister(self) {
        let ok = unsafe { rtl::unregisterInputStreamMethod(self.name.as_ptr()) };

        if !crate::bool_from_mbool(ok) {
            panic!(
                "no input stream method with name '{}' is registered",
                self.name.to_string_lossy()
            );
        }
    }
}

/// A registered output stream method. See [`InputStreamMethodHandle`].
#[derive(Debug)]
pub struct OutputStreamMethodHandle {
    name: CString,
}

impl OutputStreamMethodHandle {
    /// The name this method is registered under.
    pub fn name(&self) -> &str {
        self.name.to_str().unwrap_or("")
    }

    /// Unregister this method.
    ///
    /// *LibraryLink C Function:* [`unregisterOutputStreamMethod`][sys::st_WolframLibraryData::unregisterOutputStreamMethod].
    pub fn unregister(self) {
        let ok = unsafe { rtl::unregisterOutputStreamMethod(self.name.as_ptr()) };

        if !crate::bool_from_mbool(ok) {
            panic!(
                "no output stream method with name '{}' is registered",
                self.name.to_string_lossy()
            );
        }
    }
}

/// Register an input stream method under `name`.
///
/// The Wolfram Language can then open streams with it:
///
/// ```wolfram
/// OpenRead["some-name", Method -> name]
/// ```
///
/// Call this from a function annotated with [`#[init]`][crate::init].
///
/// # Panics
///
/// Panics if a stream method is already registered under `name`.
///
/// *LibraryLink C Function:* [`registerInputStreamMethod`][sys::st_WolframLibraryData::registerInputStreamMethod].
pub fn register_input_stream_method<M: InputStreamMethod>(
    name: &str,
    method: M,
) -> InputStreamMethodHandle {
    let name = CString::new(name).expect("failed to allocate C string");

    // The method itself is the C API's `methodData`. The kernel hands it back
    // through the stream's `handlerIdentity` field, which is how `input_ctor`
    // reaches it — the constructor callback takes no `methodData` parameter.
    let method_data = Box::into_raw(Box::new(method)) as *mut c_void;

    let handler_test = if M::NAME_DISPATCH {
        Some(input_handler_test::<M> as unsafe extern "C" fn(_, _) -> _)
    } else {
        None
    };

    let ok = unsafe {
        rtl::registerInputStreamMethod(
            name.as_ptr(),
            Some(input_ctor::<M>),
            handler_test,
            method_data,
            Some(drop_method_data::<M>),
        )
    };

    if !crate::bool_from_mbool(ok) {
        // Registration failed, so `destroyMethod` will never run. Reclaim the
        // box here rather than leaking it.
        drop(unsafe { Box::from_raw(method_data as *mut M) });
        panic!(
            "input stream method with name '{}' has already been registered",
            name.to_string_lossy()
        );
    }

    InputStreamMethodHandle { name }
}

/// Register an output stream method under `name`.
///
/// The Wolfram Language can then open streams with it:
///
/// ```wolfram
/// OpenWrite["some-name", Method -> name]
/// ```
///
/// # Panics
///
/// Panics if a stream method is already registered under `name`.
///
/// *LibraryLink C Function:* [`registerOutputStreamMethod`][sys::st_WolframLibraryData::registerOutputStreamMethod].
pub fn register_output_stream_method<M: OutputStreamMethod>(
    name: &str,
    method: M,
) -> OutputStreamMethodHandle {
    let name = CString::new(name).expect("failed to allocate C string");

    let method_data = Box::into_raw(Box::new(method)) as *mut c_void;

    let handler_test = if M::NAME_DISPATCH {
        Some(output_handler_test::<M> as unsafe extern "C" fn(_, _) -> _)
    } else {
        None
    };

    let ok = unsafe {
        rtl::registerOutputStreamMethod(
            name.as_ptr(),
            Some(output_ctor::<M>),
            handler_test,
            method_data,
            Some(drop_method_data::<M>),
        )
    };

    if !crate::bool_from_mbool(ok) {
        drop(unsafe { Box::from_raw(method_data as *mut M) });
        panic!(
            "output stream method with name '{}' has already been registered",
            name.to_string_lossy()
        );
    }

    OutputStreamMethodHandle { name }
}

//======================================
// Panic handling
//======================================

/// Run a stream callback, converting a panic into a [`StreamError`].
///
/// Unwinding out of an `extern "C"` callback is undefined behavior, so every
/// trampoline funnels through this.
///
/// This deliberately does not use [`call_and_catch_panic`][crate::call_and_catch_panic]:
/// that installs and removes a global panic hook on every call, which is far too
/// much for something the kernel invokes once per 64 KiB of I/O.
fn catch<T>(what: &str, f: impl FnOnce() -> T) -> Result<T, StreamError> {
    panic::catch_unwind(AssertUnwindSafe(f)).map_err(|payload| {
        let detail = if let Some(s) = payload.downcast_ref::<&'static str>() {
            (*s).to_owned()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            String::from("Box<dyn Any>")
        };

        StreamError::Error(format!(
            "Rust panic in stream {what}: {}",
            end_sentence(&detail)
        ))
    })
}

/// Give `message` a terminating period if it does not already end a sentence.
///
/// Panic messages come from user code and may or may not be punctuated, but
/// what reaches the Wolfram Language should read as a sentence either way (see
/// [`StreamError::Error`]).
fn end_sentence(message: &str) -> String {
    let trimmed = message.trim_end();

    if trimmed.ends_with(['.', '!', '?']) {
        trimmed.to_owned()
    } else {
        format!("{trimmed}.")
    }
}

//======================================
// Per-stream state
//======================================

/// What a stream's `MSdata` points at.
///
/// The error text lives here rather than in the user's stream so that the
/// pointer returned by `MferrorText` stays valid — the kernel borrows it and
/// polls it after every read.
struct StreamBox<S> {
    stream: Option<S>,
    error: Option<CString>,
    eof: bool,
    position: i64,
}

impl<S> StreamBox<S> {
    fn new(stream: S) -> Self {
        StreamBox {
            stream: Some(stream),
            error: None,
            eof: false,
            position: 0,
        }
    }

    fn set_error(&mut self, error: &StreamError) {
        let Some(message) = error.message() else {
            return;
        };

        // A NUL in the message would truncate it; replace rather than drop the
        // error entirely.
        self.error = Some(CString::new(message).unwrap_or_else(|_| {
            CString::new(message.replace('\0', "\u{fffd}")).unwrap()
        }));
    }

    fn error_ptr(&self) -> *mut c_char {
        match &self.error {
            Some(text) => text.as_ptr() as *mut c_char,
            None => ptr::null_mut(),
        }
    }
}

/// Borrow the `StreamBox` out of a stream's `MSdata`.
///
/// Returns `None` once the stream has been closed, so a callback the kernel
/// makes after `Mfclose` is an inert no-op rather than a use-after-free.
unsafe fn input_box<'a, S>(strm: MInputStream) -> Option<&'a mut StreamBox<S>> {
    if strm.is_null() || (*strm).MSdata.is_null() {
        return None;
    }
    Some(&mut *((*strm).MSdata as *mut StreamBox<S>))
}

unsafe fn output_box<'a, S>(strm: MOutputStream) -> Option<&'a mut StreamBox<S>> {
    if strm.is_null() || (*strm).MSdata.is_null() {
        return None;
    }
    Some(&mut *((*strm).MSdata as *mut StreamBox<S>))
}

//======================================
// Input trampolines
//======================================

unsafe extern "C" fn input_ctor<M: InputStreamMethod>(
    strm: MInputStream,
    msg_head: *const c_char,
    options: *mut c_void,
) {
    if strm.is_null() {
        return;
    }

    // The kernel sets `handlerIdentity` to the `methodData` this method was
    // registered with, before calling the constructor.
    let method = (*strm).handlerIdentity as *const M;
    if method.is_null() {
        (*strm).hasError = TRUE;
        return;
    }
    let method: &M = &*method;

    let mut request = OpenRequest::from_raw(
        (*strm).name,
        (*strm).userSuppliedName,
        (*strm).expandedName,
        (*strm).filePathNameQ,
        msg_head,
        options,
    );

    let result = catch("constructor", || method.open(&mut request))
        .and_then(std::convert::identity);

    let stream = match result {
        Ok(stream) => stream,
        Err(error) => {
            // Allocate a box holding just the error text, so the Wolfram
            // Language can report *why* the open failed rather than only that
            // it did. `hasError` is what makes the open fail.
            let mut boxed: StreamBox<M::Stream> = StreamBox {
                stream: None,
                error: None,
                eof: true,
                position: 0,
            };
            boxed.set_error(&error);

            (*strm).MSdata = Box::into_raw(Box::new(boxed)) as *mut c_void;
            (*strm).MferrorText = Some(input_error_text::<M::Stream>);
            (*strm).Mclearerr = Some(input_clear_error::<M::Stream>);
            (*strm).Mfclose = Some(input_close::<M::Stream>);
            (*strm).hasError = TRUE;
            return;
        },
    };

    (*strm).MSdata = Box::into_raw(Box::new(StreamBox::new(stream))) as *mut c_void;
    (*strm).isClosed = FALSE;

    (*strm).Mfread = Some(input_read::<M::Stream>);
    (*strm).Mfeof = Some(input_eof::<M::Stream>);
    (*strm).Mfseek = Some(input_seek::<M::Stream>);
    (*strm).SeekableQ = Some(input_seekable_q::<M::Stream>);
    (*strm).Mftell = Some(input_tell::<M::Stream>);
    (*strm).Mstreamsize = Some(input_stream_size::<M::Stream>);
    (*strm).MferrorText = Some(input_error_text::<M::Stream>);
    (*strm).Mclearerr = Some(input_clear_error::<M::Stream>);
    (*strm).WaitForInput = Some(input_wait_for_input::<M::Stream>);
    (*strm).MstreamUnitSize = Some(input_unit_size::<M::Stream>);
    (*strm).MoptionChanges = Some(input_option_changes::<M::Stream>);
    (*strm).Mfclose = Some(input_close::<M::Stream>);
}

unsafe extern "C" fn input_handler_test<M: InputStreamMethod>(
    handler_identity: *mut c_void,
    name: *mut c_char,
) -> mbool {
    if handler_identity.is_null() || name.is_null() {
        return FALSE;
    }

    let method: &M = &*(handler_identity as *const M);
    let Ok(name) = CStr::from_ptr(name).to_str() else {
        return FALSE;
    };

    match catch("handler test", || method.handles_name(name)) {
        Ok(true) => TRUE,
        Ok(false) | Err(_) => FALSE,
    }
}

unsafe extern "C" fn drop_method_data<M>(method_data: *mut c_void) {
    if method_data.is_null() {
        return;
    }
    let _ = catch("method destructor", || {
        drop(Box::from_raw(method_data as *mut M))
    });
}

unsafe extern "C" fn input_read<S: InputStream>(
    strm: MInputStream,
    buf: *mut c_void,
    count: usize,
) -> mint {
    let Some(boxed) = input_box::<S>(strm) else {
        return 0;
    };
    let Some(stream) = boxed.stream.as_mut() else {
        return 0;
    };

    if count == 0 {
        return 0;
    }

    let slice = std::slice::from_raw_parts_mut(buf as *mut u8, count);

    let result = catch("read", || stream.read(slice)).and_then(std::convert::identity);

    match result {
        // End of stream. `Mfread` returning 0 is ambiguous on its own; `Mfeof`
        // is what distinguishes this from "not ready yet".
        Ok(0) => {
            boxed.eof = true;
            0
        },
        Ok(read) => {
            // Defend against an implementation reporting more than it was given.
            let read = std::cmp::min(read, count);
            boxed.position += read as i64;
            read as mint
        },
        // Not ready. Report no bytes *without* setting eof or an error, which is
        // what makes the kernel call `WaitForInput` and read again.
        Err(StreamError::WouldBlock) => 0,
        Err(error) => {
            boxed.set_error(&error);
            // Never return a negative value here: the kernel consumes this as a
            // length and crashes on a negative one.
            0
        },
    }
}

unsafe extern "C" fn input_eof<S: InputStream>(strm: MInputStream) -> c_int {
    match input_box::<S>(strm) {
        Some(boxed) => c_int::from(boxed.eof),
        None => 1,
    }
}

unsafe extern "C" fn input_seek<S: InputStream>(
    strm: MInputStream,
    offset: i64,
) -> c_int {
    let Some(boxed) = input_box::<S>(strm) else {
        return 1;
    };
    let Some(stream) = boxed.stream.as_mut() else {
        return 1;
    };

    let result = catch("seek", || stream.seek(offset)).and_then(std::convert::identity);

    match result {
        Ok(()) => {
            boxed.position = offset;
            // Repositioning makes the stream readable again.
            boxed.eof = false;
            0
        },
        Err(error) => {
            boxed.set_error(&error);
            1
        },
    }
}

unsafe extern "C" fn input_seekable_q<S: InputStream>(strm: MInputStream) -> mbool {
    let Some(boxed) = input_box::<S>(strm) else {
        return FALSE;
    };
    let Some(stream) = boxed.stream.as_ref() else {
        return FALSE;
    };

    match catch("seekable test", || stream.is_seekable()) {
        Ok(true) => TRUE,
        Ok(false) | Err(_) => FALSE,
    }
}

unsafe extern "C" fn input_tell<S: InputStream>(strm: MInputStream) -> i64 {
    let Some(boxed) = input_box::<S>(strm) else {
        return -1;
    };
    let Some(stream) = boxed.stream.as_mut() else {
        return -1;
    };

    match catch("tell", || stream.tell()).and_then(std::convert::identity) {
        Ok(position) => position,
        // Fall back to the bytes-transferred counter this module keeps.
        Err(StreamError::Unsupported) => boxed.position,
        Err(error) => {
            boxed.set_error(&error);
            -1
        },
    }
}

unsafe extern "C" fn input_stream_size<S: InputStream>(strm: MInputStream) -> i64 {
    let Some(boxed) = input_box::<S>(strm) else {
        return -1;
    };
    let Some(stream) = boxed.stream.as_mut() else {
        return -1;
    };

    match catch("size", || stream.size()).and_then(std::convert::identity) {
        Ok(size) => size,
        // -1 is how the C API spells "unknown".
        Err(StreamError::Unsupported) => -1,
        Err(error) => {
            boxed.set_error(&error);
            -1
        },
    }
}

unsafe extern "C" fn input_error_text<S: InputStream>(strm: MInputStream) -> *mut c_char {
    match input_box::<S>(strm) {
        Some(boxed) => boxed.error_ptr(),
        None => ptr::null_mut(),
    }
}

unsafe extern "C" fn input_clear_error<S: InputStream>(strm: MInputStream) {
    if let Some(boxed) = input_box::<S>(strm) {
        // Clear only the error. The end-of-stream flag is left alone: a stream
        // that has genuinely ended has not un-ended, and the case that must
        // reset it -- repositioning -- is handled by `input_seek`.
        boxed.error = None;
    }
    if !strm.is_null() {
        (*strm).hasError = FALSE;
    }
}

unsafe extern "C" fn input_wait_for_input<S: InputStream>(strm: MInputStream) {
    let Some(boxed) = input_box::<S>(strm) else {
        return;
    };
    let Some(stream) = boxed.stream.as_mut() else {
        return;
    };

    if let Err(error) = catch("wait for input", || stream.wait_for_input()) {
        boxed.set_error(&error);
    }
}

unsafe extern "C" fn input_unit_size<S: InputStream>(
    strm: MInputStream,
) -> sys::MStream_StreamUnitSize_t {
    let unit = input_box::<S>(strm)
        .and_then(|boxed| boxed.stream.as_ref())
        .and_then(|stream| catch("unit size", || stream.unit_size()).ok())
        .unwrap_or(StreamUnitSize::Bytes8);

    unit.to_raw()
}

unsafe extern "C" fn input_option_changes<S: InputStream>(
    strm: MInputStream,
    options: *mut c_void,
) {
    let Some(boxed) = input_box::<S>(strm) else {
        return;
    };
    let Some(stream) = boxed.stream.as_mut() else {
        return;
    };

    let _ = catch("option changes", || {
        stream.set_options(StreamOptions::new(options))
    });
}

unsafe extern "C" fn input_close<S: InputStream>(strm: MInputStream) -> c_int {
    if strm.is_null() || (*strm).MSdata.is_null() {
        return 1;
    }

    let mut boxed = Box::from_raw((*strm).MSdata as *mut StreamBox<S>);

    // Null this before running user code, so that a callback made during or
    // after the close finds a closed stream rather than a freed box.
    (*strm).MSdata = ptr::null_mut();
    (*strm).isClosed = TRUE;

    match boxed.stream.take() {
        Some(stream) => match catch("close", || stream.close()) {
            Ok(Ok(())) => 0,
            Ok(Err(_)) | Err(_) => 1,
        },
        None => 0,
    }
}

//======================================
// Output trampolines
//======================================

unsafe extern "C" fn output_ctor<M: OutputStreamMethod>(
    strm: MOutputStream,
    msg_head: *const c_char,
    options: *mut c_void,
    append_mode: mbool,
) {
    if strm.is_null() {
        return;
    }

    let method = (*strm).handlerIdentity as *const M;
    if method.is_null() {
        (*strm).hasError = TRUE;
        return;
    }
    let method: &M = &*method;

    let mode = if append_mode != FALSE {
        OutputMode::Append
    } else {
        OutputMode::Truncate
    };

    let mut request = OpenRequest::from_raw(
        (*strm).name,
        (*strm).userSuppliedName,
        (*strm).expandedName,
        (*strm).filePathNameQ,
        msg_head,
        options,
    );

    let result = catch("constructor", || method.open(&mut request, mode))
        .and_then(std::convert::identity);

    let stream = match result {
        Ok(stream) => stream,
        Err(error) => {
            let mut boxed: StreamBox<M::Stream> = StreamBox {
                stream: None,
                error: None,
                eof: true,
                position: 0,
            };
            boxed.set_error(&error);

            (*strm).MSdata = Box::into_raw(Box::new(boxed)) as *mut c_void;
            (*strm).MferrorText = Some(output_error_text::<M::Stream>);
            (*strm).Mclearerr = Some(output_clear_error::<M::Stream>);
            (*strm).Mfclose = Some(output_close::<M::Stream>);
            (*strm).hasError = TRUE;
            return;
        },
    };

    (*strm).MSdata = Box::into_raw(Box::new(StreamBox::new(stream))) as *mut c_void;
    (*strm).isClosed = FALSE;

    (*strm).Mfwrite = Some(output_write::<M::Stream>);
    (*strm).Mfflush = Some(output_flush::<M::Stream>);
    (*strm).Mftell = Some(output_tell::<M::Stream>);
    (*strm).MferrorText = Some(output_error_text::<M::Stream>);
    (*strm).Mclearerr = Some(output_clear_error::<M::Stream>);
    (*strm).MstreamUnitSize = Some(output_unit_size::<M::Stream>);
    (*strm).MoptionChanges = Some(output_option_changes::<M::Stream>);
    (*strm).Mfclose = Some(output_close::<M::Stream>);
}

unsafe extern "C" fn output_handler_test<M: OutputStreamMethod>(
    handler_identity: *mut c_void,
    name: *mut c_char,
) -> mbool {
    if handler_identity.is_null() || name.is_null() {
        return FALSE;
    }

    let method: &M = &*(handler_identity as *const M);
    let Ok(name) = CStr::from_ptr(name).to_str() else {
        return FALSE;
    };

    match catch("handler test", || method.handles_name(name)) {
        Ok(true) => TRUE,
        Ok(false) | Err(_) => FALSE,
    }
}

/// Record an output stream failure and give the stream a chance to raise a
/// message about it.
///
/// The error text is stored for `MferrorText` -- harmless, and correct if the
/// Wolfram Language ever starts consulting it on output streams -- but
/// [`OutputStream::report_error`] is the only hook that can actually make the
/// failure visible today.
fn report_failure<S: OutputStream>(boxed: &mut StreamBox<S>, error: &StreamError) {
    boxed.set_error(error);

    if let Some(stream) = boxed.stream.as_mut() {
        let _ = catch("error report", || stream.report_error(error));
    }
}

unsafe extern "C" fn output_write<S: OutputStream>(
    strm: MOutputStream,
    buf: *mut c_void,
    count: usize,
) -> mint {
    let Some(boxed) = output_box::<S>(strm) else {
        return 0;
    };

    if count == 0 || boxed.stream.is_none() {
        return 0;
    }

    let slice = std::slice::from_raw_parts(buf as *const u8, count);

    // Nothing in the Wolfram Language retries a partial write, so loop here
    // rather than letting a short write silently drop the remainder. The stream
    // is re-borrowed each iteration so the borrow ends before `report_failure`.
    let mut written = 0usize;
    let mut failure: Option<StreamError> = None;

    while written < count {
        let Some(stream) = boxed.stream.as_mut() else {
            break;
        };

        let result = catch("write", || stream.write(&slice[written..]))
            .and_then(std::convert::identity);

        match result {
            Ok(0) => {
                failure = Some(StreamError::new(
                    "The output stream accepted none of the bytes written.",
                ));
                break;
            },
            Ok(n) => written = std::cmp::min(written + n, count),
            Err(error) => {
                failure = Some(error);
                break;
            },
        }
    }

    if let Some(error) = failure {
        report_failure(boxed, &error);
    }

    boxed.position += written as i64;
    written as mint
}

unsafe extern "C" fn output_flush<S: OutputStream>(strm: MOutputStream) -> c_int {
    let Some(boxed) = output_box::<S>(strm) else {
        return 1;
    };

    let result = match boxed.stream.as_mut() {
        Some(stream) => {
            catch("flush", || stream.flush()).and_then(std::convert::identity)
        },
        None => return 1,
    };

    match result {
        Ok(()) => 0,
        Err(error) => {
            report_failure(boxed, &error);
            1
        },
    }
}

unsafe extern "C" fn output_tell<S: OutputStream>(strm: MOutputStream) -> i64 {
    let Some(boxed) = output_box::<S>(strm) else {
        return -1;
    };
    let Some(stream) = boxed.stream.as_mut() else {
        return -1;
    };

    match catch("tell", || stream.tell()).and_then(std::convert::identity) {
        Ok(position) => position,
        Err(StreamError::Unsupported) => boxed.position,
        Err(error) => {
            boxed.set_error(&error);
            -1
        },
    }
}

unsafe extern "C" fn output_error_text<S: OutputStream>(
    strm: MOutputStream,
) -> *mut c_char {
    match output_box::<S>(strm) {
        Some(boxed) => boxed.error_ptr(),
        None => ptr::null_mut(),
    }
}

unsafe extern "C" fn output_clear_error<S: OutputStream>(strm: MOutputStream) {
    if let Some(boxed) = output_box::<S>(strm) {
        boxed.error = None;
    }
    if !strm.is_null() {
        (*strm).hasError = FALSE;
    }
}

unsafe extern "C" fn output_unit_size<S: OutputStream>(
    strm: MOutputStream,
) -> sys::MStream_StreamUnitSize_t {
    let unit = output_box::<S>(strm)
        .and_then(|boxed| boxed.stream.as_ref())
        .and_then(|stream| catch("unit size", || stream.unit_size()).ok())
        .unwrap_or(StreamUnitSize::Bytes8);

    unit.to_raw()
}

unsafe extern "C" fn output_option_changes<S: OutputStream>(
    strm: MOutputStream,
    options: *mut c_void,
) {
    let Some(boxed) = output_box::<S>(strm) else {
        return;
    };
    let Some(stream) = boxed.stream.as_mut() else {
        return;
    };

    let _ = catch("option changes", || {
        stream.set_options(StreamOptions::new(options))
    });
}

unsafe extern "C" fn output_close<S: OutputStream>(strm: MOutputStream) -> c_int {
    if strm.is_null() || (*strm).MSdata.is_null() {
        return 1;
    }

    let mut boxed = Box::from_raw((*strm).MSdata as *mut StreamBox<S>);

    (*strm).MSdata = ptr::null_mut();
    (*strm).isClosed = TRUE;

    match boxed.stream.take() {
        Some(stream) => match catch("close", || stream.close()) {
            Ok(Ok(())) => 0,
            Ok(Err(_)) | Err(_) => 1,
        },
        None => 0,
    }
}

//======================================
// std::io adapters
//======================================

/// Blanket helper so a `Read + Seek` value can be stored as a trait object.
trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

trait WriteSeek: Write + Seek + Send {}
impl<T: Write + Seek + Send> WriteSeek for T {}

enum Reader {
    Plain(Box<dyn Read + Send>),
    Seekable(Box<dyn ReadSeek>),
}

/// An [`InputStream`] backed by any [`std::io::Read`].
///
/// Use this to expose a reader — a [`File`][std::fs::File], a
/// [`TcpStream`][std::net::TcpStream], a decompressor — as a Wolfram Language
/// stream, without implementing [`InputStream`] by hand.
///
/// The underlying reader is type-erased, so a single method can return readers
/// of different concrete types from the same `open()`.
///
/// # Example
///
/// ```no_run
/// use std::fs::File;
/// use wolfram_library_link::stream::ReaderInputStream;
///
/// # fn main() -> std::io::Result<()> {
/// // Not seekable: reads only.
/// let stream = ReaderInputStream::new(File::open("data.bin")?);
///
/// // Seekable: `StreamPosition` and `SetStreamPosition` work.
/// let stream = ReaderInputStream::seekable(File::open("data.bin")?);
/// # Ok(())
/// # }
/// ```
pub struct ReaderInputStream {
    reader: Reader,
    size: Option<i64>,
    unit_size: StreamUnitSize,
}

impl ReaderInputStream {
    /// Wrap a reader as a non-seekable input stream.
    pub fn new<R: Read + Send + 'static>(reader: R) -> ReaderInputStream {
        ReaderInputStream {
            reader: Reader::Plain(Box::new(reader)),
            size: None,
            unit_size: StreamUnitSize::Bytes8,
        }
    }

    /// Wrap a seekable reader as a seekable input stream.
    ///
    /// [`InputStream::is_seekable`] reports `true`, and
    /// [`seek`][InputStream::seek], [`tell`][InputStream::tell] and
    /// [`size`][InputStream::size] are served by the underlying
    /// [`Seek`].
    pub fn seekable<R: Read + Seek + Send + 'static>(reader: R) -> ReaderInputStream {
        ReaderInputStream {
            reader: Reader::Seekable(Box::new(reader)),
            size: None,
            unit_size: StreamUnitSize::Bytes8,
        }
    }

    /// Report a known total size, for streams whose length is known up front but
    /// which cannot seek.
    ///
    /// Overrides the size derived from a seekable reader.
    pub fn with_size(mut self, size: i64) -> ReaderInputStream {
        self.size = Some(size);
        self
    }

    /// Set the stream's unit size. Defaults to [`StreamUnitSize::Bytes8`].
    pub fn with_unit_size(mut self, unit_size: StreamUnitSize) -> ReaderInputStream {
        self.unit_size = unit_size;
        self
    }
}

impl InputStream for ReaderInputStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, StreamError> {
        let read = match &mut self.reader {
            Reader::Plain(reader) => reader.read(buf)?,
            Reader::Seekable(reader) => reader.read(buf)?,
        };
        Ok(read)
    }

    fn seek(&mut self, offset: i64) -> Result<(), StreamError> {
        match &mut self.reader {
            Reader::Plain(_) => Err(StreamError::Unsupported),
            Reader::Seekable(reader) => {
                reader.seek(SeekFrom::Start(offset.max(0) as u64))?;
                Ok(())
            },
        }
    }

    fn is_seekable(&self) -> bool {
        matches!(self.reader, Reader::Seekable(_))
    }

    fn tell(&mut self) -> Result<i64, StreamError> {
        match &mut self.reader {
            Reader::Plain(_) => Err(StreamError::Unsupported),
            Reader::Seekable(reader) => Ok(reader.stream_position()? as i64),
        }
    }

    fn size(&mut self) -> Result<i64, StreamError> {
        if let Some(size) = self.size {
            return Ok(size);
        }

        match &mut self.reader {
            Reader::Plain(_) => Err(StreamError::Unsupported),
            Reader::Seekable(reader) => {
                // `Seek::stream_len` is unstable, so do it by hand and restore
                // the original position.
                let original = reader.stream_position()?;
                let size = reader.seek(SeekFrom::End(0))?;
                reader.seek(SeekFrom::Start(original))?;
                Ok(size as i64)
            },
        }
    }

    fn unit_size(&self) -> StreamUnitSize {
        self.unit_size
    }
}

impl SeekableInputStream for ReaderInputStream {}

enum Writer {
    Plain(Box<dyn Write + Send>),
    Seekable(Box<dyn WriteSeek>),
}

/// An [`OutputStream`] backed by any [`std::io::Write`].
///
/// # Example
///
/// ```no_run
/// use std::fs::File;
/// use wolfram_library_link::stream::WriterOutputStream;
///
/// # fn main() -> std::io::Result<()> {
/// let stream = WriterOutputStream::new(File::create("out.bin")?);
/// # Ok(())
/// # }
/// ```
pub struct WriterOutputStream {
    writer: Writer,
    unit_size: StreamUnitSize,
}

impl WriterOutputStream {
    /// Wrap a writer as an output stream.
    ///
    /// [`OutputStream::tell`] reports the number of bytes written through this
    /// stream.
    pub fn new<W: Write + Send + 'static>(writer: W) -> WriterOutputStream {
        WriterOutputStream {
            writer: Writer::Plain(Box::new(writer)),
            unit_size: StreamUnitSize::Bytes8,
        }
    }

    /// Wrap a seekable writer as an output stream.
    ///
    /// [`OutputStream::tell`] reports the underlying stream position rather than
    /// a count of bytes written, which is what you want for a file opened in
    /// [`OutputMode::Append`].
    pub fn seekable<W: Write + Seek + Send + 'static>(writer: W) -> WriterOutputStream {
        WriterOutputStream {
            writer: Writer::Seekable(Box::new(writer)),
            unit_size: StreamUnitSize::Bytes8,
        }
    }

    /// Set the stream's unit size. Defaults to [`StreamUnitSize::Bytes8`].
    pub fn with_unit_size(mut self, unit_size: StreamUnitSize) -> WriterOutputStream {
        self.unit_size = unit_size;
        self
    }

    fn as_write(&mut self) -> &mut dyn Write {
        match &mut self.writer {
            Writer::Plain(writer) => writer.as_mut(),
            Writer::Seekable(writer) => {
                // `dyn WriteSeek` is `Write`, but coercing through the trait
                // object needs a concrete reborrow.
                writer.as_mut() as &mut dyn Write
            },
        }
    }
}

impl OutputStream for WriterOutputStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, StreamError> {
        Ok(self.as_write().write(buf)?)
    }

    fn flush(&mut self) -> Result<(), StreamError> {
        self.as_write().flush()?;
        Ok(())
    }

    fn tell(&mut self) -> Result<i64, StreamError> {
        match &mut self.writer {
            Writer::Plain(_) => Err(StreamError::Unsupported),
            Writer::Seekable(writer) => Ok(writer.stream_position()? as i64),
        }
    }

    fn unit_size(&self) -> StreamUnitSize {
        self.unit_size
    }

    fn close(mut self) -> Result<(), StreamError> {
        self.as_write().flush()?;
        Ok(())
    }
}

//======================================
// Tests
//======================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn io_error_kinds_map_to_stream_errors() {
        use std::io::{Error, ErrorKind};

        assert!(matches!(
            StreamError::from(Error::new(ErrorKind::WouldBlock, "later")),
            StreamError::WouldBlock
        ));
        assert!(matches!(
            StreamError::from(Error::new(ErrorKind::Interrupted, "signal")),
            StreamError::WouldBlock
        ));
        assert!(matches!(
            StreamError::from(Error::new(ErrorKind::Unsupported, "nope")),
            StreamError::Unsupported
        ));
        assert!(matches!(
            StreamError::from(Error::new(ErrorKind::NotFound, "gone")),
            StreamError::Error(_)
        ));
    }

    #[test]
    fn would_block_produces_no_message() {
        // `WouldBlock` must not set error text: non-NULL error text is what the
        // kernel treats as a failure, and this case is not a failure.
        assert_eq!(StreamError::WouldBlock.message(), None);
        assert_eq!(StreamError::new("boom").message(), Some("boom"));
    }

    #[test]
    fn error_text_survives_interior_nul() {
        let mut boxed: StreamBox<ReaderInputStream> = StreamBox {
            stream: None,
            error: None,
            eof: false,
            position: 0,
        };

        boxed.set_error(&StreamError::new("bad\0message"));
        let text = boxed.error.as_ref().expect("error text was dropped");
        assert!(text.to_str().unwrap().starts_with("bad"));
        assert!(!text.to_bytes().is_empty());

        // `WouldBlock` leaves the error text alone.
        boxed.error = None;
        boxed.set_error(&StreamError::WouldBlock);
        assert!(boxed.error.is_none());
        assert!(boxed.error_ptr().is_null());
    }

    #[test]
    fn plain_reader_is_not_seekable() {
        let mut stream = ReaderInputStream::new(Cursor::new(b"hello".to_vec()));

        assert!(!stream.is_seekable());
        assert!(matches!(stream.seek(0), Err(StreamError::Unsupported)));
        assert!(matches!(stream.tell(), Err(StreamError::Unsupported)));
        assert!(matches!(stream.size(), Err(StreamError::Unsupported)));
    }

    #[test]
    fn seekable_reader_reports_position_and_size() {
        let mut stream =
            ReaderInputStream::seekable(Cursor::new(b"hello world".to_vec()));

        assert!(stream.is_seekable());
        assert_eq!(stream.size().unwrap(), 11);
        // Computing the size must not disturb the read position.
        assert_eq!(stream.tell().unwrap(), 0);

        let mut buf = [0u8; 5];
        assert_eq!(stream.read(&mut buf).unwrap(), 5);
        assert_eq!(&buf, b"hello");
        assert_eq!(stream.tell().unwrap(), 5);

        stream.seek(6).unwrap();
        assert_eq!(stream.tell().unwrap(), 6);
        assert_eq!(stream.read(&mut buf).unwrap(), 5);
        assert_eq!(&buf, b"world");

        // End of stream is `Ok(0)`, matching `std::io::Read`.
        assert_eq!(stream.read(&mut buf).unwrap(), 0);
    }

    #[test]
    fn explicit_size_overrides_seekable_size() {
        let mut stream =
            ReaderInputStream::new(Cursor::new(b"abc".to_vec())).with_size(99);
        assert_eq!(stream.size().unwrap(), 99);
    }

    #[test]
    fn writer_reports_bytes_written() {
        let mut stream = WriterOutputStream::seekable(Cursor::new(Vec::new()));

        assert_eq!(stream.write(b"abcde").unwrap(), 5);
        assert_eq!(stream.tell().unwrap(), 5);
        stream.flush().unwrap();
    }

    #[test]
    fn plain_writer_has_no_position() {
        let mut stream = WriterOutputStream::new(Vec::new());
        assert_eq!(stream.write(b"abc").unwrap(), 3);
        // Falls back to the module's byte counter at the C boundary.
        assert!(matches!(stream.tell(), Err(StreamError::Unsupported)));
    }

    #[test]
    fn unit_size_round_trips() {
        assert_eq!(
            StreamUnitSize::Bytes8.to_raw(),
            sys::MStream_StreamUnitSize_t_MSTREAM_8BIT
        );
        assert_eq!(
            StreamUnitSize::Utf32.to_raw(),
            sys::MStream_StreamUnitSize_t_MSTREAM_UTF32
        );

        let stream = ReaderInputStream::new(Cursor::new(Vec::new()))
            .with_unit_size(StreamUnitSize::Utf32);
        assert_eq!(stream.unit_size(), StreamUnitSize::Utf32);
    }

    #[test]
    fn panic_message_is_a_single_sentence() {
        // A panic message that is already punctuated must not gain a second
        // period when it reaches `General::strmerr`.
        assert_eq!(end_sentence("Already punctuated."), "Already punctuated.");
        assert_eq!(end_sentence("Not punctuated"), "Not punctuated.");
        assert_eq!(end_sentence("Trailing space "), "Trailing space.");
        assert_eq!(end_sentence("What?"), "What?");

        let result = catch("read", || panic!("The stream failed."));
        match result {
            Err(StreamError::Error(message)) => {
                assert_eq!(message, "Rust panic in stream read: The stream failed.")
            },
            other => panic!("expected a StreamError::Error, got {other:?}"),
        }
    }

    #[test]
    fn catch_converts_panic_to_error() {
        let result = catch("test", || panic!("kaboom"));
        match result {
            Err(StreamError::Error(message)) => {
                assert!(message.contains("kaboom"), "got: {message}");
                assert!(message.contains("test"), "got: {message}");
            },
            other => panic!("expected a StreamError::Error, got {other:?}"),
        }
    }
}
