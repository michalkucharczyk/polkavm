#![no_std]
#![no_main]

include!("../../bench-common.rs");

#[cfg(target_env = "polkavm")]
polkavm_derive::min_stack_size!(256 * 1024);

// The `ed_on_bls12_381_bandersnatch_mul` host function:
// `sp_crypto_ec_utils::utils::mul_te` calls
// `<EdwardsConfig as TECurveConfig>::mul_affine(&base, &scalar).into_affine()`.
//
// The twisted Edwards `mul_affine` is a plain double-and-add over
// `BitIteratorBE::without_leading_zeros`, with no windowing — unlike the short
// Weierstrass side, which uses `sw_double_and_add_affine`. That difference is why
// the SW curves in this suite (pallas, vesta, bls12-381) are not interchangeable
// with this row.
use ark_ec::{twisted_edwards::TECurveConfig, CurveGroup};
use ark_ed_on_bls12_381_bandersnatch::{EdwardsAffine, EdwardsConfig};

type Base = EdwardsAffine;

fn mul_once(base: &Base, scalar: &[u64]) -> Base {
    EdwardsConfig::mul_affine(base, scalar).into_affine()
}

include!("../../ec-mul-bench-common.rs");
