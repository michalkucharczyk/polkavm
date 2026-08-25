#![no_std]
#![no_main]

include!("../../bench-common.rs");

mod asm_fq_mul;

// The bls12-381 base-field Montgomery multiply, hand-scheduled for the x86 the
// recompiler emits rather than for the PVM instruction count. Compare against
// `bench-fq381mul` (LLVM's codegen) and `bench-fq381mul-asm` (the same kernel
// scheduled for the smallest instruction count) - all three run the identical
// harness.
use ark_bls12_381::Fq;

#[inline(always)]
fn fq_mul(a: &mut Fq, b: &Fq) {
    asm_fq_mul::mul_assign(a, b);
}

include!("../../fq-mul-bench-common.rs");
