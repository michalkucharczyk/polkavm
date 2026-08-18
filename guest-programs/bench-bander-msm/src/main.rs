#![no_std]
#![no_main]

include!("../../bench-common.rs");

#[cfg(target_env = "polkavm")]
polkavm_derive::min_stack_size!(256 * 1024);

// The `ed_on_bls12_381_bandersnatch_msm` host function:
// `sp_crypto_ec_utils::utils::msm_te` decodes into `Vec<EdwardsAffine>` /
// `Vec<ScalarField>` and calls
// `<EdwardsConfig as TECurveConfig>::msm(..).into_affine()`.
//
// Bandersnatch in twisted Edwards form is the curve JAM's Safrole / ring-VRF uses,
// so this is a directly relevant row. The size sweep exists to expose Pippenger's
// sub-linear per-point scaling, not because any caller is known to need a
// particular n.
use ark_ec::{twisted_edwards::TECurveConfig, CurveGroup};
use ark_ed_on_bls12_381_bandersnatch::{EdwardsAffine, EdwardsConfig};

type Base = EdwardsAffine;

fn msm_once(bases: &[Base], scalars: &[Scalar]) -> Base {
    EdwardsConfig::msm(bases, scalars).unwrap().into_affine()
}

fn mul_once(base: &Base, scalar: &[u64]) -> Base {
    EdwardsConfig::mul_affine(base, scalar).into_affine()
}

include!("../../ec-bench-common.rs");
