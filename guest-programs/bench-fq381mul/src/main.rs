#![no_std]
#![no_main]

include!("../../bench-common.rs");

// LLVM's codegen for the bls12-381 base-field Montgomery multiply, i.e. the
// inner kernel of every arkworks operation on that curve. The baseline
// `bench-fq381mul-asm` is measured against; keep the two harnesses identical.
//
// `inline(never)` pins the kernel to one call site so the static and executed
// instruction counts are attributable, and matches what the asm crate's
// `extern "C"` kernel does whether we ask for it or not.
use ark_bls12_381::Fq;

#[inline(never)]
fn fq_mul(a: &mut Fq, b: &Fq) {
    *a *= b;
}

include!("../../fq-mul-bench-common.rs");
