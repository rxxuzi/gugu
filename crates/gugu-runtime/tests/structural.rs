mod common;
use common::lower_err;

#[test]
fn duplicate_rule_rejected() {
    let src = "\
        rule @ADD >< @ZERO : ~/lft -- ~/rgt\n\
        @GEN : @ZERO -> >out\n\
    ";
    let e = lower_err(src);
    assert!(e.msg.contains("duplicate rule"), "msg = {}", e.msg);
}

#[test]
fn duplicate_port_name_rejected() {
    let e = lower_err("agent @FOO /a /a @GEN : @ZERO -> >out\n");
    assert!(e.msg.contains("declares port /a twice"), "msg = {}", e.msg);
}

#[test]
fn unknown_port_assignment_rejected() {
    let src = "\
        agent @BAZ /lft /rgt\n\
        @GEN : @BAZ /bogus=@ZERO /rgt=(>out) -> @ZERO\n\
    ";
    let e = lower_err(src);
    assert!(
        e.msg.contains("unknown port /bogus on @BAZ"),
        "msg = {}",
        e.msg
    );
}

#[test]
fn duplicate_port_assign_rejected() {
    let src = "\
        agent @BAZ /lft /rgt\n\
        @GEN : @BAZ /lft=@ZERO /lft=@ZERO /rgt=(>out) -> @ZERO\n\
    ";
    let e = lower_err(src);
    assert!(
        e.msg.contains("port /lft of @BAZ assigned twice"),
        "msg = {}",
        e.msg
    );
}

#[test]
fn unknown_self_port_rejected() {
    let src = "\
        agent @FOO /a /b\n\
        rule @FOO >< @ZERO : ~/xyz -- ~/a\n\
        @GEN : @ZERO -> >out\n\
    ";
    let e = lower_err(src);
    assert!(e.msg.contains("~/xyz"), "msg = {}", e.msg);
}

#[test]
fn anon_wire_once_rejected() {
    let e = lower_err("@GEN : @ZERO -- >>w\n");
    assert!(e.msg.contains(">>w"), "msg = {}", e.msg);
}

#[test]
fn anon_wire_three_times_rejected() {
    let src = "@GEN :\n  @ZERO -- >>w\n  @ZERO -- >>w\n  @ZERO -- >>w\n  @ZERO -> >out\n";
    let e = lower_err(src);
    assert!(
        e.msg.contains("3 times") || e.msg.contains("must be exactly 2"),
        "msg = {}",
        e.msg
    );
}
