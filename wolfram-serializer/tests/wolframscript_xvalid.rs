//! Cross-validation against `wolframscript`. For each WXF type:
//!
//! 1. Have wolframscript `BinarySerialize` an expression. We import → re-export →
//!    wolframscript `BinaryDeserialize`s our bytes → `InputForm` matches the original.
//! 2. (Optionally for atoms:) check that the round-trip preserves semantic equality.
//!
//! Tests early-exit (pass) if `wolframscript` is not installed.

use std::process::Command;

use wolfram_expr::{Association, ByteArray, Expr, NumericArray, RuleEntry, Symbol};
use wolfram_serializer::{export, import, Format};

fn wolframscript_available() -> bool {
    Command::new("wolframscript")
        .arg("-h")
        .output()
        .map(|o| o.status.success() || !o.stdout.is_empty())
        .unwrap_or(false)
}

/// Have wolframscript serialize `wl_code` to WXF bytes; return them.
fn wl_to_wxf(wl_code: &str) -> Vec<u8> {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("wxf_xvalid_{}_{}.bin", pid, nanos));
    let script = format!(
        r#"BinaryWrite["{}", Normal @ BinarySerialize[{}]]; Close["{}"];"#,
        path.display(),
        wl_code,
        path.display()
    );
    let out = Command::new("wolframscript")
        .args(["-code", &script])
        .output()
        .expect("invoke wolframscript");
    assert!(
        out.status.success(),
        "wolframscript failed: stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = std::fs::read(&path).expect("read wolframscript output");
    let _ = std::fs::remove_file(&path);
    bytes
}

/// Take WXF bytes, hand to wolframscript, get back `InputForm` of the resulting
/// expression. Uses `Exit[]` to suppress the wolframscript REPL printing the final
/// expression's value.
fn wxf_to_wl_inputform(bytes: &[u8]) -> String {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("wxf_xvalid_in_{}_{}.bin", pid, nanos));
    std::fs::write(&path, bytes).expect("write wxf bytes");
    let script = format!(
        r#"WriteString[$Output, ToString[BinaryDeserialize[ReadByteArray["{}"]], InputForm]]; Exit[]"#,
        path.display()
    );
    let out = Command::new("wolframscript")
        .args(["-code", &script])
        .output()
        .expect("invoke wolframscript");
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "wolframscript failed: stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// True round-trip via WXF: WL → wolframscript serialize → our deserialize → our
/// serialize → wolframscript deserialize → InputForm. Equality of InputForm with
/// the original `wl_repr` proves both directions of WXF preserve the expression.
fn rt_via_inputform(wl_repr: &str) {
    let from_wl = wl_to_wxf(wl_repr);
    let parsed: Expr = import(&from_wl, Format::Wxf).expect("import wolfram-produced WXF");
    let bytes_from_rust = export(&parsed, Format::Wxf).expect("export Wxf");
    let echoed = wxf_to_wl_inputform(&bytes_from_rust);
    assert_eq!(echoed, wl_repr, "round-trip InputForm mismatch");
}

#[test]
fn integer_rt() {
    if !wolframscript_available() {
        return;
    }
    rt_via_inputform("42");
    rt_via_inputform("-1234567890");
}

#[test]
fn real_rt() {
    if !wolframscript_available() {
        return;
    }
    rt_via_inputform("3.5");
}

