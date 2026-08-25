// The bls12-381 base-field Montgomery multiply with a hand-scheduled RISC-V
// assembly kernel (`fq_mul.S`, generated), version 2.
//
// v1 (`bench-fq381mul-asm`) minimised PVM instructions: it split ark-ff's
// interleaved CIOS into an a*b pass and an m*p pass so the live set fitted
// rv64e's twelve registers, which cut gas 25.6% and lost ~5% of wall clock on
// Zen 4. One long carry chain replaced two overlapping ones, so the host's
// dispatch width went unused.
//
// This kernel targets the emitted x86 instead. It folds both products of a
// limb into one pass, and it regroups which carry bits belong to which of the
// two carry chains so that only one addition per limb sits on the critical
// path: measured 4 cycles per limb of chain latency against v1's 5 per
// multiply-accumulate, with the same instruction count. See
// `.agent/worklog/fqmul-emitted-x86-analysis.md` for the derivation.
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
    fn fq381_mul_asm2(a: *mut u64, b: *const u64, pool: *const u64);
}

// `inline(always)`: the kernel is already an out-of-line `extern "C"` function,
// so inlining the shim leaves exactly one call per multiply - the same shape as
// `bench-fq381mul`'s `inline(never)` kernel, which is what makes the gas
// numbers comparable.
#[cfg(target_arch = "riscv64")]
#[inline(always)]
pub fn mul_assign(a: &mut Fq, b: &Fq) {
    unsafe { fq381_mul_asm2(a.0 .0.as_mut_ptr(), b.0 .0.as_ptr(), POOL.as_ptr()) }
}

#[cfg(not(target_arch = "riscv64"))]
#[inline(always)]
pub fn mul_assign(a: &mut Fq, b: &Fq) {
    *a *= b;
}
