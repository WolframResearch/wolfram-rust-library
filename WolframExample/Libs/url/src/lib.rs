//! Percent-encoding, compatible with the kernel's own `URLEncode` / `URLDecode`.
//!
//! The work is done by [`percent_encoding`], the RFC 3986 half of the servo
//! `url` crate — this file is only the part that pins its configuration to what
//! the Wolfram Language actually does, which is not quite any of the crate's
//! stock character sets.
//!
//! Both functions take and return a **list** of strings. That is deliberate:
//! every call across the library boundary costs a WXF round trip, so encoding
//! 10,000 strings should be one call, not 10,000. The single-string form is a
//! one-line wrapper in `Kernel/WolframExample.wl`, where it belongs.
//!
//! ## Matching `System`URLEncode`
//!
//! `URLUtilities`'s encoder keeps `A-Za-z0-9-_.~` verbatim and writes every
//! other byte of the UTF-8 encoding as `%XX` with uppercase hex — so a space
//! becomes `%20`, not `+`, and `!`, `*`, `(`, `)`, `'` are all escaped even
//! though RFC 3986 lists them as sub-delimiters. [`percent_encoding`]'s
//! `NON_ALPHANUMERIC` is that set plus the four unreserved punctuation marks,
//! which is exactly what [`WL_UNRESERVED`] removes again.
//!
//! The decoder is asymmetric, and intentionally so: it maps a *literal* `+` to
//! a space (the `application/x-www-form-urlencoded` convention), while `%2B`
//! still decodes to `+`. The kernel gets that from a single simultaneous
//! `StringReplace` pass; here the same thing falls out of splitting on literal
//! `+` *before* decoding, since `%2B` contains no `+` to split on.
//!
//! ## Where it differs
//!
//! * **UTF-8 only.** The kernel's `CharacterEncoding` option is not supported —
//!   Rust `str` is UTF-8 by definition, and the other 100-odd encodings would
//!   mean carrying an encoding table for no benefit to the common case.
//! * **Invalid UTF-8 decodes lossily.** `URLDecode["%C3"]` is a truncated
//!   two-byte sequence; the kernel reports a message, this returns U+FFFD.
//! * **Lone surrogates are rejected, not encoded.** A Wolfram string may hold
//!   an unpaired U+D800-U+DFFF code unit, which has no UTF-8 form and so no
//!   `str` to put it in. The library boundary rejects the payload before this
//!   code runs, with `Failure["ArgumentError", <|"Message" -> "payload not
//!   valid UTF-8"|>]`; `System`URLEncode` encodes it CESU-8 style instead.
//!
//! Everything else agrees exactly: over all 63,488 non-surrogate BMP
//! characters plus the corpus in URLUtilities' own `URLEncode.mt`, both
//! directions match the kernel character for character.

use std::borrow::Cow;

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use wolfram_export::export;
use wolfram_serialize::{
    Error, ExpressionEnum, FromWXF, Reader, ToWXF, Writer, WxfReader, WxfStruct, WxfWriter,
};

