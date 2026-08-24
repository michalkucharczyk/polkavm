#![no_std]
#![no_main]

include!("../../bench-common.rs");

mod asm_fq_mul;

// The bls12-381 base-field Montgomery multiply with a hand-scheduled RISC-V
// assembly kernel on riscv64; other targets fall back to ark-ff (see
// `asm_fq_mul.rs`). Compare against `bench-fq381mul`, which is the same
// harness over LLVM's codegen for the same kernel.
use ark_bls12_381::Fq;

#[inline(never)]
fn fq_mul(a: &mut Fq, b: &Fq) {
    asm_fq_mul::mul_assign(a, b);
}

include!("../../fq-mul-bench-common.rs");
