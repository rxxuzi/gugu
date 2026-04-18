mod common;
use common::run;

#[test]
fn int_round_trip_across_range() {
    for n in [
        0_i64,
        1,
        2,
        3,
        5,
        42,
        255,
        1023,
        i32::MAX as i64,
        -1,
        -42,
        i32::MIN as i64,
        i64::MIN,
        i64::MAX,
    ] {
        let src = format!("@GEN : {n} -> >out\n");
        assert_eq!(run(&src), vec![n.to_string()], "n = {n}");
    }
}

#[test]
fn int_powers_of_two_round_trip() {
    for k in 0..62 {
        let n = 1_i64 << k;
        let src = format!("@GEN : {n} -> >out\n");
        assert_eq!(run(&src), vec![n.to_string()], "2^{k} = {n}");
    }
}

#[test]
fn char_ascii() {
    for (src, expect) in [
        ("'A'", "'A'"),
        ("'z'", "'z'"),
        ("'0'", "'0'"),
        ("'!'", "'!'"),
        ("' '", "' '"),
    ] {
        assert_eq!(run(&format!("@GEN : {src} -> >out\n")), vec![expect]);
    }
}

#[test]
fn char_escapes() {
    for (src, expect) in [
        (r"'\n'", r"'\n'"),
        (r"'\t'", r"'\t'"),
        (r"'\\'", r"'\\'"),
        (r"'\''", r"'\''"),
    ] {
        assert_eq!(run(&format!("@GEN : {src} -> >out\n")), vec![expect]);
    }
}

#[test]
fn char_utf8() {
    // Byte lengths: 2 (ä), 3 (あ), 4 (🌸).
    for (src, expect) in [
        ("'ä'", "'ä'"),
        ("'あ'", "'あ'"),
        ("'漢'", "'漢'"),
        ("'🌸'", "'🌸'"),
    ] {
        assert_eq!(run(&format!("@GEN : {src} -> >out\n")), vec![expect]);
    }
}

#[test]
fn str_ascii() {
    assert_eq!(run("@GEN : \"hi\" -> >out\n"), vec!["\"hi\""]);
    assert_eq!(run("@GEN : \"hello, world\" -> >out\n"), vec!["\"hello, world\""]);
}

#[test]
fn str_empty_is_bare_nil_rendered_as_empty_list() {
    // "" and [] share the same web (bare @NIL) so both render as [].
    assert_eq!(run("@GEN : \"\" -> >out\n"), vec!["[]"]);
    assert_eq!(run("@GEN : [] -> >out\n"), vec!["[]"]);
}

#[test]
fn str_escapes() {
    assert_eq!(
        run(r#"@GEN : "a\tb\nc" -> >out"#),
        vec![r#""a\tb\nc""#]
    );
}

#[test]
fn str_utf8() {
    assert_eq!(run("@GEN : \"こんにちは\" -> >out\n"), vec!["\"こんにちは\""]);
    assert_eq!(run("@GEN : \"🌸🌸🌸\" -> >out\n"), vec!["\"🌸🌸🌸\""]);
}

#[test]
fn str_long() {
    // 200-char string exercises longer CONS chains.
    let s: String = "abcdefghij".repeat(20);
    let src = format!("@GEN : \"{s}\" -> >out\n");
    assert_eq!(run(&src), vec![format!("\"{s}\"")]);
}
