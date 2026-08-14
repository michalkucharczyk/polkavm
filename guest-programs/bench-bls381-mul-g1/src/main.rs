#![no_std]
#![no_main]

include!("../../bench-common.rs");

#[cfg(target_env = "polkavm")]
polkavm_derive::min_stack_size!(256 * 1024);

// The `bls12_381_mul_g1` host function: `sp_crypto_ec_utils::utils::mul_sw`
// calls `<g1::Config as SWCurveConfig>::mul_affine(&base, &scalar).into_affine()`
// where the scalar arrives as `u64` limbs, not as a field element.
#[path = "../../ec-fixtures.rs"]
mod fixtures;

use ark_bls12_381::{g1::Config, Fr, G1Affine};
use ark_ec::{short_weierstrass::SWCurveConfig, AffineRepr, CurveGroup};
use ark_ff::PrimeField;

struct State;
define_benchmark! {
    heap_size = 64 * 1024,
    state = State,
}

fn mul_once(base: &G1Affine, scalar: &[u64]) -> G1Affine {
    Config::mul_affine(base, scalar).into_affine()
}

fn benchmark_initialize(_state: &mut State) {
    // Two checks the double-and-add ladder cannot pass by accident: a known
    // small multiple, and the group order annihilating the generator.
    let generator = G1Affine::generator();
    assert!(mul_once(&generator, &[2]) == (generator.into_group() + generator).into_affine());
    assert!(mul_once(&generator, Fr::MODULUS.as_ref()) == G1Affine::zero());
}

fn benchmark_run(_state: &mut State) {
    use core::hint::black_box;

    let scalar = fixtures::seed_limbs::<Fr>();
    let _ = black_box(mul_once(black_box(&G1Affine::generator()), black_box(scalar.as_ref())));
}
