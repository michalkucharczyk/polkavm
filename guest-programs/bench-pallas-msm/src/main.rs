#![no_std]
#![no_main]

include!("../../bench-common.rs");

#[cfg(target_env = "polkavm")]
polkavm_derive::min_stack_size!(256 * 1024);

// The `pallas_msm` host function: `sp_crypto_ec_utils::utils::msm_sw` calls
// `<PallasConfig as SWCurveConfig>::msm(..).into_affine()`.
//
// One of the Pasta curves, in RFC-163 alongside bls12-381 and bandersnatch.
// Short Weierstrass over a 4-limb (256-bit) field — same width as bandersnatch
// but a different curve form, so the two are not interchangeable.
use ark_ec::{short_weierstrass::SWCurveConfig, CurveGroup};
use ark_pallas::{Affine, PallasConfig as Config};

type Base = Affine;

fn msm_once(bases: &[Base], scalars: &[Scalar]) -> Base {
    Config::msm(bases, scalars).unwrap().into_affine()
}

fn mul_once(base: &Base, scalar: &[u64]) -> Base {
    Config::mul_affine(base, scalar).into_affine()
}

include!("../../ec-bench-common.rs");
