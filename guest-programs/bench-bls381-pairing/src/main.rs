#![no_std]
#![no_main]

include!("../../bench-common.rs");

// Fq12 towers plus the Miller loop's line-coefficient tables need far more than
// the default 8 KiB of stack.
#[cfg(target_env = "polkavm")]
polkavm_derive::min_stack_size!(256 * 1024);

// The `bls12_381_multi_miller_loop` and `bls12_381_final_exponentiation` host
// functions. 381-bit base field, 6 limbs.
type Curve = ark_bls12_381::Bls12_381;

include!("../../ec-pairing-bench-common.rs");
