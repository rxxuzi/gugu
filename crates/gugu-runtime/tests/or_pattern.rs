mod common;
use common::{lower_err, run_one};

const KILL: &str = "\
    agent @KILL /result\n\
    rule @KILL >< @BIT0 | @BIT1 :\n\
      ~/result -- @TRUE\n\
      ~/hi -- @ERA\n\
    rule @KILL >< @ZERO :\n\
      ~/result -- @FALSE\n\
";

#[test]
fn or_expands_to_each_rhs() {
    // `@BIT0 | @BIT1` fans out into one specific entry per alternative,
    // so both 1 (BIT1(ZERO)) and 2 (BIT0(BIT1(ZERO))) trigger the TRUE body.
    let src_zero = format!("{KILL}@GEN : @KILL /result=(>out) -> 0\n");
    let src_one = format!("{KILL}@GEN : @KILL /result=(>out) -> 1\n");
    let src_two = format!("{KILL}@GEN : @KILL /result=(>out) -> 2\n");
    assert_eq!(run_one(&src_zero), "FALSE");
    assert_eq!(run_one(&src_one), "TRUE");
    assert_eq!(run_one(&src_two), "TRUE");
}

#[test]
fn or_respects_arm_linearity_for_each_alternative() {
    // The body is checked against each alternative's arms individually.
    // BIT0 and BIT1 both have /hi, so the single body is valid for both.
    // If one alternative lacked /hi, lowering should reject it — the check
    // here is that the common body succeeds.
    let src = format!("{KILL}@GEN : @KILL /result=(>out) -> 5\n");
    assert_eq!(run_one(&src), "TRUE");
}

#[test]
fn or_duplicate_specific_rule_rejected() {
    // Expanding `|` into separate entries must still honour the
    // duplicate-rule check. Declaring `@FOO >< @BIT0` both inside an
    // OR and as its own rule is a collision.
    let src = "\
        agent @FOO /result\n\
        rule @FOO >< @BIT0 | @BIT1 :\n  \
          ~/result -- @TRUE\n  \
          ~/hi -- @ERA\n\
        rule @FOO >< @BIT0 :\n  \
          ~/result -- @FALSE\n  \
          ~/hi -- @ERA\n\
        @GEN : @ZERO -> >out\n\
    ";
    let e = lower_err(src);
    assert!(
        e.msg.contains("duplicate rule for @FOO >< @BIT0"),
        "msg = {}",
        e.msg
    );
}

#[test]
fn or_body_arm_mismatch_rejected() {
    // BIT0 has /hi, ZERO has none. A body that only works for BIT0
    // (referencing /hi) must be rejected when expanded against ZERO.
    let src = "\
        agent @FOO /result\n\
        rule @FOO >< @BIT0 | @ZERO :\n  \
          ~/result -- @TRUE\n  \
          ~/hi -- @ERA\n\
        @GEN : @ZERO -> >out\n\
    ";
    let e = lower_err(src);
    assert!(
        e.msg.contains("~/hi") && e.msg.contains("@ZERO"),
        "msg = {}",
        e.msg
    );
}