#[test]
fn string_rt() {
    if !wolframscript_available() {
        return;
    }
    rt_via_inputform(r#""hello""#);
}

#[test]
fn symbol_rt() {
    if !wolframscript_available() {
        return;
    }
    // System` symbols come over the wire with the System` context stripped — they
    // print as the bare name. Round-trip preserves that.
    rt_via_inputform("Plus");
    // Use a non-default context — wolframscript's InputForm strips Global` (since
    // it's in $ContextPath) but preserves user package contexts.
    rt_via_inputform("MyPkg`x");
}

#[test]
fn list_rt() {
    if !wolframscript_available() {
        return;
    }
    rt_via_inputform("{1, 2, 3}");
    rt_via_inputform("{}");
    rt_via_inputform(r#"{"a", 1, 2.5}"#);
}

#[test]
fn function_unevaluated_rt() {
    if !wolframscript_available() {
        return;
    }
    // Use MyPkg` context so wolframscript's InputForm doesn't strip the prefix.
    rt_via_inputform("MyPkg`myFunc[1, 2, 3]");
}

#[test]
fn association_rt() {
    if !wolframscript_available() {
        return;
    }
    rt_via_inputform(r#"<|"a" -> 1, "b" -> 2|>"#);
}

#[test]
fn association_with_delayed_rt() {
    if !wolframscript_available() {
        return;
    }
    rt_via_inputform(r#"<|"a" -> 1, "b" :> 2|>"#);
}

#[test]
fn byte_array_semantic() {
    // ByteArray's WL InputForm uses BaseEncoding (not the original byte values),
    // so we can't compare InputForm strings directly. Instead, verify semantic
    // equivalence: our deserialize of WL's output reproduces our original bytes.
    if !wolframscript_available() {
        return;
    }
    let original = Expr::from(ByteArray::from(vec![0u8, 1, 2, 0xff, 0x80]));
    let bytes_from_wl = wl_to_wxf("ByteArray[{0, 1, 2, 255, 128}]");
    let parsed = import(&bytes_from_wl, Format::Wxf).unwrap();
    assert_eq!(parsed, original);

    // Reverse: our WXF deserializes correctly through wolframscript.
    let bytes_from_rust = export(&original, Format::Wxf).unwrap();
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("ba_check_{}.bin", pid));
    std::fs::write(&path, &bytes_from_rust).unwrap();
    let script = format!(
        r#"WriteString[$Output, ToString[Normal[BinaryDeserialize[ReadByteArray["{}"]]], InputForm]]; Exit[]"#,
        path.display()
    );
    let out = Command::new("wolframscript")
        .args(["-code", &script])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&path);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim(), "{0, 1, 2, 255, 128}");
}

#[test]
fn numeric_array_semantic() {
    if !wolframscript_available() {
        return;
    }
    let arr_expr = Expr::from(NumericArray::from_slice::<i32>(vec![3], &[10, 20, 30]));

    // WL → us:
    let from_wl = wl_to_wxf(r#"NumericArray[{10, 20, 30}, "Integer32"]"#);
    let parsed = import(&from_wl, Format::Wxf).unwrap();
    assert_eq!(parsed, arr_expr);

    // Us → WL: verify head + element type + values.
    let bytes_from_rust = export(&arr_expr, Format::Wxf).unwrap();
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("na_check_{}.bin", pid));
    std::fs::write(&path, &bytes_from_rust).unwrap();
    let script = format!(
        r#"
        na = BinaryDeserialize[ReadByteArray["{}"]];
        WriteString[$Output, ToString[Head[na]], "|", ToString[NumericArrayType[na]], "|", ToString[Normal[na], InputForm]]; Exit[]"#,
        path.display()
    );
    let out = Command::new("wolframscript")
        .args(["-code", &script])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&path);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim(), "NumericArray|Integer32|{10, 20, 30}");
}

#[test]
fn import_struct_check() {
    // Sanity: even after the wire round-trip, an Association we constructed in Rust
    // is equal-by-value to the same Association built from a wolframscript-produced
    // WXF. This exercises the BTreeMap-based equality + RuleEntry handling.
    if !wolframscript_available() {
        return;
    }
    let mut a = Association::new();
    a.insert(Expr::from("a"), RuleEntry::rule(Expr::from(1)));
    a.insert(Expr::from("b"), RuleEntry::rule_delayed(Expr::from(2)));
    let expected = Expr::from(a);

    let bytes_from_wl = wl_to_wxf(r#"<|"a" -> 1, "b" :> 2|>"#);
    let parsed = import(&bytes_from_wl, Format::Wxf).unwrap();
    assert_eq!(parsed, expected);
}

#[test]
fn compressed_xvalid() {
    // Wolfram serializes a compressible expression with PerformanceGoal -> "Size"
    // (which produces an `8C:` header), and we deserialize it transparently.
    if !wolframscript_available() {
        return;
    }
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("wxf_compressed_{}.bin", pid));
    let script = format!(
        r#"BinaryWrite["{}", Normal @ BinarySerialize[Range[100], PerformanceGoal -> "Size"]]; Close["{}"];"#,
        path.display(),
        path.display()
    );
    let out = Command::new("wolframscript")
        .args(["-code", &script])
        .output()
        .unwrap();
    assert!(out.status.success(), "wolframscript failed");
    let bytes = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(&bytes[..3], b"8C:", "Wolfram should produce a compressed header");

    // We deserialize it. Wolfram size-optimizes Range[100] into a PackedArray
    // of Integer8, so what we get back is `PackedArray[…, Integer8]` containing
    // bytes 1..=100 — proving both decompression *and* PackedArray decoding work.
    let parsed: Expr = wolfram_serializer::import(&bytes, Format::Wxf)
        .expect("import compressed WXF from wolfram");
    let arr = parsed
        .try_as_packed_array()
        .expect("expected PackedArray from Range[100]");
    assert_eq!(arr.dimensions(), &[100]);
    assert_eq!(
        arr.try_as_slice::<i8>(),
        Some((1..=100i8).collect::<Vec<_>>().as_slice())
    );
}

// Suppress unused-import warning when wolframscript is not available
#[allow(dead_code)]
fn _unused() {
    let _ = Symbol::new;
}
