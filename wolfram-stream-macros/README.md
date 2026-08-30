# wolfram-stream-macros

Derive macros for the [`wolfram-library-link`](https://crates.io/crates/wolfram-library-link)
I/O streams API:

| Derive | Replaces | Requires |
|--------|----------|----------|
| `#[derive(SeekableInputStream)]` | two `impl`s, five methods | `std::io::Read + std::io::Seek` |
| `#[derive(OutputStream)]` | three methods | `std::io::Write` |
| `#[derive(InputStream)]` | one method | `std::io::Read` |

Each forwards the Wolfram stream traits to a type's existing `std::io`
implementations, emitting exactly the code you would otherwise write by hand.

`SeekableInputStream` is the one most worth using: besides the boilerplate, it
gets right two things that are easy to miss — that `is_seekable()` must report
`true` (the Wolfram Language consults that, not the presence of a `seek`
implementation) and that `size()` must restore the position it moved to measure.
`InputStream` replaces a single forwarding `read`, so hand-writing it is barely
longer.

Deriving is all or nothing: a derive emits a whole `impl`, so a stream that needs
custom behavior anywhere has to implement the trait by hand, in full. That is
also why `InputStream` and `SeekableInputStream` are alternatives rather than
companions.

For a type you *don't* own — a `File`, a `TcpStream` — use
`ReaderInputStream` / `WriterOutputStream` instead, which take any reader or
writer by value.

This crate is not used directly. The macros are re-exported from
`wolfram_library_link::stream`, alongside the traits they implement.