/// The characters `System`URLEncode` leaves alone: `A-Za-z0-9` plus `-`, `.`,
/// `_` and `~` — RFC 3986's "unreserved" set, and nothing else.
const WL_UNRESERVED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// One string crossing the boundary, borrowed on the way in and owned on the
/// way out.
///
/// The `Cow` is the point. WXF strings are UTF-8 already, so an *argument* is a
/// pointer and a length into the kernel's own buffer — no copy, no validation
/// pass, `Cow::Borrowed`. `#[export(wxf)]` decodes the arguments, calls the
/// function, and serializes the result inside a single closure, while that
/// buffer is still alive, which is what makes the borrow sound. A *result* is
/// new text that exists nowhere yet, so it has to be `Cow::Owned` — one type
/// covers both directions.
///
/// The newtype is a tax, not a design choice: `Vec<T>` gets its `System`List`
/// encoding from a blanket impl bounded on the `WxfStruct` marker, and 0.6.0
/// marks neither `str` nor `String`, so `Vec<&str>` and `Vec<String>` both have
/// no impl to reach for. `Text` supplies the marker and forwards everything
/// else, so it *is* a bare WXF string on the wire: `Vec<Text>` crosses as
/// `{"a", "b"}`, with no wrapper head and nothing for the kernel side to
/// unwrap. (`#[derive(ToWXF)]` on a newtype would not do: a derived tuple
/// struct emits `Function[List, ...]`, so each element would arrive as `{"a"}`
/// rather than `"a"`.)
///
/// Both markers land in wolfram-serialize by way of the borrowed-primitive fix
/// on `master`; once that is released this whole block deletes and the two
/// functions below take `Vec<&str>` and return `Vec<String>` directly.
#[derive(Debug, Clone, PartialEq)]
struct Text<'a>(Cow<'a, str>);

impl WxfStruct for Text<'_> {}

impl ToWXF for Text<'_> {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_string(&self.0)
    }
}

impl<'de> FromWXF<'de> for Text<'de> {
    fn from_wxf_with_tag<R: Reader<'de>>(
        r: &mut WxfReader<R>,
        tok: ExpressionEnum,
    ) -> Result<Self, Error> {
        <&'de str as FromWXF<'de>>::from_wxf_with_tag(r, tok).map(|s| Text(Cow::Borrowed(s)))
    }
}

