#![no_std]
#![no_main]

include!("../../bench-common.rs");

#[cfg(target_env = "polkavm")]
polkavm_derive::min_stack_size!(256 * 1024);

// The `bls12_377_multi_miller_loop` and `bls12_377_final_exponentiation` host
// functions. 377-bit base field, 6 limbs — the same shape as bls12-381, so this
// row separates curve choice from field size.
type Curve = ark_bls12_377::Bls12_377;

include!("../../ec-pairing-bench-common.rs");
