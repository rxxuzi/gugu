mod common;
use gugu_parser::parse;
use gugu_runtime::run_tests;

fn outcomes(src: &str) -> Vec<(String, bool, String, String)> {
    let program = parse(src).unwrap();
    run_tests(&program)
        .into_iter()
        .map(|o| (o.label, o.pass, o.lhs, o.rhs))
        .collect()
}

#[test]
fn passing_arithmetic_tests() {
    let src = "\
        test \"1+1=2\" : @ADD /lft=1 /rgt=(>out) -> 1 == 2\n\
        test \"3*4=12\" : @MUL /lft=3 /rgt=(>out) -> 4 == 12\n\
    ";
    let out = outcomes(src);
    assert_eq!(out.len(), 2);
    assert!(out.iter().all(|(_, p, _, _)| *p));
}

#[test]
fn literal_equals_literal() {
    // `2 == 2` — both sides auto-wired to >out by the test runner.
    let src = "test \"triv\" : 2 == 2\n";
    let out = outcomes(src);
    assert_eq!(out[0].1, true);
    assert_eq!(out[0].2, "2");
    assert_eq!(out[0].3, "2");
}

#[test]
fn failing_test_reports_both_sides() {
    let src = "test \"no\" : 1 == 2\n";
    let out = outcomes(src);
    assert_eq!(out[0].1, false);
    assert_eq!(out[0].2, "1");
    assert_eq!(out[0].3, "2");
}

#[test]
fn list_equality() {
    let src = "test \"tail\" : @TAIL /result=(>out) -> [1, 2, 3] == [2, 3]\n";
    let out = outcomes(src);
    assert!(out[0].1, "outcomes = {:?}", out);
}

#[test]
fn test_block_respects_user_rules_and_fragments() {
    // The mini-program carries over agent defs, rules, and fragments so
    // tests can exercise user-defined logic.
    let src = "\
        $one = @BIT1 -> @ZERO\n\
        test \"frag\" : @ADD /lft=$one /rgt=(>out) -> $one == 2\n\
    ";
    let out = outcomes(src);
    assert!(out[0].1, "{out:?}");
}

#[test]
fn mixed_pass_fail() {
    let src = "\
        test \"ok\"   : 1 == 1\n\
        test \"bad\"  : 1 == 2\n\
        test \"ok2\"  : @ADD /lft=2 /rgt=(>out) -> 3 == 5\n\
    ";
    let out = outcomes(src);
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].1, true);
    assert_eq!(out[1].1, false);
    assert_eq!(out[2].1, true);
}
