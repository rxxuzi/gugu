mod common;
use common::{lower_err, run_one};

#[test]
fn simple_fragment_inlines() {
    let src = "\
        $one = @BIT1 -> @ZERO\n\
        @GEN : $one -> >out\n\
    ";
    assert_eq!(run_one(src), "1");
}

#[test]
fn fragment_used_twice_is_independent() {
    // Each `$one` reference must produce a fresh BIT1-ZERO chain so
    // @ADD sees two distinct operands.
    let src = "\
        $one = @BIT1 -> @ZERO\n\
        @GEN : @ADD /lft=$one /rgt=(>out) -> $one\n\
    ";
    assert_eq!(run_one(src), "2");
}

#[test]
fn fragment_can_reference_another_fragment() {
    let src = "\
        $one   = @BIT1 -> @ZERO\n\
        $two   = @BIT0 -> (@BIT1 -> @ZERO)\n\
        $three = @BIT1 -> (@BIT1 -> @ZERO)\n\
        @GEN : @ADD /lft=$one /rgt=(>out) -> $two\n\
        @GEN : @MUL /lft=$two /rgt=(>out) -> $three\n\
    ";
    let out = common::run(src);
    assert_eq!(out, vec!["3", "6"]);
}

#[test]
fn duplicate_fragment_is_error() {
    let src = "\
        $a = @BIT1 -> @ZERO\n\
        $a = @BIT0 -> @ZERO\n\
        @GEN : @ZERO -> >out\n\
    ";
    let e = lower_err(src);
    assert!(e.msg.contains("duplicate fragment $a"), "msg = {}", e.msg);
}

#[test]
fn unknown_fragment_is_error() {
    let src = "@GEN : $missing -> >out\n";
    let e = lower_err(src);
    assert!(
        e.msg.contains("unknown fragment $missing"),
        "msg = {}",
        e.msg
    );
}

#[test]
fn parameterized_fragment_is_rejected() {
    let src = "\
        $inc /n = @ADD /lft=/n /rgt=@ZERO\n\
        @GEN : @ZERO -> >out\n\
    ";
    let e = lower_err(src);
    assert!(
        e.msg.contains("parameterized fragment $inc"),
        "msg = {}",
        e.msg
    );
}

#[test]
fn self_port_in_fragment_body_is_rejected() {
    // `~/x` has no LHS/RHS aux context when a fragment is expanded outside
    // a rule body; catch the typo early.
    let src = "\
        $bad = @ADD /lft=~/x /rgt=@ZERO\n\
        @GEN : $bad -> >out\n\
    ";
    let e = lower_err(src);
    assert!(e.msg.contains("~/x"), "msg = {}", e.msg);
}

#[test]
fn cyclic_fragments_hit_depth_limit() {
    // $a -> $b -> $a ... expansion recurses until FRAG_DEPTH_LIMIT trips.
    let src = "\
        $a = $b\n\
        $b = $a\n\
        @GEN : $a -> >out\n\
    ";
    let e = lower_err(src);
    assert!(
        e.msg.contains("exceeded depth") || e.msg.contains("cycle"),
        "msg = {}",
        e.msg
    );
}
