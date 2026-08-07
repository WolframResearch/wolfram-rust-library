use wolfram_serialize::{from_wxf};

#[test]
fn boolean_symbol_without_context_deserializes() {
    let bytes_true: Vec<u8> = vec![56, 58, 115, 4, 84, 114, 117, 101]; // Normal@BinarySerialize[True]
    let bytes_false: Vec<u8> = vec![56, 58, 115, 5, 70, 97, 108, 115, 101]; // Normal@BinarySerialize[False]
    assert_eq!(from_wxf::<bool>(&bytes_true).unwrap(), true);
    assert_eq!(from_wxf::<bool>(&bytes_false).unwrap(), false);
}

#[test]
fn boolean_symbol_with_context_deserializes() {
    // WXF bytes manually edited to include symbol context
    let bytes_true: Vec<u8> = vec![56, 58, 115, 11, 83, 121, 115, 116, 101, 109, 96, 84, 114, 117, 101];
    let bytes_false: Vec<u8> = vec![56, 58, 115, 12, 83, 121, 115, 116, 101, 109, 96, 70, 97, 108, 115, 101];
    assert_eq!(from_wxf::<bool>(&bytes_true).unwrap(), true);
    assert_eq!(from_wxf::<bool>(&bytes_false).unwrap(), false);
}