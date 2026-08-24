// bls12-381 base-field (Fq) Montgomery multiply for PVM (rv64e).
// Generated - see .agent/worklog/fqmul-asm-results.md for the derivation.
//
// fn fq381_mul_asm(a: *mut u64 [a0], b: *const u64 [a1], pool: *const u64 [a2])
//
// a and b are six little-endian 64-bit limbs in Montgomery form; the product
// (fully reduced, < p) is written back over a. `pool` is [p0..p5, INV] where
// INV = -p^-1 mod 2^64. Bit-identical to ark-ff 0.5's
// MontBackend::<FqConfig, 6>::mul_assign for every input, in range or not.
//
// Algorithm: the no-carry CIOS Montgomery multiply ark-ff runs, but with the
// a*b row and the m*p reduction as two separate passes instead of interleaved.
// Both shapes need eight live words of state (six accumulator limbs plus two
// carries, or seven plus one); splitting them means b_i is dead while m is
// live, which frees the register that makes the whole thing fit in rv64e's
// twelve. LLVM cannot find that allocation and spills ~230 times per multiply.
//
// Register budget, exactly 12 (x16-x31 do not exist on rv64e):
//   a1 a3 a4 a5 s0 s1  accumulator t[0..5], resident end to end
//   a0                 &a[0], also the destination
//   a2                 &pool[0]
//   t1                 b_i (pass one) / m (pass two)
//   t2                 one short-lived temp: operand limb -> lo -> carry bit
//   t0 ra              the carry, ping-ponging so no step needs a mv
// The accumulator rotates one register per outer iteration rather than moving
// limbs down after the shift; six iterations over six registers comes back to
// the identity, so the whole function is unrolled and there are no mvs at all.
// b's pointer is the one thing that does not fit and lives on the stack.
//
// Not constant-time: the final conditional subtraction branches, as does
// ark-ff's. This is benchmark code.
.section .text.fq381_mul_asm,"ax"
.globl fq381_mul_asm
fq381_mul_asm:
    addi  sp, sp, -32
    sd    ra, 8(sp)
    sd    s0, 16(sp)
    sd    s1, 24(sp)
    sd    a1, 0(sp)
    // b's pointer is spilled: 12 registers is the whole rv64e budget
    // --- limb 0 of b: t += a * b[0], then fold in m*p and shift ---
    ld    t2, 0(sp)
    ld    t1, 0(t2)
    ld    t2, 0(a0)
    mulhu ra, t2, t1
    mul   a1, t2, t1
    ld    t2, 8(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   a3, t2, ra
    sltu  t2, a3, t2
    add   t0, t0, t2
    ld    t2, 16(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   a4, t2, t0
    sltu  t2, a4, t2
    add   ra, ra, t2
    ld    t2, 24(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   a5, t2, ra
    sltu  t2, a5, t2
    add   t0, t0, t2
    ld    t2, 32(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   s0, t2, t0
    sltu  t2, s0, t2
    add   ra, ra, t2
    ld    t2, 40(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   s1, t2, ra
    sltu  t2, s1, t2
    add   t0, t0, t2
    ld    t2, 48(a2)
    mul   t1, a1, t2
    ld    t2, 0(a2)
    mulhu ra, t1, t2
    snez  t2, a1
    add   ra, ra, t2
    ld    t2, 8(a2)
    mulhu a1, t1, t2
    mul   t2, t1, t2
    add   a3, a3, ra
    sltu  ra, a3, ra
    add   a3, a3, t2
    sltu  t2, a3, t2
    add   a1, a1, ra
    add   a1, a1, t2
    ld    t2, 16(a2)
    mulhu ra, t1, t2
    mul   t2, t1, t2
    add   a4, a4, a1
    sltu  a1, a4, a1
    add   a4, a4, t2
    sltu  t2, a4, t2
    add   ra, ra, a1
    add   ra, ra, t2
    ld    t2, 24(a2)
    mulhu a1, t1, t2
    mul   t2, t1, t2
    add   a5, a5, ra
    sltu  ra, a5, ra
    add   a5, a5, t2
    sltu  t2, a5, t2
    add   a1, a1, ra
    add   a1, a1, t2
    ld    t2, 32(a2)
    mulhu ra, t1, t2
    mul   t2, t1, t2
    add   s0, s0, a1
    sltu  a1, s0, a1
    add   s0, s0, t2
    sltu  t2, s0, t2
    add   ra, ra, a1
    add   ra, ra, t2
    ld    t2, 40(a2)
    mulhu a1, t1, t2
    mul   t2, t1, t2
    add   s1, s1, ra
    sltu  ra, s1, ra
    add   s1, s1, t2
    sltu  t2, s1, t2
    add   a1, a1, ra
    add   a1, a1, t2
    add   a1, a1, t0
    // --- limb 1 of b: t += a * b[1], then fold in m*p and shift ---
    ld    t2, 0(sp)
    ld    t1, 8(t2)
    ld    t2, 0(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   a3, a3, t2
    sltu  t2, a3, t2
    add   ra, ra, t2
    ld    t2, 8(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   a4, a4, ra
    sltu  ra, a4, ra
    add   a4, a4, t2
    sltu  t2, a4, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 16(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   a5, a5, t0
    sltu  t0, a5, t0
    add   a5, a5, t2
    sltu  t2, a5, t2
    add   ra, ra, t0
    add   ra, ra, t2
    ld    t2, 24(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   s0, s0, ra
    sltu  ra, s0, ra
    add   s0, s0, t2
    sltu  t2, s0, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 32(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   s1, s1, t0
    sltu  t0, s1, t0
    add   s1, s1, t2
    sltu  t2, s1, t2
    add   ra, ra, t0
    add   ra, ra, t2
    ld    t2, 40(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   a1, a1, ra
    sltu  ra, a1, ra
    add   a1, a1, t2
    sltu  t2, a1, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 48(a2)
    mul   t1, a3, t2
    ld    t2, 0(a2)
    mulhu ra, t1, t2
    snez  t2, a3
    add   ra, ra, t2
    ld    t2, 8(a2)
    mulhu a3, t1, t2
    mul   t2, t1, t2
    add   a4, a4, ra
    sltu  ra, a4, ra
    add   a4, a4, t2
    sltu  t2, a4, t2
    add   a3, a3, ra
    add   a3, a3, t2
    ld    t2, 16(a2)
    mulhu ra, t1, t2
    mul   t2, t1, t2
    add   a5, a5, a3
    sltu  a3, a5, a3
    add   a5, a5, t2
    sltu  t2, a5, t2
    add   ra, ra, a3
    add   ra, ra, t2
    ld    t2, 24(a2)
    mulhu a3, t1, t2
    mul   t2, t1, t2
    add   s0, s0, ra
    sltu  ra, s0, ra
    add   s0, s0, t2
    sltu  t2, s0, t2
    add   a3, a3, ra
    add   a3, a3, t2
    ld    t2, 32(a2)
    mulhu ra, t1, t2
    mul   t2, t1, t2
    add   s1, s1, a3
    sltu  a3, s1, a3
    add   s1, s1, t2
    sltu  t2, s1, t2
    add   ra, ra, a3
    add   ra, ra, t2
    ld    t2, 40(a2)
    mulhu a3, t1, t2
    mul   t2, t1, t2
    add   a1, a1, ra
    sltu  ra, a1, ra
    add   a1, a1, t2
    sltu  t2, a1, t2
    add   a3, a3, ra
    add   a3, a3, t2
    add   a3, a3, t0
    // --- limb 2 of b: t += a * b[2], then fold in m*p and shift ---
    ld    t2, 0(sp)
    ld    t1, 16(t2)
    ld    t2, 0(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   a4, a4, t2
    sltu  t2, a4, t2
    add   ra, ra, t2
    ld    t2, 8(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   a5, a5, ra
    sltu  ra, a5, ra
    add   a5, a5, t2
    sltu  t2, a5, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 16(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   s0, s0, t0
    sltu  t0, s0, t0
    add   s0, s0, t2
    sltu  t2, s0, t2
    add   ra, ra, t0
    add   ra, ra, t2
    ld    t2, 24(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   s1, s1, ra
    sltu  ra, s1, ra
    add   s1, s1, t2
    sltu  t2, s1, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 32(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   a1, a1, t0
    sltu  t0, a1, t0
    add   a1, a1, t2
    sltu  t2, a1, t2
    add   ra, ra, t0
    add   ra, ra, t2
    ld    t2, 40(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   a3, a3, ra
    sltu  ra, a3, ra
    add   a3, a3, t2
    sltu  t2, a3, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 48(a2)
    mul   t1, a4, t2
    ld    t2, 0(a2)
    mulhu ra, t1, t2
    snez  t2, a4
    add   ra, ra, t2
    ld    t2, 8(a2)
    mulhu a4, t1, t2
    mul   t2, t1, t2
    add   a5, a5, ra
    sltu  ra, a5, ra
    add   a5, a5, t2
    sltu  t2, a5, t2
    add   a4, a4, ra
    add   a4, a4, t2
    ld    t2, 16(a2)
    mulhu ra, t1, t2
    mul   t2, t1, t2
    add   s0, s0, a4
    sltu  a4, s0, a4
    add   s0, s0, t2
    sltu  t2, s0, t2
    add   ra, ra, a4
    add   ra, ra, t2
    ld    t2, 24(a2)
    mulhu a4, t1, t2
    mul   t2, t1, t2
    add   s1, s1, ra
    sltu  ra, s1, ra
    add   s1, s1, t2
    sltu  t2, s1, t2
    add   a4, a4, ra
    add   a4, a4, t2
    ld    t2, 32(a2)
    mulhu ra, t1, t2
    mul   t2, t1, t2
    add   a1, a1, a4
    sltu  a4, a1, a4
    add   a1, a1, t2
    sltu  t2, a1, t2
    add   ra, ra, a4
    add   ra, ra, t2
    ld    t2, 40(a2)
    mulhu a4, t1, t2
    mul   t2, t1, t2
    add   a3, a3, ra
    sltu  ra, a3, ra
    add   a3, a3, t2
    sltu  t2, a3, t2
    add   a4, a4, ra
    add   a4, a4, t2
    add   a4, a4, t0
    // --- limb 3 of b: t += a * b[3], then fold in m*p and shift ---
    ld    t2, 0(sp)
    ld    t1, 24(t2)
    ld    t2, 0(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   a5, a5, t2
    sltu  t2, a5, t2
    add   ra, ra, t2
    ld    t2, 8(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   s0, s0, ra
    sltu  ra, s0, ra
    add   s0, s0, t2
    sltu  t2, s0, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 16(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   s1, s1, t0
    sltu  t0, s1, t0
    add   s1, s1, t2
    sltu  t2, s1, t2
    add   ra, ra, t0
    add   ra, ra, t2
    ld    t2, 24(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   a1, a1, ra
    sltu  ra, a1, ra
    add   a1, a1, t2
    sltu  t2, a1, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 32(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   a3, a3, t0
    sltu  t0, a3, t0
    add   a3, a3, t2
    sltu  t2, a3, t2
    add   ra, ra, t0
    add   ra, ra, t2
    ld    t2, 40(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   a4, a4, ra
    sltu  ra, a4, ra
    add   a4, a4, t2
    sltu  t2, a4, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 48(a2)
    mul   t1, a5, t2
    ld    t2, 0(a2)
    mulhu ra, t1, t2
    snez  t2, a5
    add   ra, ra, t2
    ld    t2, 8(a2)
    mulhu a5, t1, t2
    mul   t2, t1, t2
    add   s0, s0, ra
    sltu  ra, s0, ra
    add   s0, s0, t2
    sltu  t2, s0, t2
    add   a5, a5, ra
    add   a5, a5, t2
    ld    t2, 16(a2)
    mulhu ra, t1, t2
    mul   t2, t1, t2
    add   s1, s1, a5
    sltu  a5, s1, a5
    add   s1, s1, t2
    sltu  t2, s1, t2
    add   ra, ra, a5
    add   ra, ra, t2
    ld    t2, 24(a2)
    mulhu a5, t1, t2
    mul   t2, t1, t2
    add   a1, a1, ra
    sltu  ra, a1, ra
    add   a1, a1, t2
    sltu  t2, a1, t2
    add   a5, a5, ra
    add   a5, a5, t2
    ld    t2, 32(a2)
    mulhu ra, t1, t2
    mul   t2, t1, t2
    add   a3, a3, a5
    sltu  a5, a3, a5
    add   a3, a3, t2
    sltu  t2, a3, t2
    add   ra, ra, a5
    add   ra, ra, t2
    ld    t2, 40(a2)
    mulhu a5, t1, t2
    mul   t2, t1, t2
    add   a4, a4, ra
    sltu  ra, a4, ra
    add   a4, a4, t2
    sltu  t2, a4, t2
    add   a5, a5, ra
    add   a5, a5, t2
    add   a5, a5, t0
    // --- limb 4 of b: t += a * b[4], then fold in m*p and shift ---
    ld    t2, 0(sp)
    ld    t1, 32(t2)
    ld    t2, 0(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   s0, s0, t2
    sltu  t2, s0, t2
    add   ra, ra, t2
    ld    t2, 8(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   s1, s1, ra
    sltu  ra, s1, ra
    add   s1, s1, t2
    sltu  t2, s1, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 16(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   a1, a1, t0
    sltu  t0, a1, t0
    add   a1, a1, t2
    sltu  t2, a1, t2
    add   ra, ra, t0
    add   ra, ra, t2
    ld    t2, 24(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   a3, a3, ra
    sltu  ra, a3, ra
    add   a3, a3, t2
    sltu  t2, a3, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 32(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   a4, a4, t0
    sltu  t0, a4, t0
    add   a4, a4, t2
    sltu  t2, a4, t2
    add   ra, ra, t0
    add   ra, ra, t2
    ld    t2, 40(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   a5, a5, ra
    sltu  ra, a5, ra
    add   a5, a5, t2
    sltu  t2, a5, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 48(a2)
    mul   t1, s0, t2
    ld    t2, 0(a2)
    mulhu ra, t1, t2
    snez  t2, s0
    add   ra, ra, t2
    ld    t2, 8(a2)
    mulhu s0, t1, t2
    mul   t2, t1, t2
    add   s1, s1, ra
    sltu  ra, s1, ra
    add   s1, s1, t2
    sltu  t2, s1, t2
    add   s0, s0, ra
    add   s0, s0, t2
    ld    t2, 16(a2)
    mulhu ra, t1, t2
    mul   t2, t1, t2
    add   a1, a1, s0
    sltu  s0, a1, s0
    add   a1, a1, t2
    sltu  t2, a1, t2
    add   ra, ra, s0
    add   ra, ra, t2
    ld    t2, 24(a2)
    mulhu s0, t1, t2
    mul   t2, t1, t2
    add   a3, a3, ra
    sltu  ra, a3, ra
    add   a3, a3, t2
    sltu  t2, a3, t2
    add   s0, s0, ra
    add   s0, s0, t2
    ld    t2, 32(a2)
    mulhu ra, t1, t2
    mul   t2, t1, t2
    add   a4, a4, s0
    sltu  s0, a4, s0
    add   a4, a4, t2
    sltu  t2, a4, t2
    add   ra, ra, s0
    add   ra, ra, t2
    ld    t2, 40(a2)
    mulhu s0, t1, t2
    mul   t2, t1, t2
    add   a5, a5, ra
    sltu  ra, a5, ra
    add   a5, a5, t2
    sltu  t2, a5, t2
    add   s0, s0, ra
    add   s0, s0, t2
    add   s0, s0, t0
    // --- limb 5 of b: t += a * b[5], then fold in m*p and shift ---
    ld    t2, 0(sp)
    ld    t1, 40(t2)
    ld    t2, 0(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   s1, s1, t2
    sltu  t2, s1, t2
    add   ra, ra, t2
    ld    t2, 8(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   a1, a1, ra
    sltu  ra, a1, ra
    add   a1, a1, t2
    sltu  t2, a1, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 16(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   a3, a3, t0
    sltu  t0, a3, t0
    add   a3, a3, t2
    sltu  t2, a3, t2
    add   ra, ra, t0
    add   ra, ra, t2
    ld    t2, 24(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   a4, a4, ra
    sltu  ra, a4, ra
    add   a4, a4, t2
    sltu  t2, a4, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 32(a0)
    mulhu ra, t2, t1
    mul   t2, t2, t1
    add   a5, a5, t0
    sltu  t0, a5, t0
    add   a5, a5, t2
    sltu  t2, a5, t2
    add   ra, ra, t0
    add   ra, ra, t2
    ld    t2, 40(a0)
    mulhu t0, t2, t1
    mul   t2, t2, t1
    add   s0, s0, ra
    sltu  ra, s0, ra
    add   s0, s0, t2
    sltu  t2, s0, t2
    add   t0, t0, ra
    add   t0, t0, t2
    ld    t2, 48(a2)
    mul   t1, s1, t2
    ld    t2, 0(a2)
    mulhu ra, t1, t2
    snez  t2, s1
    add   ra, ra, t2
    ld    t2, 8(a2)
    mulhu s1, t1, t2
    mul   t2, t1, t2
    add   a1, a1, ra
    sltu  ra, a1, ra
    add   a1, a1, t2
    sltu  t2, a1, t2
    add   s1, s1, ra
    add   s1, s1, t2
    ld    t2, 16(a2)
    mulhu ra, t1, t2
    mul   t2, t1, t2
    add   a3, a3, s1
    sltu  s1, a3, s1
    add   a3, a3, t2
    sltu  t2, a3, t2
    add   ra, ra, s1
    add   ra, ra, t2
    ld    t2, 24(a2)
    mulhu s1, t1, t2
    mul   t2, t1, t2
    add   a4, a4, ra
    sltu  ra, a4, ra
    add   a4, a4, t2
    sltu  t2, a4, t2
    add   s1, s1, ra
    add   s1, s1, t2
    ld    t2, 32(a2)
    mulhu ra, t1, t2
    mul   t2, t1, t2
    add   a5, a5, s1
    sltu  s1, a5, s1
    add   a5, a5, t2
    sltu  t2, a5, t2
    add   ra, ra, s1
    add   ra, ra, t2
    ld    t2, 40(a2)
    mulhu s1, t1, t2
    mul   t2, t1, t2
    add   s0, s0, ra
    sltu  ra, s0, ra
    add   s0, s0, t2
    sltu  t2, s0, t2
    add   s1, s1, ra
    add   s1, s1, t2
    add   s1, s1, t0
    // --- t < 2p: one conditional subtraction, comparing from the top ---
    ld    t2, 40(a2)
    bltu  s1, t2, .Lstore
    bne   s1, t2, .Lsub
    ld    t2, 32(a2)
    bltu  s0, t2, .Lstore
    bne   s0, t2, .Lsub
    ld    t2, 24(a2)
    bltu  a5, t2, .Lstore
    bne   a5, t2, .Lsub
    ld    t2, 16(a2)
    bltu  a4, t2, .Lstore
    bne   a4, t2, .Lsub
    ld    t2, 8(a2)
    bltu  a3, t2, .Lstore
    bne   a3, t2, .Lsub
    ld    t2, 0(a2)
    bltu  a1, t2, .Lstore
.Lsub:
    ld    t2, 0(a2)
    sltu  t0, a1, t2
    sub   a1, a1, t2
    ld    t2, 8(a2)
    sltu  ra, a3, t2
    sub   a3, a3, t2
    sltu  t2, a3, t0
    sub   a3, a3, t0
    add   t0, ra, t2
    ld    t2, 16(a2)
    sltu  ra, a4, t2
    sub   a4, a4, t2
    sltu  t2, a4, t0
    sub   a4, a4, t0
    add   t0, ra, t2
    ld    t2, 24(a2)
    sltu  ra, a5, t2
    sub   a5, a5, t2
    sltu  t2, a5, t0
    sub   a5, a5, t0
    add   t0, ra, t2
    ld    t2, 32(a2)
    sltu  ra, s0, t2
    sub   s0, s0, t2
    sltu  t2, s0, t0
    sub   s0, s0, t0
    add   t0, ra, t2
    ld    t2, 40(a2)
    sub   s1, s1, t2
    sub   s1, s1, t0
.Lstore:
    sd    a1, 0(a0)
    sd    a3, 8(a0)
    sd    a4, 16(a0)
    sd    a5, 24(a0)
    sd    s0, 32(a0)
    sd    s1, 40(a0)
    ld    ra, 8(sp)
    ld    s0, 16(sp)
    ld    s1, 24(sp)
    addi  sp, sp, 32
    ret
