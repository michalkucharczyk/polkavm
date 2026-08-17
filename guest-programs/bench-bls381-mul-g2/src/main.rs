#![no_std]
#![no_main]

include!("../../bench-common.rs");

#[cfg(target_env = "polkavm")]
polkavm_derive::min_stack_size!(256 * 1024);

// The `bls12_381_mul_g2` host function: `sp_crypto_ec_utils::utils::mul_sw`
// calls `<g2::Config as SWCurveConfig>::mul_affine(&base, &scalar).into_affine()`.
//
// The only row in the suite that measures a scalar multiplication over an
// extension field (Fq2), so it is not predictable from `mul_g1` without
// borrowing the G1->G2 ratio from the MSM rows — a different algorithm.
use ark_bls12_381::{g2::Config, G2Affine};
use ark_ec::{short_weierstrass::SWCurveConfig, CurveGroup};

type Base = G2Affine;

fn mul_once(base: &Base, scalar: &[u64]) -> Base {
    Config::mul_affine(base, scalar).into_affine()
}

include!("../../ec-mul-bench-common.rs");
