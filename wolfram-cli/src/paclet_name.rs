//! Paclet name sanitizing, ported from the existing Wolfram tooling.
//!
//! `CreatePacletArchive` derives its output filename from the paclet's
//! *qualified name*, computed from the `Name`/`Qualifier`/`Version` fields of
//! `PacletInfo.m`. That logic already exists in two places which must be kept
//! in sync — Java's `Utils.pacletNameToQualifiedName` (used by
//! `PacletPacker.pack()`) and WL's `PgetQualifiedName` (`Paclet.m`) — and this
//! module is a third port of it, so names generated here match the ones the
//! existing tooling produces for the same paclet.
//!
//! The only transforms are, in order: `/` → `__`, then form-urlencode, then
//! `-`-join. Deliberately absent is any *additional* sanitizing (stripping
//! `..`, blocklisting characters, trimming, case-folding) — the reference
//! implementations do none of that, and anything extra would produce names the
//! Java/WL side won't recognize as equivalent.
//!
//! # Why not a percent-encoding crate
//!
//! The encoding step is `java.net.URLEncoder.encode`, i.e.
//! `application/x-www-form-urlencoded` — which the usual Rust crates
//! (`urlencoding`, `percent-encoding`) do not reproduce: form-urlencoding maps
//! space to `+` rather than `%20`, and its unreserved set (`A-Z a-z 0-9 . - *
//! _`) is not one of theirs. A generic percent-encoder therefore gets both
//! spaces and the safe-character set wrong, so [`java_url_encode`] implements
//! the rule set directly.

/// Uppercase hex digits, for the `%XX` byte escapes.
const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Form-urlencode `input`, matching `java.net.URLEncoder.encode(s, "UTF-8")`
/// exactly: unreserved characters (`A-Z a-z 0-9 . - * _`) pass through, space
/// becomes `+`, and everything else is percent-encoded byte-by-byte over the
/// UTF-8 encoding with uppercase hex.
///
/// Note this encodes `/` as `%2F`; callers wanting the paclet-name treatment
/// of `/` want [`sanitize_paclet_name`], which replaces it *before* encoding.
pub fn java_url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '-' | '*' | '_' => out.push(c),
            ' ' => out.push('+'),
            _ => {
                let mut buf = [0u8; 4];
                for &b in c.encode_utf8(&mut buf).as_bytes() {
                    out.push('%');
                    out.push(HEX[(b >> 4) as usize] as char);
                    out.push(HEX[(b & 0xF) as usize] as char);
                }
            },
        }
    }
    out
}

/// Sanitize a paclet `Name` into a single filename/path component.
///
/// `/` → `__` happens *before* encoding (matching `Utils.java:52` and
/// `Paclet.m:822`); since `_` is an unreserved character it then survives
/// [`java_url_encode`] untouched. Every other separator-ish or otherwise
/// unsafe character is percent-encoded, so the result is always one component
/// — a Windows `\`, for instance, becomes `%5C` rather than nesting a
/// directory.
pub fn sanitize_paclet_name(name: &str) -> String {
    java_url_encode(&name.replace('/', "__"))
}

/// The paclet's qualified name: sanitized name, optional qualifier, and
/// version joined by `-`.
///
/// This follows WL's rule, where an empty qualifier means "no qualifier".
/// Java's rule keys off `null` instead, so a `Some("")` there yields a
/// trailing empty segment (`Name--1.0.0`); if you ever need bit-compatibility
/// with the Java side specifically, that is the one case where the two
/// implementations disagree.
pub fn qualified_name(
    paclet_name: &str,
    qualifier: Option<&str>,
    version: &str,
) -> String {
    let name = sanitize_paclet_name(paclet_name);
    match qualifier {
        Some(q) if !q.is_empty() => format!("{name}-{q}-{version}"),
        _ => format!("{name}-{version}"),
    }
}

/// The `.paclet` archive filename `CreatePacletArchive` would produce for this
/// name/qualifier/version triple.
pub fn paclet_archive_filename(
    paclet_name: &str,
    qualifier: Option<&str>,
    version: &str,
) -> String {
    format!("{}.paclet", qualified_name(paclet_name, qualifier, version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_java_urlencoder() {
        assert_eq!(java_url_encode("Hello World"), "Hello+World");
        // The `/` → `__` replace has to come first: encoding on its own turns
        // a slash into %2F, not the `__` the reference implementations use.
        assert_eq!(
            java_url_encode("WolframResearch/MyPaclet"),
            "WolframResearch%2FMyPaclet"
        );
        assert_eq!(
            sanitize_paclet_name("WolframResearch/MyPaclet"),
            "WolframResearch__MyPaclet"
        );
        assert_eq!(qualified_name("Foo", None, "1.0.0"), "Foo-1.0.0");
        assert_eq!(
            qualified_name("Foo", Some("Windows"), "1.0.0"),
            "Foo-Windows-1.0.0"
        );
    }

    #[test]
    fn unreserved_characters_pass_through() {
        assert_eq!(java_url_encode("Abc.xyz-123*_"), "Abc.xyz-123*_");
    }

    #[test]
    fn non_ascii_is_encoded_per_utf8_byte() {
        // é is two UTF-8 bytes, so it becomes two uppercase %XX escapes.
        assert_eq!(java_url_encode("café"), "caf%C3%A9");
        assert_eq!(java_url_encode("日"), "%E6%97%A5");
    }

    #[test]
    fn sanitized_name_is_always_one_path_component() {
        for name in ["A/B", "A\\B", "../escape", "a b/c"] {
            let sanitized = sanitize_paclet_name(name);
            assert!(
                !sanitized.contains('/') && !sanitized.contains('\\'),
                "{name:?} sanitized to {sanitized:?}, which still has a separator"
            );
        }
        // Backslash is not unreserved, so it encodes rather than becoming `__`.
        assert_eq!(sanitize_paclet_name("A\\B"), "A%5CB");
        // No extra sanitizing: `..` and `.` are unreserved and pass through.
        assert_eq!(sanitize_paclet_name("../escape"), "..__escape");
    }

    #[test]
    fn empty_qualifier_is_omitted() {
        assert_eq!(qualified_name("Foo", Some(""), "1.0.0"), "Foo-1.0.0");
    }

    #[test]
    fn archive_filename_has_paclet_extension() {
        assert_eq!(
            paclet_archive_filename("My Paclet", None, "2.1.0"),
            "My+Paclet-2.1.0.paclet"
        );
    }
}
