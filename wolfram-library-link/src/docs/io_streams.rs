/*!
# How To: Provide a custom Wolfram Language stream

A library can register a named *stream method*, which the Wolfram Language then
opens streams with. Reads and writes on such a stream are serviced by Rust, so
anything a Rust type can read from or write to — a socket, an archive entry, a
decompressor, a remote object — can be used with
[`ReadString`][ref/ReadString], [`Import`][ref/Import],
[`WriteString`][ref/WriteString] and the rest of the Wolfram Language's stream
functions.

[ref/ReadString]: https://reference.wolfram.com/language/ref/ReadString.html
[ref/Import]: https://reference.wolfram.com/language/ref/Import.html
[ref/WriteString]: https://reference.wolfram.com/language/ref/WriteString.html

Registering a method takes two pieces: an [`InputStreamMethod`], which the
Wolfram Language calls to open a stream, and an [`InputStream`], which services
that one stream until it is closed. [`ReaderInputStream`] supplies the second
from any [`std::io::Read`], so the method is often all you write.

[`InputStreamMethod`]: crate::stream::InputStreamMethod
[`InputStream`]: crate::stream::InputStream
[`ReaderInputStream`]: crate::stream::ReaderInputStream

**Rust**

```rust
# mod scope {
*/
#![doc = include_str!("../../examples/docs/io_streams/echo_stream.rs")]
/*!
# }
```

**Wolfram**

```wolfram
*/
#![doc = include_str!("../../RustLink/Examples/Docs/IOStreams/EchoStream.wlt")]
/*!
```

Output streams work the same way, with [`OutputStreamMethod`] and
[`OutputStream`], and [`WriterOutputStream`] adapting any [`std::io::Write`].

[`OutputStreamMethod`]: crate::stream::OutputStreamMethod
[`OutputStream`]: crate::stream::OutputStream
[`WriterOutputStream`]: crate::stream::WriterOutputStream

## Using a type you own

[`ReaderInputStream`] takes a reader by value, which is what you want for a
type from another crate. For a type of your own, derive the trait instead and
skip the wrapper:

```rust
# mod scope {
use wolfram_library_link::stream::SeekableInputStream;

#[derive(SeekableInputStream)]
struct Archive {
    // ...
#     _private: (),
}
# impl std::io::Read for Archive {
#     fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> { Ok(0) }
# }
# impl std::io::Seek for Archive {
#     fn seek(&mut self, _: std::io::SeekFrom) -> std::io::Result<u64> { Ok(0) }
# }
# }
```

`#[derive(SeekableInputStream)]` forwards to the type's [`std::io::Read`] and
[`std::io::Seek`] impls and reports the stream as seekable, so
[`SetStreamPosition`][ref/SetStreamPosition] works on it. Use
`#[derive(InputStream)]` for a reader that cannot seek, and
`#[derive(OutputStream)]` for a writer — the two input derives are alternatives,
not companions.

[ref/SetStreamPosition]: https://reference.wolfram.com/language/ref/SetStreamPosition.html

## Reporting errors

Returning [`StreamError`] from a read makes the Wolfram Language issue
[`General::strmerr`][ref/message/strmerr] with your message inserted verbatim:

```text
Read::strmerr: Error on stream my-stream. Error message: The connection was reset.
```

so write messages in sentence case, with a period after each sentence. Since
[`From<std::io::Error>`][std::io::Error] is implemented, `?` works directly on
[`std::io`] operations.

Note that **the Wolfram Language has no equivalent error channel for output
streams**: a failed write produces only a generic message from
[`BinaryWrite`][ref/BinaryWrite], and nothing at all from `WriteString` or
`Write`. To report a write failure, raise a message from
[`OutputStream::report_error`].

[`StreamError`]: crate::stream::StreamError
[`OutputStream::report_error`]: crate::stream::OutputStream::report_error
[ref/message/strmerr]: https://reference.wolfram.com/language/ref/message/General/strmerr.html
[ref/BinaryWrite]: https://reference.wolfram.com/language/ref/BinaryWrite.html

## Streams that are not always ready

A stream backed by something that can be slow — a socket, a subprocess — should
return [`StreamError::WouldBlock`] rather than blocking inside
[`read`][crate::stream::InputStream::read]. The Wolfram Language responds by
calling [`wait_for_input`][crate::stream::InputStream::wait_for_input] and
reading again, which keeps the read interruptible. Waiting a short, bounded time
there and returning is correct; an implementation that does block should poll
[`aborted()`][crate::aborted] so the user can still abort.

[`StreamError::WouldBlock`]: crate::stream::StreamError::WouldBlock

## Related links

* [`wolfram_library_link::stream`][crate::stream], which documents the rest of
  the kernel behavior a stream implementation should know about.
*/
