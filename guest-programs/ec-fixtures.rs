//! Deterministic fixtures for the `sp_crypto_ec_utils` (arkworks elliptic
//! curve) benchmarks. No RNG: everything derives from one fixed seed, so the
//! PVM guest and the host libraries measure byte-identical work and runs stay
//! comparable across machines.

#![allow(dead_code)]

use alloc::vec::Vec;
use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::{BigInteger, PrimeField, Zero};

// Arbitrary fixed pattern, reduced modulo the scalar field.
//
// Full-width scalars are not cosmetic here: `msm_bigint_wnaf` skips a bucket
// addition for every zero wNAF digit, and `TECurveConfig::mul_affine` iterates
// `BitIteratorBE::without_leading_zeros`. Narrow scalars would report a
// fraction of the real cost, which `seed_scalar`'s assertion guards against.
const SEED: [u8; 32] = [
    0x9e, 0x37, 0x79, 0xb9, 0x7f, 0x4a, 0x7c, 0x15, 0xbf, 0x58, 0x47, 0x6d, 0x1c, 0xe4, 0xe5, 0xb9,
    0x94, 0xd0, 0x49, 0xbb, 0x13, 0x37, 0x11, 0xeb, 0x2f, 0xe3, 0x5a, 0x66, 0xd5, 0xa4, 0xf0, 0x6c,
];

pub fn seed_scalar<F: PrimeField>() -> F {
    let scalar = F::from_le_bytes_mod_order(&SEED);
    assert!(scalar.into_bigint().num_bits() + 8 >= F::MODULUS_BIT_SIZE);
    scalar
}

/// `count` full-width scalars: `seed, seed^2, ..., seed^count`.
pub fn scalars<F: PrimeField>(count: usize) -> Vec<F> {
    let seed = seed_scalar::<F>();
    let mut scalar = F::one();
    (0..count)
        .map(|_| {
            scalar *= seed;
            scalar
        })
        .collect()
}

/// The seed scalar as `u64` limbs, the form `mul_affine` takes.
pub fn seed_limbs<F: PrimeField>() -> F::BigInt {
    seed_scalar::<F>().into_bigint()
}

/// `count` distinct bases `G, 2G, ..., count*G`.
///
/// Built by repeated addition and normalized in one batch rather than by
/// `count` scalar multiplications: `initialize()` runs once per spawned
/// instance, and the harness spawns a fresh instance for every outer iteration.
pub fn bases<A: AffineRepr>(count: usize) -> Vec<A> {
    let generator = A::generator();
    let mut point = A::Group::zero();
    let projective: Vec<A::Group> = (0..count)
        .map(|_| {
            point += generator;
            point
        })
        .collect();

    A::Group::normalize_batch(&projective)
}
