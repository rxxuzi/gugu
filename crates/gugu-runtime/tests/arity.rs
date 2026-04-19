mod common;
use common::run_one;

#[test]
fn positional_inside_paren_chain() {
    // `(@BIT1 @ZERO)` is @BIT1 with @ZERO as its /hi arm — a complete
    // Nat chain for `1`.
    assert_eq!(run_one("@GEN : (@BIT1 @ZERO) -> >out\n"), "1");
}

#[test]
fn nested_arity_builds_binary_chain() {
    // `(@BIT0 (@BIT1 @ZERO))` = BIT0 -> BIT1 -> ZERO = binary 10 = 2
    assert_eq!(run_one("@GEN : (@BIT0 (@BIT1 @ZERO)) -> >out\n"), "2");
}

#[test]
fn paren_arity_feeds_add() {
    let src = "@GEN : @ADD /lft=(@BIT1 @ZERO) /rgt=(>out) -> (@BIT1 @ZERO)\n";
    assert_eq!(run_one(src), "2");
}

#[test]
fn arity_in_ADD_positional_form() {
    // Fully parenthesized: @ADD(@BIT1 @ZERO)(...). The outer ADD takes
    // two positional parens; each inner paren itself uses arity to build
    // a 1 Nat.
    let src = "@GEN : @ADD(@BIT1 @ZERO)(>out) -> (@BIT1 @ZERO)\n";
    assert_eq!(run_one(src), "2");
}

#[test]
fn stmt_level_is_not_greedy() {
    // At statement level, adjacent `@BIT1 @ZERO` must stay as two stmts
    // (the second @BIT1's `@ZERO` would not get swallowed by @BIT1).
    // We check this via a rule body with two stmts on separate lines.
    let src = "\
        agent @FOO /a /b\n\
        rule @FOO >< @ZERO :\n  \
          ~/a -- @ZERO\n  \
          ~/b -- @ZERO\n\
        @GEN : @FOO /a=(>out) /b=@ZERO -> @ZERO\n\
    ";
    // If stmt-level became greedy, @ZERO on the first stmt would eat the
    // second @ZERO and the body would collapse (FOO never fires).
    // The Nat-int formatter renders the bare @ZERO output as "0".
    assert_eq!(run_one(src), "0");
}
