//! Public API and JSON grammar regression tests.
use std::borrow::Cow;
use strata_json::{from_slice, from_str, from_str_with_depth, ErrorKind, Value, MAX_DEPTH};

#[test]
fn parses_nested_document() {
    let v = from_str(r#" {"name":"Ada","active":true,"items":[null,false,42,{},[]]} "#).unwrap();
    assert_eq!(v.get("name").unwrap().as_str(), Some("Ada"));
    assert_eq!(v.get("active").unwrap().as_bool(), Some(true));
    let a = v.get("items").unwrap().as_array().unwrap();
    assert!(a[0].is_null());
    assert_eq!(a[1].as_bool(), Some(false));
    assert_eq!(a[2].as_number().unwrap().as_i64(), Some(42));
    assert!(a[3].as_object().unwrap().is_empty());
    assert!(a[4].as_array().unwrap().is_empty());
    assert_eq!(v.get("absent"), None);
}

#[test]
fn borrows_strings_keys_and_numbers() {
    let input = String::from(r#"{"key":"héllo 🦀","number":123.40e+2}"#);
    let v = from_str(&input).unwrap();
    let entries = v.as_object().unwrap();
    assert!(matches!(entries[0].0, Cow::Borrowed(_)));
    match &entries[0].1 {
        Value::String(Cow::Borrowed(s)) => assert_eq!(s.as_ptr(), input[8..].as_ptr()),
        _ => panic!("expected borrowed string"),
    }
    let n = entries[1].1.as_number().unwrap();
    assert_eq!(n.as_str(), "123.40e+2");
    assert_eq!(
        n.as_str().as_ptr(),
        input[input.find("123").unwrap()..].as_ptr()
    );
}

#[test]
fn decodes_every_escape() {
    let v = from_str(r#""\"\\\/\b\f\n\r\t\u0041\u00e9\uD83E\uDD80""#).unwrap();
    assert_eq!(v.as_str(), Some("\"\\/\u{8}\u{c}\n\r\tAé🦀"));
    assert!(matches!(v, Value::String(Cow::Owned(_))));
    assert_eq!(
        from_str(r#""é\n🦀\u0000fin""#).unwrap().as_str(),
        Some("é\n🦀\0fin")
    );
}

#[test]
fn preserves_duplicates_and_decodes_keys() {
    let v = from_str(r#"{"a":1,"\u0061":2}"#).unwrap();
    assert_eq!(v.as_object().unwrap().len(), 2);
    assert_eq!(v.get("a").unwrap().as_number().unwrap().as_i64(), Some(2));
}

#[test]
fn lossless_numbers_and_checked_conversions() {
    for s in [
        "0",
        "-0",
        "1",
        "-42",
        "1.25",
        "1e2",
        "1E-2",
        "1e+2",
        "1e9999",
        "18446744073709551616",
    ] {
        assert_eq!(from_str(s).unwrap().as_number().unwrap().as_str(), s);
    }
    assert_eq!(
        from_str("18446744073709551615")
            .unwrap()
            .as_number()
            .unwrap()
            .as_u64(),
        Some(u64::MAX)
    );
    assert_eq!(
        from_str("-9223372036854775808")
            .unwrap()
            .as_number()
            .unwrap()
            .as_i64(),
        Some(i64::MIN)
    );
    assert_eq!(
        from_str("18446744073709551616")
            .unwrap()
            .as_number()
            .unwrap()
            .as_u64(),
        None
    );
    assert_eq!(
        from_str("1e9999").unwrap().as_number().unwrap().as_f64(),
        None
    );
    assert_eq!(
        from_str("1.25").unwrap().as_number().unwrap().as_f64(),
        Some(1.25)
    );
    assert_eq!(from_str("1e2").unwrap().as_number().unwrap().as_i64(), None);
}

#[test]
fn rejects_invalid_documents() {
    for s in [
        "",
        " ",
        "nul",
        "True",
        "undefined",
        "NaN",
        "Infinity",
        "+1",
        "01",
        "-01",
        "-",
        "1.",
        ".1",
        "1e",
        "1e+",
        "--1",
        "[1,]",
        "[,1]",
        "[1 2]",
        "[",
        "{",
        "{a:1}",
        "{\"a\" 1}",
        "{\"a\":}",
        "{\"a\":1,}",
        "true false",
        "[]x",
        "/*x*/null",
        "\u{feff}null",
        "\u{a0}null",
        "\"unterminated",
        "\"line\nfeed\"",
        r#""\x20""#,
        r#""\uZZZZ""#,
        r#""\uD800""#,
        r#""\uDC00""#,
        r#""\uD800\u0041""#,
        r#""\uD800\uD800""#,
        r#""\u12""#,
    ] {
        assert!(from_str(s).is_err(), "unexpectedly accepted {s:?}");
    }
    for control in 0..32u8 {
        let bytes = [b'"', control, b'"'];
        assert!(from_slice(&bytes).is_err());
    }
}

#[test]
fn precise_error_locations() {
    let e = from_str("{\n  \"x\": @}").unwrap_err();
    assert_eq!(
        (e.kind, e.offset, e.line, e.column),
        (ErrorKind::UnexpectedToken, 9, 2, 8)
    );
    assert_eq!(
        e.to_string(),
        "unexpected token at line 2, column 8 (byte 9)"
    );
    let e = from_str("\"é\" x").unwrap_err();
    assert_eq!(
        (e.kind, e.offset, e.column),
        (ErrorKind::TrailingCharacters, 5, 6)
    );
    let e = from_str("[\n").unwrap_err();
    assert_eq!(
        (e.kind, e.offset, e.line, e.column),
        (ErrorKind::UnexpectedEof, 2, 2, 1)
    );
}

#[test]
fn validates_utf8() {
    assert_eq!(from_slice(b"[true]").unwrap(), from_str("[true]").unwrap());
    let e = from_slice(&[b'\n', b'"', 0xff, b'"']).unwrap_err();
    assert_eq!(
        (e.kind, e.offset, e.line, e.column),
        (ErrorKind::InvalidUtf8, 2, 2, 2)
    );
}

#[test]
fn enforces_depth_boundaries() {
    assert!(from_str_with_depth("true", 0).is_ok());
    assert_eq!(
        from_str_with_depth("[]", 0).unwrap_err().kind,
        ErrorKind::DepthLimit
    );
    assert!(from_str_with_depth("[{}]", 2).is_ok());
    assert!(from_str_with_depth("{\"a\":[]}", 1).is_err());
    let accepted = format!("{}0{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
    assert!(from_str(&accepted).is_ok());
    let rejected = format!("[{accepted}]");
    assert_eq!(
        from_str_with_depth(&rejected, usize::MAX).unwrap_err().kind,
        ErrorKind::DepthLimit
    );
    assert!(from_str(&"[".repeat(100_000)).is_err());
}

#[test]
fn all_unicode_scalars_round_trip_through_escapes() {
    for code in 0..=0x10ffff {
        let Some(ch) = char::from_u32(code) else {
            continue;
        };
        let json = if code <= 0xffff {
            format!("\"\\u{code:04x}\"")
        } else {
            let n = code - 0x10000;
            format!(
                "\"\\u{:04x}\\u{:04x}\"",
                0xd800 + (n >> 10),
                0xdc00 + (n & 0x3ff)
            )
        };
        assert_eq!(
            from_str(&json).unwrap().as_str().unwrap().chars().next(),
            Some(ch)
        );
    }
}

#[test]
fn truncated_and_mutated_inputs_never_panic() {
    let source = br#"{"name":"unicode \uD83E\uDD80","items":[1,-2.3e+4,true,null]}"#;
    for end in 0..source.len() {
        assert!(from_slice(&source[..end]).is_err());
    }
    for i in 0..source.len() {
        for byte in 0..=255 {
            let mut mutated = source.to_vec();
            mutated[i] = byte;
            if let Err(e) = from_slice(&mutated) {
                assert!(e.offset <= mutated.len());
            }
        }
    }
}
