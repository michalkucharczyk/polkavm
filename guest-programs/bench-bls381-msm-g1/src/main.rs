#![no_std]
#![no_main]

include!("../../bench-common.rs");

// 384-bit field arithmetic plus Pippenger's bucket accumulation needs far more
// than the default 8 KiB of stack.
#[cfg(target_env = "polkavm")]
polkavm_derive::min_stack_size!(256 * 1024);

// The `bls12_381_msm_g1` host function: `sp_crypto_ec_utils::utils::msm_sw`
// decodes into `Vec<G1Affine>` / `Vec<ScalarField>` and calls
// `<g1::Config as SWCurveConfig>::msm(..).into_affine()`.
//
// Plain `ark-bls12-381`, not `ark-bls12-381-ext`: the hooks in the `-ext` crate
// are the host-offload plumbing, which is exactly what this benchmark exists to
// question.
use ark_bls12_381::{g1::Config, G1Affine};
use ark_ec::{short_weierstrass::SWCurveConfig, CurveGroup};

type Base = G1Affine;

fn msm_once(bases: &[Base], scalars: &[Scalar]) -> Base {
    Config::msm(bases, scalars).unwrap().into_affine()
}

fn mul_once(base: &Base, scalar: &[u64]) -> Base {
    Config::mul_affine(base, scalar).into_affine()
}

include!("../../ec-bench-common.rs");
