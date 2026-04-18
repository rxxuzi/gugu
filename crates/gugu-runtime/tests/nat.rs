mod common;
use common::run_one;

fn add(l: i64, r: i64) -> String {
    run_one(&format!("@GEN : @ADD /lft={l} /rgt=(>out) -> {r}\n"))
}
fn sub(l: i64, r: i64) -> String {
    run_one(&format!("@GEN : @SUB /lft={l} /rgt=(>out) -> {r}\n"))
}
fn mul(l: i64, r: i64) -> String {
    run_one(&format!("@GEN : @MUL /lft={l} /rgt=(>out) -> {r}\n"))
}

#[test]
fn add_identity_with_zero() {
    for n in [0_i64, 1, 7, 255, 1024, 65535] {
        assert_eq!(add(n, 0), n.to_string(), "ADD({n}, 0)");
        assert_eq!(add(0, n), n.to_string(), "ADD(0, {n})");
    }
}

#[test]
fn add_small_mixed() {
    let cases: &[(i64, i64)] = &[
        (1, 1),
        (2, 3),
        (3, 5),
        (7, 9),
        (10, 100),
        (42, 42),
        (255, 1),
        (1023, 1),
        (12345, 67890),
    ];
    for &(a, b) in cases {
        assert_eq!(add(a, b), (a + b).to_string(), "ADD({a}, {b})");
    }
}

#[test]
fn add_commutative() {
    for &(a, b) in &[(3_i64, 5), (7, 12), (100, 27), (0, 999)] {
        assert_eq!(add(a, b), add(b, a), "ADD({a}, {b})");
    }
}

#[test]
fn add_carry_boundary_powers_of_two() {
    // Power-of-two + 1 triggers a carry across the whole bit width.
    for bits in 1u32..12 {
        let p = 1_i64 << bits;
        assert_eq!(add(p - 1, 1), p.to_string(), "ADD({}, 1)", p - 1);
        assert_eq!(add(p, p), (2 * p).to_string(), "ADD({p}, {p})");
    }
}

#[test]
fn add_large_numbers() {
    // 20-bit operands exercise long carry chains.
    let cases: &[(i64, i64)] = &[
        (0xF_FFFF, 1),
        (0xA_AAAA, 0x5_5555),
        (1_000_000, 999_999),
    ];
    for &(a, b) in cases {
        assert_eq!(add(a, b), (a + b).to_string(), "ADD({a}, {b})");
    }
}

#[test]
fn sub_small() {
    let cases: &[(i64, i64)] = &[
        (0, 0),
        (1, 0),
        (1, 1),
        (7, 7),
        (10, 3),
        (100, 37),
        (255, 1),
        (1000, 1),
        (1024, 512),
    ];
    for &(a, b) in cases {
        assert_eq!(sub(a, b), (a - b).to_string(), "SUB({a}, {b})");
    }
}

#[test]
fn sub_boundary_borrow_across_bits() {
    // k - 1 forces borrow all the way through a zero run.
    for bits in 1u32..12 {
        let p = 1_i64 << bits;
        assert_eq!(sub(p, 1), (p - 1).to_string(), "SUB({p}, 1)");
    }
}

#[test]
fn sub_from_zero_saturates() {
    // lft = 0 hits SL1/SL2 >< ZERO → saturates to 0.
    for n in [1_i64, 7, 42, 1000] {
        assert_eq!(sub(0, n), "0", "SUB(0, {n})");
    }
}

#[test]
fn mul_zero_absorbs() {
    for n in [0_i64, 1, 7, 255, 1024] {
        assert_eq!(mul(0, n), "0", "MUL(0, {n})");
        assert_eq!(mul(n, 0), "0", "MUL({n}, 0)");
    }
}

#[test]
fn mul_one_is_identity() {
    for n in [0_i64, 1, 3, 17, 255, 1024] {
        assert_eq!(mul(1, n), n.to_string(), "MUL(1, {n})");
        assert_eq!(mul(n, 1), n.to_string(), "MUL({n}, 1)");
    }
}

#[test]
fn mul_small() {
    let cases: &[(i64, i64)] = &[
        (3, 4),
        (7, 8),
        (12, 12),
        (13, 17),
        (15, 15),
        (31, 33),
    ];
    for &(a, b) in cases {
        assert_eq!(mul(a, b), (a * b).to_string(), "MUL({a}, {b})");
    }
}

#[test]
fn mul_commutative() {
    for &(a, b) in &[(3_i64, 7), (13, 5), (12, 11), (100, 7)] {
        assert_eq!(mul(a, b), mul(b, a), "MUL({a}, {b})");
    }
}

#[test]
fn mul_powers_of_two() {
    // n * 2^k = n << k. Pure BIT0 prepending — no carry fan-out.
    for n in [1_i64, 3, 17, 255] {
        for k in 0..10 {
            let p = 1_i64 << k;
            assert_eq!(mul(n, p), (n * p).to_string(), "MUL({n}, {p})");
        }
    }
}
