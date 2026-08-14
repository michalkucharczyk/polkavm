#![no_std]
#![no_main]

include!("../../bench-common.rs");

// 761-bit base field means Fq6 towers over 12-limb elements; the Miller loop's
// line-coefficient tables are correspondingly larger.
#[cfg(target_env = "polkavm")]
polkavm_derive::min_stack_size!(256 * 1024);

// The `bw6_761_multi_miller_loop` and `bw6_761_final_exponentiation` host
// functions. 761-bit base field, 12 limbs — twice bls12-381's, which is the
// point of this row.
//
// Note its *scalar* field is bls12-377's base field, 377 bits, not the ~255 bits
// every other curve here has. That is why `ec-fixtures.rs` widens the seed past
// 32 bytes.
type Curve = ark_bw6_761::BW6_761;

include!("../../ec-pairing-bench-common.rs");
