// This is mostly here so that we can share the implementation between the interpreter and the optimizer.

#[inline]
pub const fn divu(lhs: u32, rhs: u32) -> u32 {
    if rhs == 0 {
        u32::MAX
    } else {
        lhs / rhs
    }
}

#[inline]
pub const fn divu64(lhs: u64, rhs: u64) -> u64 {
    if rhs == 0 {
        u64::MAX
    } else {
        lhs / rhs
    }
}

#[inline]
pub const fn remu(lhs: u32, rhs: u32) -> u32 {
    if rhs == 0 {
        lhs
    } else {
        lhs % rhs
    }
}

#[inline]
pub const fn remu64(lhs: u64, rhs: u64) -> u64 {
    if rhs == 0 {
        lhs
    } else {
        lhs % rhs
    }
}

#[inline]
pub const fn div(lhs: i32, rhs: i32) -> i32 {
    if rhs == 0 {
        -1
    } else if lhs == i32::MIN && rhs == -1 {
        lhs
    } else {
        lhs / rhs
    }
}

#[inline]
pub const fn div64(lhs: i64, rhs: i64) -> i64 {
    if rhs == 0 {
        -1
    } else if lhs == i64::MIN && rhs == -1 {
        lhs
    } else {
        lhs / rhs
    }
}

#[inline]
pub const fn rem(lhs: i32, rhs: i32) -> i32 {
    if rhs == 0 {
        lhs
    } else if lhs == i32::MIN && rhs == -1 {
        0
    } else {
        lhs % rhs
    }
}

#[inline]
pub const fn rem64(lhs: i64, rhs: i64) -> i64 {
    if rhs == 0 {
        lhs
    } else if lhs == i64::MIN && rhs == -1 {
        0
    } else {
        lhs % rhs
    }
}

#[inline]
pub const fn mulh(lhs: i32, rhs: i32) -> i32 {
    ((lhs as i64).wrapping_mul(rhs as i64) >> 32) as i32
}

#[inline]
pub const fn mulh64(lhs: i64, rhs: i64) -> i64 {
    ((lhs as i128).wrapping_mul(rhs as i128) >> 64) as i64
}

#[inline]
pub const fn mulhsu(lhs: i32, rhs: u32) -> i32 {
    ((lhs as i64).wrapping_mul(rhs as i64) >> 32) as i32
}

#[inline]
pub const fn mulhsu64(lhs: i64, rhs: u64) -> i64 {
    ((lhs as i128).wrapping_mul(rhs as i128) >> 64) as i64
}

#[inline]
pub const fn mulhu(lhs: u32, rhs: u32) -> u32 {
    ((lhs as i64).wrapping_mul(rhs as i64) >> 32) as u32
}

#[inline]
pub const fn mulhu64(lhs: u64, rhs: u64) -> u64 {
    ((lhs as i128).wrapping_mul(rhs as i128) >> 64) as u64
}

#[inline]
pub const fn mulwide64(lhs: u64, rhs: u64) -> (u64, u64) {
    let product = (lhs as u128).wrapping_mul(rhs as u128);
    ((product >> 64) as u64, product as u64)
}

// Reference semantics for the 256-bit wide-arithmetic instructions. All values are
// little-endian 4×u64 (or 8×u64 for 512-bit) limb arrays. These definitions are
// normative: the interpreter uses them directly and the recompiler backends must
// produce identical results.

#[inline]
pub fn wide_mul256(lhs: &[u64; 4], rhs: &[u64; 4]) -> [u64; 8] {
    let mut result = [0u64; 8];
    for i in 0..4 {
        let mut carry: u128 = 0;
        for j in 0..4 {
            let value = u128::from(result[i + j]) + u128::from(lhs[i]) * u128::from(rhs[j]) + carry;
            result[i + j] = value as u64;
            carry = value >> 64;
        }
        result[i + 4] = carry as u64;
    }
    result
}

#[inline]
pub fn wide_add256(lhs: &[u64; 4], rhs: &[u64; 4]) -> ([u64; 4], u64) {
    let mut result = [0u64; 4];
    let mut carry: u128 = 0;
    for i in 0..4 {
        let value = u128::from(lhs[i]) + u128::from(rhs[i]) + carry;
        result[i] = value as u64;
        carry = value >> 64;
    }
    (result, carry as u64)
}

