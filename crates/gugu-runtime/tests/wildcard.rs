mod common;
use common::{lower_err, run_one};

const IS_ZERO_B: &str = "\
    agent @IS_ZERO_B /result\n\
    rule @IS_ZERO_B >< @ZERO : ~/result -- @TRUE\n\
    rule @IS_ZERO_B >< _     : ~/result -- @FALSE\n\
";

fn is_zero(n: i64) -> String {
    let src = format!(
        "{IS_ZERO_B}@GEN : @IS_ZERO_B /result=(>out) -> {n}\n"
    );
    run_one(&src)
}

#[test]
fn wildcard_matches_any_non_zero() {
    assert_eq!(is_zero(0), "TRUE");
    for n in [1_i64, 2, 7, 255, 1024, 65535] {
        assert_eq!(is_zero(n), "FALSE", "IS_ZERO_B({n})");
    }
}

#[test]
fn wildcard_auto_eras_string_chain() {
    // The RHS is a CONS-CHAR chain. Wildcard fire must not leave a dangling
    // BIT/CONS structure (checked by the fact that the run completes without
    // maxing out steps or panicking).
    let src = format!(
        "{IS_ZERO_B}@GEN : @IS_ZERO_B /result=(>out) -> \"hello\"\n"
    );
    assert_eq!(run_one(&src), "FALSE");
}

#[test]
fn wildcard_auto_eras_list_chain() {
    let src = format!(
        "{IS_ZERO_B}@GEN : @IS_ZERO_B /result=(>out) -> [1, 2, 3, 4, 5]\n"
    );
    assert_eq!(run_one(&src), "FALSE");
}

#[test]
fn specific_rule_overrides_wildcard() {
    // Specific @FOO >< @ZERO takes priority over @FOO >< _.
    let src = "\
        agent @FOO /result\n\
        rule @FOO >< @ZERO : ~/result -- 111\n\
        rule @FOO >< _     : ~/result -- 222\n\
        @GEN : @FOO /result=(>out) -> 0\n\
    ";
    assert_eq!(run_one(src), "111");
}

#[test]
fn wildcard_fires_when_no_specific_match() {
    let src = "\
        agent @FOO /result\n\
        rule @FOO >< @ZERO : ~/result -- 111\n\
        rule @FOO >< _     : ~/result -- 222\n\
        @GEN : @FOO /result=(>out) -> 5\n\
    ";
    assert_eq!(run_one(src), "222");
}

#[test]
fn wildcard_composes_with_if() {
    // IS_ZERO_B's output threads into IF's condition.
    let src = format!(
        "{IS_ZERO_B}@GEN :\n  \
            @IS_ZERO_B /result=>>b -> 0\n  \
            @IF /then=999 /else=1 /result=(>out) -> >>b\n"
    );
    assert_eq!(run_one(&src), "999");

    let src = format!(
        "{IS_ZERO_B}@GEN :\n  \
            @IS_ZERO_B /result=>>b -> 7\n  \
            @IF /then=999 /else=1 /result=(>out) -> >>b\n"
    );
    assert_eq!(run_one(&src), "1");
}

#[test]
fn duplicate_wildcard_rejected() {
    let src = "\
        agent @FOO /result\n\
        rule @FOO >< _ : ~/result -- @TRUE\n\
        rule @FOO >< _ : ~/result -- @FALSE\n\
        @GEN : @ZERO -> >out\n\
    ";
    let e = lower_err(src);
    assert!(
        e.msg.contains("duplicate wildcard rule for @FOO"),
        "msg = {}",
        e.msg
    );
}

#[test]
fn wildcard_body_unknown_self_port_rejected() {
    // ~/xyz isn't an arm of @FOO (LHS), and there's no RHS namespace to
    // fall back to in a wildcard rule.
    let src = "\
        agent @FOO /result\n\
        rule @FOO >< _ : ~/xyz -- ~/result\n\
        @GEN : @ZERO -> >out\n\
    ";
    let e = lower_err(src);
    assert!(e.msg.contains("~/xyz"), "msg = {}", e.msg);
}
