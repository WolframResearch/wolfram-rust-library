//! Zero-copy borrowed deserialization: `&str` / `&[u8]` fields point straight
//! into the input buffer.

use wolfram_serialize::{read_wxf, to_wxf, FromWXF, ToWXF};

#[derive(Debug, PartialEq, ToWXF, FromWXF)]
struct Frame<'a> {
    name: &'a str,
    payload: &'a [u8],
    count: i64,
}

#[test]
fn borrowed_struct_roundtrips_zero_copy() {
    let bytes = to_wxf(
        &Frame {
            name: "hello",
            payload: &[1u8, 2, 3, 0xff],
            count: 42,
        },
        None,
    )
    .unwrap();

    // The borrow is tied to `bytes`, so read and assert inside the closure
    // rather than letting `Frame` escape it.
    read_wxf(&bytes, |r| {
        let frame = Frame::from_wxf(r)?;
        assert_eq!(frame.name, "hello");
        assert_eq!(frame.payload, &[1u8, 2, 3, 0xff]);
        assert_eq!(frame.count, 42);

        // Zero-copy: the &str/&[u8] point *inside* `bytes`, not into a fresh alloc.
        let buf_range = bytes.as_ptr_range();
        let name_ptr = frame.name.as_ptr();
        let payload_ptr = frame.payload.as_ptr();
        assert!(
            buf_range.start <= name_ptr && name_ptr < buf_range.end,
            "name should borrow the input buffer"
        );
        assert!(
            buf_range.start <= payload_ptr && payload_ptr < buf_range.end,
            "payload should borrow the input buffer"
        );
        Ok(())
    })
    .unwrap();
}

// A borrowed struct nested inside a tuple-struct argument list still works.
#[derive(Debug, PartialEq, ToWXF, FromWXF)]
struct Pair<'a>(&'a str, &'a str);

#[test]
fn borrowed_tuple_struct() {
    let bytes = to_wxf(&Pair("a", "bcd"), None).unwrap();
    read_wxf(&bytes, |r| {
        let p = Pair::from_wxf(r)?;
        assert_eq!(p, Pair("a", "bcd"));
        Ok(())
    })
    .unwrap();
}

// Borrowed primitives inside containers: `Vec<&str>`, `Option<&str>`, tuples.
// These route through the generic `T: FromWXF<'de>` bounds rather than the
// derive's inline `&str` / `&[u8]` special cases, so they only compile because
// the borrowed impls are `'de: 'a` rather than `&'de`-exact.
#[derive(Debug, PartialEq, ToWXF, FromWXF)]
struct Batch<'a> {
    names: Vec<&'a str>,
    chunks: Vec<&'a [u8]>,
    tag: Option<&'a str>,
    pair: (&'a str, &'a [u8]),
}

#[test]
fn borrowed_containers_are_zero_copy() {
    let bytes = to_wxf(
        &Batch {
            names: vec!["alpha", "beta"],
            chunks: vec![&[1u8, 2][..], &[3u8][..]],
            tag: Some("gamma"),
            pair: ("delta", &[4u8, 5]),
        },
        None,
    )
    .unwrap();

    read_wxf(&bytes, |r| {
        let b = Batch::from_wxf(r)?;
        assert_eq!(b.names, vec!["alpha", "beta"]);
        assert_eq!(b.chunks, vec![&[1u8, 2][..], &[3u8][..]]);
        assert_eq!(b.tag, Some("gamma"));
        assert_eq!(b.pair, ("delta", &[4u8, 5][..]));

        // Every element points inside `bytes` — no per-string allocation.
        let range = bytes.as_ptr_range();
        let inside = |p: *const u8| range.start <= p && p < range.end;
        for s in &b.names {
            assert!(inside(s.as_ptr()), "Vec<&str> element should borrow");
        }
        for c in &b.chunks {
            assert!(inside(c.as_ptr()), "Vec<&[u8]> element should borrow");
        }
        assert!(inside(b.tag.unwrap().as_ptr()));
        Ok(())
    })
    .unwrap();
}

// A bare `Vec<&str>` / `Vec<&[u8]>` as the whole payload — needs the `WxfStruct`
// marker on the borrowed types so the blanket List impls apply.
#[test]
fn top_level_vec_of_borrowed() {
    let bytes = to_wxf(&vec!["x", "y", "z"], None).unwrap();
    read_wxf(&bytes, |r| {
        assert_eq!(Vec::<&str>::from_wxf(r)?, vec!["x", "y", "z"]);
        Ok(())
    })
    .unwrap();
}
