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
