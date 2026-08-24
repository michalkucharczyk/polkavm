// The bls12-381 base-field Montgomery multiply with a hand-scheduled RISC-V
// assembly kernel (`fq_mul.S`, generated).
//
// Rationale: the kernel needs eight live 64-bit words of state (six
// accumulator limbs plus two carry chains) on top of two operand pointers and
// a constant pointer. rv64e has twelve usable registers - it fits, but only
// just, and LLVM does not find the allocation: of the 944 instructions it emits
// for this function, 326 are memory operations where about 96 are necessary, so
// a quarter of the multiply is spill traffic. `fq_mul.S` splits the a*b row
// from the m*p reduction so that b_i and m are never live together, pins the
// accumulator for the whole function, and rotates the accumulator's register
// names instead of moving limbs after the Montgomery shift.
//
// On non-riscv64 targets (host builds, riscv32) this falls back to ark-ff so
// the crate still builds and the benchmark still measures a correct multiply -
// see SKIP_RV32 in build-benchmarks.sh for why no 32-bit blob is produced.

use ark_bls12_381::Fq;

#[cfg(target_arch = "riscv64")]
core::arch::global_asm!(include_str!("fq_mul.S"));

/// `[p0..p5, INV]`, taken from ark-ff itself so the assembly cannot drift from
/// the reference it is checked against.
#[cfg(target_arch = "riscv64")]
static POOL: [u64; 7] = {
    use ark_bls12_381::FqConfig;
    use ark_ff::MontConfig;
    [
        FqConfig::MODULUS.0[0],
        FqConfig::MODULUS.0[1],
        FqConfig::MODULUS.0[2],
        FqConfig::MODULUS.0[3],
        FqConfig::MODULUS.0[4],
        FqConfig::MODULUS.0[5],
        FqConfig::INV,
    ]
};

#[cfg(target_arch = "riscv64")]
extern "C" {
    /// `a = a * b * R^-1 mod p`, fully reduced. Six little-endian limbs each.
    fn fq381_mul_asm(a: *mut u64, b: *const u64, pool: *const u64);
}

// `inline(always)`: the kernel is already an out-of-line `extern "C"` function,
// so inlining the shim leaves exactly one call per multiply - the same shape as
// `bench-fq381mul`'s `inline(never)` kernel, which is what makes the gas
// numbers comparable.
#[cfg(target_arch = "riscv64")]
#[inline(always)]
pub fn mul_assign(a: &mut Fq, b: &Fq) {
    unsafe { fq381_mul_asm(a.0 .0.as_mut_ptr(), b.0 .0.as_ptr(), POOL.as_ptr()) }
}

#[cfg(not(target_arch = "riscv64"))]
#[inline(always)]
pub fn mul_assign(a: &mut Fq, b: &Fq) {
    *a *= b;
}
