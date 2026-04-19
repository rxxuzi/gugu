mod common;
use common::{lower_err, run_one};

#[test]
fn non_pub_agent_unreachable_from_outside_mod() {
    let src = "\
        mod Priv :\n  \
          agent @SECRET /x\n  \
          rule @SECRET >< @ZERO : ~/x -- @ZERO\n\
        end\n\
        @GEN : @SECRET /x=(>out) -> @ZERO\n\
    ";
    let e = lower_err(src);
    assert!(
        e.msg.contains("@SECRET") && e.msg.contains("private") && e.msg.contains("Priv"),
        "msg = {}",
        e.msg
    );
}

#[test]
fn pub_agent_reachable_from_outside_mod() {
    let src = "\
        mod Lib :\n  \
          pub agent @ALIAS /a\n  \
          rule @ALIAS >< @ZERO : ~/a -- @ZERO\n\
        end\n\
        @GEN : @ALIAS /a=(>out) -> @ZERO\n\
    ";
    assert_eq!(run_one(src), "0");
}

#[test]
fn non_pub_still_reachable_within_its_own_mod() {
    // Inside `mod Priv`, the rule body can refer to @SECRET even though
    // it's not `pub` — only cross-mod references are gated.
    let src = "\
        mod Priv :\n  \
          agent @SECRET\n  \
          agent @CALLER /r\n  \
          rule @CALLER >< @ZERO : ~/r -- @SECRET\n\
        end\n\
        mod Open :\n  \
          pub agent @GO /r\n  \
          rule @GO >< @ZERO : ~/r -- @TRUE\n\
        end\n\
        @GEN : @GO /r=(>out) -> @ZERO\n\
    ";
    assert_eq!(run_one(src), "TRUE");
}

#[test]
fn cross_mod_non_pub_reference_rejected() {
    // @SECRET declared in mod A, referenced from a rule inside mod B.
    let src = "\
        mod A :\n  \
          agent @SECRET\n\
        end\n\
        mod B :\n  \
          pub agent @CALLER /r\n  \
          rule @CALLER >< @ZERO : ~/r -- @SECRET\n\
        end\n\
        @GEN : @CALLER /r=(>out) -> @ZERO\n\
    ";
    let e = lower_err(src);
    assert!(
        e.msg.contains("@SECRET") && e.msg.contains("private"),
        "msg = {}",
        e.msg
    );
}
