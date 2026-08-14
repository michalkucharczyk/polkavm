#![no_std]
#![no_main]

include!("../../bench-common.rs");

// G2 works over Fq2, so every field operation is ~3x the G1 cost and the
// intermediate points are twice the size.
#[cfg(target_env = "polkavm")]
polkavm_derive::min_stack_size!(256 * 1024);

// The `bls12_381_msm_g2` host function: `sp_crypto_ec_utils::utils::msm_sw`
// decodes into `Vec<G2Affine>` / `Vec<ScalarField>` and calls
// `<g2::Config as SWCurveConfig>::msm(..).into_affine()`.
use ark_bls12_381::{g2::Config, G2Affine};
use ark_ec::{short_weierstrass::SWCurveConfig, CurveGroup};

type Base = G2Affine;

fn msm_once(bases: &[Base], scalars: &[Scalar]) -> Base {
    Config::msm(bases, scalars).unwrap().into_affine()
}

fn mul_once(base: &Base, scalar: &[u64]) -> Base {
    Config::mul_affine(base, scalar).into_affine()
}

include!("../../ec-bench-common.rs");
