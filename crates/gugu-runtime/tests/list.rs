mod common;
use common::{run, run_one};

#[test]
fn empty_list() {
    assert_eq!(run("@GEN : [] -> >out\n"), vec!["[]"]);
}

#[test]
fn singleton() {
    assert_eq!(run("@GEN : [7] -> >out\n"), vec!["[7]"]);
}

#[test]
fn small_lists() {
    assert_eq!(run("@GEN : [1, 2, 3] -> >out\n"), vec!["[1, 2, 3]"]);
    assert_eq!(run("@GEN : [42] -> >out\n"), vec!["[42]"]);
    assert_eq!(run("@GEN : [0, 0, 0] -> >out\n"), vec!["[0, 0, 0]"]);
}

#[test]
fn nested_lists_preserve_shape() {
    assert_eq!(
        run("@GEN : [1, [2, 3], 4] -> >out\n"),
        vec!["[1, [2, 3], 4]"]
    );
    assert_eq!(
        run("@GEN : [[1], [2, 3], []] -> >out\n"),
        vec!["[[1], [2, 3], []]"]
    );
}

#[test]
fn long_list_round_trip() {
    let items: Vec<i32> = (0..50).collect();
    let body = items
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let src = format!("@GEN : [{body}] -> >out\n");
    let expect = format!("[{body}]");
    assert_eq!(run(&src), vec![expect]);
}

#[test]
fn head_of_non_empty() {
    let src = "@GEN : @HEAD /result=(>out) -> [10, 20, 30]\n";
    assert_eq!(run_one(src), "10");
}

#[test]
fn head_of_single() {
    assert_eq!(run_one("@GEN : @HEAD /result=(>out) -> [42]\n"), "42");
}

#[test]
fn head_of_empty_emits_err() {
    // HEAD >< NIL: ~/result -- @ERR.  ERR has unconnected /msg so it
    // renders as ERR(_).
    let out = run_one("@GEN : @HEAD /result=(>out) -> []\n");
    assert!(out.starts_with("ERR"), "got {out}");
}

#[test]
fn tail_of_multi() {
    assert_eq!(
        run_one("@GEN : @TAIL /result=(>out) -> [10, 20, 30]\n"),
        "[20, 30]"
    );
}

#[test]
fn tail_of_single_is_empty() {
    assert_eq!(run_one("@GEN : @TAIL /result=(>out) -> [9]\n"), "[]");
}

#[test]
fn tail_of_empty_is_empty() {
    assert_eq!(run_one("@GEN : @TAIL /result=(>out) -> []\n"), "[]");
}

#[test]
fn nested_head_returns_inner_list() {
    // HEAD([[1,2,3], [4,5]]) = [1,2,3]
    assert_eq!(
        run_one("@GEN : @HEAD /result=(>out) -> [[1,2,3], [4,5]]\n"),
        "[1, 2, 3]"
    );
}

#[test]
fn list_of_strings_renders_as_nested() {
    // Mixed-type list: outer is a list of two CONS-CHAR strings.
    assert_eq!(
        run_one("@GEN : [\"hi\", \"yo\"] -> >out\n"),
        "[\"hi\", \"yo\"]"
    );
}