#[inline]
pub fn wide_sub256(lhs: &[u64; 4], rhs: &[u64; 4]) -> ([u64; 4], u64) {
    let mut result = [0u64; 4];
    let mut borrow: u64 = 0;
    for i in 0..4 {
        let (value, b1) = lhs[i].overflowing_sub(rhs[i]);
        let (value, b2) = value.overflowing_sub(borrow);
        result[i] = value;
        borrow = u64::from(b1) | u64::from(b2);
    }
    (result, borrow)
}

/// `low256(lhs × rhs)` plus bits 256..319 of the full product.
#[inline]
pub fn wide_mul256_by_u64(lhs: &[u64; 4], rhs: u64) -> ([u64; 4], u64) {
    let mut result = [0u64; 4];
    let mut carry: u128 = 0;
    for i in 0..4 {
        let value = u128::from(lhs[i]) * u128::from(rhs) + carry;
        result[i] = value as u64;
        carry = value >> 64;
    }
    (result, carry as u64)
}

/// Folds a 512-bit value modulo `2^256 - k`. The result is congruent to `src`
/// (mod 2^256 - k) and always < 2^256, but is not guaranteed to be fully
/// canonicalized (it may still be >= 2^256 - k).
///
/// The exact output is defined by this algorithm; any `k < 2^64` is legal
/// (all bounds are guaranteed by k² + k < 2^128), and k = 0 degenerates to
/// `src mod 2^256`.
#[inline]
pub fn wide_redc256(src: &[u64; 8], k: u64) -> [u64; 4] {
    // t = t_lo + k·t_hi (5 limbs; ≤ 2^320 + 2^256)
    let mut t = [0u64; 5];
    let mut carry: u128 = 0;
    for i in 0..4 {
        let value = u128::from(src[i]) + u128::from(k) * u128::from(src[4 + i]) + carry;
        t[i] = value as u64;
        carry = value >> 64;
    }
    t[4] = carry as u64;

    // h = t >> 256 (≤ k); u = (t mod 2^256) + k·h (≤ 2^256 - 1 + k²)
    let kh = u128::from(k) * u128::from(t[4]);
    let (u, c) = wide_add256(&[t[0], t[1], t[2], t[3]], &[kh as u64, (kh >> 64) as u64, 0, 0]);

    // dst = (u mod 2^256) + k·c; c ∈ {0, 1} and k² + k < 2^128, so this never carries.
    let (dst, overflow) = wide_add256(&u, &[if c != 0 { k } else { 0 }, 0, 0, 0]);
    debug_assert_eq!(overflow, 0);
    dst
}

/// `lhs + rhs` folded modulo `2^256 - k`: the exact 257-bit sum, with its
/// carry-out folded back in as `k·carry`. Congruent to `lhs + rhs` and always
/// below `2^256`, but not fully canonicalized (as for [`wide_redc256`]).
///
/// Defined as [`wide_redc256`] applied to the 257-bit sum, which makes it
/// *bit-identical* to an `add256` whose carry-out is placed in the fifth limb
/// of a 512-bit buffer followed by a `redc256` over that buffer — the exact
/// two-instruction sequence this fused operation replaces.
///
/// Only two folds are ever needed: the second fold's `carry` can only be set
/// when the value is below `k`, so adding `k` again cannot carry for any
/// `k < 2^255` (and `k < 2^64` here).
#[inline]
pub fn wide_add256_redc256(lhs: &[u64; 4], rhs: &[u64; 4], k: u64) -> [u64; 4] {
    let (sum, carry) = wide_add256(lhs, rhs);
    wide_redc256(&[sum[0], sum[1], sum[2], sum[3], carry, 0, 0, 0], k)
}

/// `lhs - rhs` folded modulo `2^256 - k`: the difference with its borrow-out
/// folded back in by subtracting `k·borrow` (since `-2^256 ≡ -k`). Congruent
/// to `lhs - rhs` and always below `2^256`, but not fully canonicalized.
///
/// The first fold can itself borrow (when the difference is tiny), which leaves
/// the value within `k` of `2^256`, so the second fold can never borrow for any
/// `k < 2^255`: `borrow₂ = 1` implies `r₁ < k`, hence
/// `r₂ = r₁ - k + 2^256 ≥ 2^256 - k` and `r₂ - k ≥ 2^256 - 2k`, which needs
/// `2k > 2^256` to borrow.
#[inline]
pub fn wide_sub256_redc256(lhs: &[u64; 4], rhs: &[u64; 4], k: u64) -> [u64; 4] {
    let (r, borrow) = wide_sub256(lhs, rhs);
    let (r, borrow) = wide_sub256(&r, &[if borrow != 0 { k } else { 0 }, 0, 0, 0]);
    let (dst, borrow) = wide_sub256(&r, &[if borrow != 0 { k } else { 0 }, 0, 0, 0]);
    debug_assert_eq!(borrow, 0);
    dst
}

