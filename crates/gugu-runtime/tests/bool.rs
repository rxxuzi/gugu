mod common;
use common::{run, run_one};

fn binop(op: &str, l: bool, r: bool) -> String {
    let l = if l { "true" } else { "false" };
    let r = if r { "true" } else { "false" };
    run_one(&format!("@GEN : @{op} /rgt={r} /result=(>out) -> {l}\n"))
}

#[test]
fn and_truth_table() {
    assert_eq!(binop("AND", true, true), "TRUE");
    assert_eq!(binop("AND", true, false), "FALSE");
    assert_eq!(binop("AND", false, true), "FALSE");
    assert_eq!(binop("AND", false, false), "FALSE");
}

#[test]
fn or_truth_table() {
    assert_eq!(binop("OR", true, true), "TRUE");
    assert_eq!(binop("OR", true, false), "TRUE");
    assert_eq!(binop("OR", false, true), "TRUE");
    assert_eq!(binop("OR", false, false), "FALSE");
}

#[test]
fn xor_truth_table() {
    assert_eq!(binop("XOR", true, true), "FALSE");
    assert_eq!(binop("XOR", true, false), "TRUE");
    assert_eq!(binop("XOR", false, true), "TRUE");
    assert_eq!(binop("XOR", false, false), "FALSE");
}

#[test]
fn not_flips() {
    assert_eq!(run_one("@GEN : @NOT /result=(>out) -> true\n"), "FALSE");
    assert_eq!(run_one("@GEN : @NOT /result=(>out) -> false\n"), "TRUE");
}

#[test]
fn if_takes_then_on_true() {
    let src = "@GEN : @IF /then=42 /else=99 /result=(>out) -> true\n";
    assert_eq!(run(src), vec!["42"]);
}

#[test]
fn if_takes_else_on_false() {
    let src = "@GEN : @IF /then=42 /else=99 /result=(>out) -> false\n";
    assert_eq!(run(src), vec!["99"]);
}

#[test]
fn if_erases_unused_branch_nat() {
    // 255 = BIT1×8 + ZERO.  IF takes TRUE → /then (0) flows out,
    // /else (255) must chain-erase through @ERA without panicking.
    let src = "@GEN : @IF /then=0 /else=255 /result=(>out) -> true\n";
    assert_eq!(run(src), vec!["0"]);
}

#[test]
fn if_erases_unused_branch_string() {
    // Untaken branch is a full CONS-CHAR chain.
    let src = "@GEN : @IF /then=1 /else=\"hello\" /result=(>out) -> true\n";
    assert_eq!(run(src), vec!["1"]);
}

#[test]
fn if_erases_unused_branch_list() {
    let src = "@GEN : @IF /then=[] /else=[1,2,3,4,5] /result=(>out) -> true\n";
    assert_eq!(run(src), vec!["[]"]);
}
