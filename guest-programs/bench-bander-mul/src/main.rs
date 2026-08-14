#![no_std]
#![no_main]

include!("../../bench-common.rs");

#[cfg(target_env = "polkavm")]
polkavm_derive::min_stack_size!(256 * 1024);

// The `ed_on_bls12_381_bandersnatch_mul` host function:
// `sp_crypto_ec_utils::utils::mul_te` calls
// `<EdwardsConfig as TECurveConfig>::mul_affine(&base, &scalar).into_affine()`.
//
// Note the twisted Edwards `mul_affine` is a plain double-and-add over
// `BitIteratorBE::without_leading_zeros`, with no windowing — unlike the short
// Weierstrass side, which uses `sw_double_and_add_affine`.
#[path = "../../ec-fixtures.rs"]
mod fixtures;

use ark_ec::{twisted_edwards::TECurveConfig, AffineRepr, CurveGroup};
use ark_ed_on_bls12_381_bandersnatch::{EdwardsAffine, EdwardsConfig, Fr};
use ark_ff::PrimeField;

struct State;
define_benchmark! {
    heap_size = 64 * 1024,
    state = State,
}

fn mul_once(base: &EdwardsAffine, scalar: &[u64]) -> EdwardsAffine {
    EdwardsConfig::mul_affine(base, scalar).into_affine()
}

fn benchmark_initialize(_state: &mut State) {
    let generator = EdwardsAffine::generator();
    assert!(mul_once(&generator, &[2]) == (generator.into_group() + generator).into_affine());
    assert!(mul_once(&generator, Fr::MODULUS.as_ref()) == EdwardsAffine::zero());
}

fn benchmark_run(_state: &mut State) {
    use core::hint::black_box;

    let scalar = fixtures::seed_limbs::<Fr>();
    let _ = black_box(mul_once(black_box(&EdwardsAffine::generator()), black_box(scalar.as_ref())));
}