#[test]
fn test_wide_arithmetic() {
    // mul256: (2^256 - 1)² = 2^512 - 2^257 + 1
    let max = [u64::MAX; 4];
    let product = wide_mul256(&max, &max);
    assert_eq!(product, [1, 0, 0, 0, u64::MAX - 1, u64::MAX, u64::MAX, u64::MAX]);

    // add256 carry-out, sub256 borrow-out
    let one = [1, 0, 0, 0];
    assert_eq!(wide_add256(&max, &one), ([0, 0, 0, 0], 1));
    assert_eq!(wide_sub256(&[0, 0, 0, 0], &one), (max, 1));

    // mul256_by_u64: (2^256 - 1) × 2^64 - 1... use max × max_limb
    let (lo, hi) = wide_mul256_by_u64(&max, u64::MAX);
    // (2^256 - 1)(2^64 - 1) = 2^320 - 2^256 - 2^64 + 1
    assert_eq!(lo, [1, u64::MAX, u64::MAX, u64::MAX]);
    assert_eq!(hi, u64::MAX - 1);

    // redc256: k = 0 degenerates to src mod 2^256
    let src = [1, 2, 3, 4, 5, 6, 7, 8];
    assert_eq!(wide_redc256(&src, 0), [1, 2, 3, 4]);

    // redc256 with k = 38 (2^255 - 19 doubled modulus): verify congruence via a
    // small case: src = 2^256 → result must be ≡ 38 (mod 2^256 - 38).
    let two_pow_256 = [0, 0, 0, 0, 1, 0, 0, 0];
    assert_eq!(wide_redc256(&two_pow_256, 38), [38, 0, 0, 0]);

    // redc256 never returns >= 2^256 even for all-ones input with the largest k.
    let all_ones = [u64::MAX; 8];
    let _ = wide_redc256(&all_ones, u64::MAX);
}