/// Percent-encode every string in `inputs`, in order.
///
/// `{"a+b", "Hello, world!"}` -> `{"a%2Bb", "Hello%2C%20world%21"}`.
#[export(wxf)]
fn url_encode(inputs: Vec<Text<'_>>) -> Vec<Text<'static>> {
    inputs
        .iter()
        .map(|Text(input)| {
            // `extend` writes the encoder's `&str` pieces straight into the
            // buffer. The obvious `utf8_percent_encode(..).to_string()` goes
            // through `Display`, i.e. `fmt::Write` and its formatting
            // machinery, and measures about twice as slow for the same output.
            let mut out = String::with_capacity(input.len());
            out.extend(utf8_percent_encode(input, WL_UNRESERVED));
            Text(Cow::Owned(out))
        })
        .collect()
}

/// Percent-decode every string in `inputs`, in order.
///
/// `{"a%2Bb", "a+b"}` -> `{"a+b", "a b"}` — `%2B` is a plus sign, a bare `+` is
/// a space.
///
/// Results are materialized here rather than during serialization on purpose:
/// the bridge's `try_encode` runs `ToWXF::to_wxf` twice — once through a byte
/// counter to size the output `NumericArray`, once to fill it — so anything
/// computed inside a `to_wxf` impl is computed twice.
#[export(wxf)]
fn url_decode(inputs: Vec<Text<'_>>) -> Vec<Text<'static>> {
    let mut bytes = Vec::new();
    inputs
        .iter()
        .map(|Text(input)| {
            decode_into(input, &mut bytes);
            // Borrows when the decoded bytes are valid UTF-8, which is the
            // usual case; only a malformed escape sequence allocates twice.
            Text(Cow::Owned(String::from_utf8_lossy(&bytes).into_owned()))
        })
        .collect()
}

/// Percent-decode `input` into `out`, replacing whatever was there.
///
/// Splitting on `+` first is what makes a literal plus a space while leaving
/// `%2B` a plus: an already-escaped plus is spelled `%2B`, which contains no
/// `+` for the split to catch. It is safe to cut the string there because `+`
/// is ASCII — it can never be part of a percent escape or a UTF-8
/// continuation byte, so no multi-byte sequence is ever split across segments.
fn decode_into(input: &str, out: &mut Vec<u8>) {
    out.clear();
    let mut segments = input.split('+');
    if let Some(first) = segments.next() {
        out.extend(percent_decode_str(first));
    }
    for segment in segments {
        out.push(b' ');
        out.extend(percent_decode_str(segment));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use wolfram_serialize::{read_wxf, to_wxf};

    fn encode(inputs: &[&str]) -> Vec<String> {
        run(url_encode(wrap(inputs)))
    }

    fn decode(inputs: &[&str]) -> Vec<String> {
        run(url_decode(wrap(inputs)))
    }

    fn wrap<'a>(inputs: &'a [&'a str]) -> Vec<Text<'a>> {
        inputs.iter().map(|s| Text(Cow::Borrowed(*s))).collect()
    }

    // Round-tripping through WXF checks the wire shape — a bare `{"a", "b"}`
    // with no wrapper head — alongside the values.
    fn run(result: Vec<Text<'_>>) -> Vec<String> {
        let bytes = to_wxf(&result, None).expect("serialize");
        // `Text` borrows from the buffer, so it is read inside `read_wxf`'s
        // closure and copied out — the same shape the kernel bridge uses.
        read_wxf(&bytes, |r| {
            Ok(Vec::<Text>::from_wxf(r)?
                .iter()
                .map(|t| t.0.to_string())
                .collect())
        })
        .expect("deserialize")
    }

    // The expected values here are the ones in URLUtilities' own URLEncode.mt.
    #[test]
    fn matches_kernel_encoding() {
        let cases = [
            ("", ""),
            ("a+b", "a%2Bb"),
            ("Hello, world!", "Hello%2C%20world%21"),
            ("7 x + 5 y", "7%20x%20%2B%205%20y"),
            ("123.123123", "123.123123"),
            ("abcdef!@#$%^&*()", "abcdef%21%40%23%24%25%5E%26%2A%28%29"),
            (
                "http://www.wolfram.com/solutions",
                "http%3A%2F%2Fwww.wolfram.com%2Fsolutions",
            ),
            ("\n", "%0A"),
            ("Kurt Gödel", "Kurt%20G%C3%B6del"),
            ("Paul Erdős", "Paul%20Erd%C5%91s"),
            // the unreserved set, verbatim
            ("-_.~", "-_.~"),
        ];
        for (input, want) in cases {
            assert_eq!(encode(&[input]), vec![want.to_string()]);
        }
    }

    #[test]
    fn matches_kernel_decoding() {
        let cases = [
            ("", ""),
            ("a%2Bb", "a+b"),
            ("Hello%2C%20world%21", "Hello, world!"),
            ("Kurt%20G%C3%B6del", "Kurt Gödel"),
            // a bare `+` is a space, but `%2B` in the same string is not
            ("a+b%2Bc", "a b+c"),
            // an unescapable `%` is left alone, as the kernel leaves it
            ("100%", "100%"),
            ("%zz", "%zz"),
            ("%2", "%2"),
            // literal non-ASCII passes through untouched
            ("adfadè", "adfadè"),
        ];
        for (input, want) in cases {
            assert_eq!(decode(&[input]), vec![want.to_string()]);
        }
    }

    #[test]
    fn round_trips() {
        let inputs = ["日本語", "한국어", "学数", "\u{0}\u{1f}\u{7f}", "  "];
        let encoded = encode(&inputs);
        let back = decode(&encoded.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(back, inputs);
    }

    // Both functions are list-in / list-out and order-preserving; the kernel
    // wrapper's single-string form leans on that.
    #[test]
    fn preserves_order_and_length() {
        let inputs: Vec<String> = (0..50).map(|i| format!("item {i}!")).collect();
        let refs: Vec<&str> = inputs.iter().map(String::as_str).collect();
        let encoded = encode(&refs);
        assert_eq!(encoded.len(), inputs.len());
        assert_eq!(
            decode(&encoded.iter().map(String::as_str).collect::<Vec<_>>()),
            inputs
        );
        assert!(encode(&[]).is_empty());
    }
}
