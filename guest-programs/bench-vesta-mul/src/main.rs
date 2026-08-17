#![no_std]
#![no_main]

include!("../../bench-common.rs");

#[cfg(target_env = "polkavm")]
polkavm_derive::min_stack_size!(256 * 1024);

// The `vesta_mul` host function: `sp_crypto_ec_utils::utils::mul_sw` calls
// `<VestaConfig as SWCurveConfig>::mul_affine(&base, &scalar).into_affine()`.
use ark_ec::{short_weierstrass::SWCurveConfig, CurveGroup};
use ark_vesta::{Affine, VestaConfig as Config};

type Base = Affine;

fn mul_once(base: &Base, scalar: &[u64]) -> Base {
    Config::mul_affine(base, scalar).into_affine()
}

include!("../../ec-mul-bench-common.rs");