#[test]
fn test_wide_arithmetic_fused_folds() {
    // `2^256 - k` as five limbs (k = 0 needs the fifth), times `m`, added to a
    // 256-bit value. Used to check congruence without a full bignum modulo.
    fn plus_multiples_of_modulus(value: &[u64; 4], k: u64, m: u32) -> [u64; 5] {
        let modulus: [u64; 5] = if k == 0 {
            [0, 0, 0, 0, 1]
        } else {
            let (low, _) = wide_sub256(&[0, 0, 0, 0], &[k, 0, 0, 0]);
            [low[0], low[1], low[2], low[3], 0]
        };

        let mut acc = [value[0], value[1], value[2], value[3], 0];
        for _ in 0..m {
            let (low, carry) = wide_add256(&[acc[0], acc[1], acc[2], acc[3]], &[modulus[0], modulus[1], modulus[2], modulus[3]]);
            acc = [low[0], low[1], low[2], low[3], acc[4] + carry + modulus[4]];
        }
        acc
    }

    // `expected == result + m·(2^256 - k)` for some small m, i.e. the result is
    // congruent to the true value and the fold consumed at most two moduli.
    fn assert_congruent(what: &str, result: &[u64; 4], expected: &[u64; 5], k: u64) {
        for m in 0..=2 {
            if plus_multiples_of_modulus(result, k, m) == *expected {
                return;
            }
        }
        panic!("{what}: {result:x?} is not congruent to {expected:x?} modulo 2^256 - {k:#x}");
    }

    // The recompiler's kernel form: two conditional folds of k, with no
    // multiply anywhere (carry ∈ {0, 1} makes k·carry a conditional move).
    // Must agree with the redc256-defined semantics bit for bit.
    fn add_two_folds(lhs: &[u64; 4], rhs: &[u64; 4], k: u64) -> [u64; 4] {
        let (r, carry) = wide_add256(lhs, rhs);
        let (r, carry) = wide_add256(&r, &[if carry != 0 { k } else { 0 }, 0, 0, 0]);
        let (r, carry) = wide_add256(&r, &[if carry != 0 { k } else { 0 }, 0, 0, 0]);
        assert_eq!(carry, 0, "the third fold must never carry");
        r
    }

    let max = [u64::MAX; 4];
    let zero = [0u64; 4];
    let one = [1u64, 0, 0, 0];
    // 2^256 - 5: subtracting it from zero leaves a difference small enough to
    // trigger the second borrow fold.
    let almost_modulus = [u64::MAX - 4, u64::MAX, u64::MAX, u64::MAX];
    let a = [0xdeadbeef12345678, 0x9e3779b97f4a7c15, 0xfedcba9876543210, 0x0123456789abcdef];
    let b = [0x243f6a8885a308d3, u64::MAX, 0, 0x082efa98ec4e6c89];

    for k in [0, 38, (1 << 32) + 977, u64::MAX] {
        for (lhs, rhs) in [
            (max, one),
            (max, max),
            (zero, one),
            (zero, almost_modulus),
            (a, b),
            (b, a),
            (zero, zero),
        ] {
            let sum = wide_add256_redc256(&lhs, &rhs, k);
            assert_eq!(sum, add_two_folds(&lhs, &rhs, k), "add k = {k:#x}");
            let (low, carry) = wide_add256(&lhs, &rhs);
            assert_congruent("add", &sum, &[low[0], low[1], low[2], low[3], carry], k);

            // `difference + rhs` must be congruent to `lhs`: same check, with
            // everything kept non-negative.
            let difference = wide_sub256_redc256(&lhs, &rhs, k);
            let (low, carry) = wide_add256(&difference, &rhs);
            assert_congruent("sub", &lhs, &[low[0], low[1], low[2], low[3], carry], k);
        }
    }

    // Hand-computed cases with the curve25519 modulus (k = 38, i.e. 2p).
    // (2^256 - 1) + 1 = 2^256 ≡ 38.
    assert_eq!(wide_add256_redc256(&max, &one, 38), [38, 0, 0, 0]);
    // 2·(2^256 - 1) = 2^257 - 2 ≡ 2·38 - 2 = 74; needs both folds.
    assert_eq!(wide_add256_redc256(&max, &max, 38), [74, 0, 0, 0]);
    // 0 - 1 ≡ -1 ≡ 2^256 - 39.
    assert_eq!(wide_sub256_redc256(&zero, &one, 38), [u64::MAX - 38, u64::MAX, u64::MAX, u64::MAX]);
    // 0 - (2^256 - 5) ≡ 5 - 38 ≡ 2^256 - 71; needs both folds.
    assert_eq!(
        wide_sub256_redc256(&zero, &almost_modulus, 38),
        [u64::MAX - 70, u64::MAX, u64::MAX, u64::MAX]
    );

    // k = 0 degenerates to plain modular arithmetic over 2^256.
    assert_eq!(wide_add256_redc256(&max, &one, 0), zero);
    assert_eq!(wide_sub256_redc256(&zero, &one, 0), max);
}

#[test]
fn test_div_rem() {
    assert_eq!(divu(10, 2), 5);
    assert_eq!(divu(10, 0), u32::MAX);

    assert_eq!(divu64(10, 2), 5);
    assert_eq!(divu64(10, 0), u64::MAX);

    assert_eq!(div(10, 2), 5);
    assert_eq!(div(10, 0), -1);
    assert_eq!(div(i32::MIN, -1), i32::MIN);

    assert_eq!(div64(10, 2), 5);
    assert_eq!(div64(10, 0), -1);
    assert_eq!(div64(i64::MIN, -1), i64::MIN);

    assert_eq!(remu(10, 9), 1);
    assert_eq!(remu(10, 5), 0);
    assert_eq!(remu(10, 0), 10);

    assert_eq!(remu64(10, 9), 1);
    assert_eq!(remu64(10, 5), 0);
    assert_eq!(remu64(10, 0), 10);

    assert_eq!(rem(10, 9), 1);
    assert_eq!(rem(10, 5), 0);
    assert_eq!(rem(10, 0), 10);
    assert_eq!(rem(i32::MIN, -1), 0);

    assert_eq!(rem64(10, 9), 1);
    assert_eq!(rem64(10, 5), 0);
    assert_eq!(rem64(10, 0), 10);
    assert_eq!(rem64(i64::MIN, -1), 0);
}
