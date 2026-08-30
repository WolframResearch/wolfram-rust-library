//! Procedural macros for the `wolfram-library-link` I/O streams API.
//!
//! Provides `#[derive(InputStream)]`, `#[derive(SeekableInputStream)]` and
//! `#[derive(OutputStream)]`, each of which forwards the Wolfram stream traits
//! to a type's existing [`std::io::Read`], [`std::io::Write`] and
//! [`std::io::Seek`] implementations. They are the counterpart to
//! `ReaderInputStream` / `WriterOutputStream`, which do the same job for types a
//! library *doesn't* own and therefore cannot derive on.
//!
//! # When deriving is worth it
//!
//! These derive nothing you could not write by hand — the emitted code is
//! exactly the forwarding you would write yourself — so how much they buy varies
//! by trait:
//!
//! * [`SeekableInputStream`][macro@SeekableInputStream] is the strong case. It
//!   replaces two `impl` blocks and five methods, and it gets right two things
//!   that are easy to miss: that `is_seekable()` must return `true` (the Wolfram
//!   Language consults *that*, not whether `seek` exists), and that `size()`
//!   must restore the stream position it moved to measure.
//!
//! * [`OutputStream`][macro@OutputStream] replaces three short methods.
//!
//! * [`InputStream`][macro@InputStream] replaces a single forwarding `read`.
//!   Writing that by hand is barely longer than the `#[derive(..)]`; the derive
//!   is mostly there so a type that gains seeking later, or sits alongside a
//!   derived sibling, reads consistently.
//!
//! # Deriving is all or nothing
//!
//! A derive emits a whole `impl` of the trait, so you cannot derive and then
//! override one method — two `impl`s of the same trait for the same type
//! collide. A stream that needs custom behavior anywhere (a real
//! `wait_for_input`, a non-default `unit_size`, error messages of its own) has
//! to implement the trait by hand, in full.
//!
//! This is also why [`InputStream`][macro@InputStream] and
//! [`SeekableInputStream`][macro@SeekableInputStream] are alternatives rather
//! than companions: both emit `impl InputStream`.
//!
//! See the `wolfram-library-link` crate docs for the stream traits themselves
//! and the kernel behavior an implementation should know about.

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod stream;

/// Derive `InputStream` for a type that already implements [`std::io::Read`].
///
/// The generated stream is **not** seekable. For a type that also implements
/// [`std::io::Seek`], use [`SeekableInputStream`][macro@SeekableInputStream]
/// instead; the two are alternatives, not companions.
///
/// To adapt a type you *don't* own — a [`File`][std::fs::File], a
/// [`TcpStream`][std::net::TcpStream] — use `ReaderInputStream`, which takes
/// any reader by value.
///
/// # Whether to bother
///
/// This derive replaces one method, so the hand-written equivalent is barely
/// longer:
///
/// ```ignore
/// impl InputStream for Decoder {
///     fn read(&mut self, buf: &mut [u8]) -> Result<usize, StreamError> {
///         Ok(std::io::Read::read(self, buf)?)
///     }
/// }
/// ```
///
/// Prefer the hand-written form as soon as you want anything the derive does not
/// give you — a `wait_for_input`, a non-default `unit_size`, or error messages
/// better than `std::io::Error`'s own (`?` converts through
/// `From<std::io::Error>`, which has no context to add). You cannot derive and
/// then override.
///
/// # Example
///
/// ```
/// use std::io::{Cursor, Read};
///
/// use wolfram_library_link::stream::InputStream;
///
/// #[derive(InputStream)]
/// struct Decoder(Cursor<Vec<u8>>);
///
/// impl Read for Decoder {
///     fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
///         self.0.read(buf)
///     }
/// }
///
/// let mut decoder = Decoder(Cursor::new(b"hello".to_vec()));
/// let mut buf = [0u8; 5];
///
/// assert_eq!(InputStream::read(&mut decoder, &mut buf).unwrap(), 5);
/// assert_eq!(&buf, b"hello");
/// // Not seekable, so the Wolfram Language will not try to reposition it.
/// assert!(!InputStream::is_seekable(&decoder));
/// ```
#[proc_macro_derive(InputStream)]
pub fn derive_input_stream(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    stream::expand(&input, stream::Kind::Input).into()
}

/// Derive `InputStream` and `SeekableInputStream` for a type that implements
/// both [`std::io::Read`] and [`std::io::Seek`].
///
/// This is the derive most worth using. As well as reading, it wires up `seek`,
/// `tell` and `size` across two `impl` blocks, and handles two things that are
/// easy to get wrong by hand:
///
/// * **`is_seekable()` must report `true`.** The Wolfram Language consults it,
///   not the presence of a `seek` implementation, before repositioning a stream.
///   Implement `seek` but forget `is_seekable` and the stream is silently
///   treated as non-seekable — no error, it simply never moves.
///
/// * **`size()` must restore the position.** Measuring the stream means seeking
///   to the end, so a naive implementation quietly moves the read cursor.
///
/// This is an alternative to [`InputStream`][macro@InputStream], not an
/// addition — derive one or the other, never both, and hand-write the impl if
/// you need to customize any part of it.
///
/// The Wolfram Language calls `seek` with an absolute position of its own
/// choosing (it buffers input and seeks to a buffer boundary), so the generated
/// `seek` treats its argument as absolute.
///
/// # Example
///
/// ```
/// use std::io::{Cursor, Read, Seek, SeekFrom};
///
/// use wolfram_library_link::stream::{InputStream, SeekableInputStream};
///
/// #[derive(SeekableInputStream)]
/// struct Archive(Cursor<Vec<u8>>);
///
/// impl Read for Archive {
///     fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
///         self.0.read(buf)
///     }
/// }
///
/// impl Seek for Archive {
///     fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
///         self.0.seek(pos)
///     }
/// }
///
/// fn assert_seekable<S: SeekableInputStream>(_: &S) {}
///
/// let mut archive = Archive(Cursor::new(b"hello world".to_vec()));
/// assert_seekable(&archive);
///
/// assert!(InputStream::is_seekable(&archive));
/// assert_eq!(InputStream::size(&mut archive).unwrap(), 11);
/// // Measuring the size left the position alone.
/// assert_eq!(InputStream::tell(&mut archive).unwrap(), 0);
///
/// InputStream::seek(&mut archive, 6).unwrap();
/// assert_eq!(InputStream::tell(&mut archive).unwrap(), 6);
///
/// let mut buf = [0u8; 5];
/// InputStream::read(&mut archive, &mut buf).unwrap();
/// assert_eq!(&buf, b"world");
/// ```
#[proc_macro_derive(SeekableInputStream)]
pub fn derive_seekable_input_stream(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    stream::expand(&input, stream::Kind::SeekableInput).into()
}

/// Derive `OutputStream` for a type that already implements [`std::io::Write`].
///
/// Replaces three methods: `write` and `flush` forward to their
/// [`std::io::Write`] counterparts, and `close` flushes.
///
/// To adapt a type you *don't* own, use `WriterOutputStream` instead. Hand-write
/// the impl if the stream needs to report write failures through
/// `report_error` — the Wolfram Language gives output streams no error channel
/// of their own, so that hook is the only way to make a failed write visible,
/// and a derive cannot supply it.
///
/// # Example
///
/// ```
/// use std::io::Write;
///
/// use wolfram_library_link::stream::OutputStream;
///
/// #[derive(OutputStream)]
/// struct Encoder(Vec<u8>);
///
/// impl Write for Encoder {
///     fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
///         self.0.write(buf)
///     }
///
///     fn flush(&mut self) -> std::io::Result<()> {
///         self.0.flush()
///     }
/// }
///
/// let mut encoder = Encoder(Vec::new());
///
/// assert_eq!(OutputStream::write(&mut encoder, b"hello").unwrap(), 5);
/// OutputStream::flush(&mut encoder).unwrap();
/// assert_eq!(encoder.0, b"hello");
/// ```
#[proc_macro_derive(OutputStream)]
pub fn derive_output_stream(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    stream::expand(&input, stream::Kind::Output).into()
}
