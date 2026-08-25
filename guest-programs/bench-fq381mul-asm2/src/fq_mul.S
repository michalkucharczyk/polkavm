// bls12-381 base-field (Fq) Montgomery multiply for PVM (rv64e), v2.
// Generated - see .agent/worklog/fqmul-asm2-results.md for the derivation.
//
// fn fq381_mul_asm2(a: *mut u64 [a0], b: *const u64 [a1], pool: *const u64 [a2])
//
// a and b are six little-endian 64-bit limbs in Montgomery form; the product
// (fully reduced, < p) is written back over a. `pool` is [p0..p5, INV] where
// INV = -p^-1 mod 2^64. Bit-identical to ark-ff 0.5's
// MontBackend::<FqConfig, 6>::mul_assign for every input, in range or not.
//
// Same algorithm as v1 (`bench-fq381mul-asm`) but scheduled for the x86 the
// recompiler emits rather than for the PVM instruction count. v1 executes 12
// six-limb carry chains back to back; this keeps the a*b and m*p carries in
// flight together, so about four cycles of chain latency buy two
// multiply-accumulates instead of one:
//
//   * each limb's own low product is folded into the accumulator before the
//     incoming carry, so that add and its sltu are off the carry chain;
//   * every carry-chain sltu writes a register that is neither of its sources,
//     which the recompiler emits as `xor; cmp; setb` instead of
//     `cmp; setb; and` - same three x86 instructions, one less cycle;
//   * the accumulator lives in the stack frame, which makes the Montgomery
//     shift an address offset instead of a register rotation and frees the
//     registers the second carry chain needs. Those reloads sit about fifteen
//     cycles off the critical path.
//
// Register budget, exactly 12 (x16-x31 do not exist on rv64e):
//   a0 a1 a2   &a[0] (and the destination), &b[0], &pool[0]
//   t1 t2      b_i and m, live for the whole outer iteration
//   t0         the accumulator word of the current inner iteration
//   a3         operand limb -> low product -> carry bit
//   a4 a5      the a*b carry, alternating so no step needs a mv
//   s0 s1      the m*p carry, likewise
//
// Not constant-time: the final conditional subtraction branches, as does
// ark-ff's. This is benchmark code.
.section .text.fq381_mul_asm2,"ax"
.globl fq381_mul_asm2
fq381_mul_asm2:
    addi  sp, sp, -64
    sd    s0, 48(sp)
    sd    s1, 56(sp)
    // --- b[0]: a*b and m*p folded into one pass over the limbs ---
    ld    t1, 0(a1)
    ld    a3, 0(a0)
    mulhu a5, a3, t1
    mul   t0, a3, t1
    ld    a3, 48(a2)
    mul   t2, t0, a3
    ld    a3, 0(a2)
    mulhu s1, t2, a3
    snez  a3, t0
    add   s1, s1, a3
    ld    a3, 8(a0)
    mulhu a4, a3, t1
    mul   t0, a3, t1
    ld    a3, 8(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 0(sp)
    ld    a3, 16(a0)
    mulhu a5, a3, t1
    mul   t0, a3, t1
    ld    a3, 16(a2)
    mulhu s1, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    add   t0, t0, a4
    sltu  a3, t0, a4
    add   s1, s1, a3
    add   t0, t0, s0
    sltu  a3, t0, s0
    add   s1, s1, a3
    sd    t0, 8(sp)
    ld    a3, 24(a0)
    mulhu a4, a3, t1
    mul   t0, a3, t1
    ld    a3, 24(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 16(sp)
    ld    a3, 32(a0)
    mulhu a5, a3, t1
    mul   t0, a3, t1
    ld    a3, 32(a2)
    mulhu s1, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    add   t0, t0, a4
    sltu  a3, t0, a4
    add   s1, s1, a3
    add   t0, t0, s0
    sltu  a3, t0, s0
    add   s1, s1, a3
    sd    t0, 24(sp)
    ld    a3, 40(a0)
    mulhu a4, a3, t1
    mul   t0, a3, t1
    ld    a3, 40(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 32(sp)
    add   t0, a4, s0
    sd    t0, 40(sp)
    // --- b[1]: a*b and m*p folded into one pass over the limbs ---
    ld    t1, 8(a1)
    ld    a3, 0(a0)
    mulhu a5, a3, t1
    ld    t0, 0(sp)
    mul   a3, a3, t1
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 48(a2)
    mul   t2, t0, a3
    ld    a3, 0(a2)
    mulhu s1, t2, a3
    snez  a3, t0
    add   s1, s1, a3
    ld    a3, 8(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 8(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 8(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 0(sp)
    ld    a3, 16(a0)
    mulhu a5, a3, t1
    mul   a3, a3, t1
    ld    t0, 16(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 16(a2)
    mulhu s1, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    add   t0, t0, a4
    sltu  a3, t0, a4
    add   s1, s1, a3
    add   t0, t0, s0
    sltu  a3, t0, s0
    add   s1, s1, a3
    sd    t0, 8(sp)
    ld    a3, 24(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 24(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 24(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 16(sp)
    ld    a3, 32(a0)
    mulhu a5, a3, t1
    mul   a3, a3, t1
    ld    t0, 32(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 32(a2)
    mulhu s1, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    add   t0, t0, a4
    sltu  a3, t0, a4
    add   s1, s1, a3
    add   t0, t0, s0
    sltu  a3, t0, s0
    add   s1, s1, a3
    sd    t0, 24(sp)
    ld    a3, 40(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 40(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 40(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 32(sp)
    add   t0, a4, s0
    sd    t0, 40(sp)
    // --- b[2]: a*b and m*p folded into one pass over the limbs ---
    ld    t1, 16(a1)
    ld    a3, 0(a0)
    mulhu a5, a3, t1
    ld    t0, 0(sp)
    mul   a3, a3, t1
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 48(a2)
    mul   t2, t0, a3
    ld    a3, 0(a2)
    mulhu s1, t2, a3
    snez  a3, t0
    add   s1, s1, a3
    ld    a3, 8(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 8(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 8(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 0(sp)
    ld    a3, 16(a0)
    mulhu a5, a3, t1
    mul   a3, a3, t1
    ld    t0, 16(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 16(a2)
    mulhu s1, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    add   t0, t0, a4
    sltu  a3, t0, a4
    add   s1, s1, a3
    add   t0, t0, s0
    sltu  a3, t0, s0
    add   s1, s1, a3
    sd    t0, 8(sp)
    ld    a3, 24(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 24(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 24(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 16(sp)
    ld    a3, 32(a0)
    mulhu a5, a3, t1
    mul   a3, a3, t1
    ld    t0, 32(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 32(a2)
    mulhu s1, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    add   t0, t0, a4
    sltu  a3, t0, a4
    add   s1, s1, a3
    add   t0, t0, s0
    sltu  a3, t0, s0
    add   s1, s1, a3
    sd    t0, 24(sp)
    ld    a3, 40(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 40(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 40(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 32(sp)
    add   t0, a4, s0
    sd    t0, 40(sp)
    // --- b[3]: a*b and m*p folded into one pass over the limbs ---
    ld    t1, 24(a1)
    ld    a3, 0(a0)
    mulhu a5, a3, t1
    ld    t0, 0(sp)
    mul   a3, a3, t1
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 48(a2)
    mul   t2, t0, a3
    ld    a3, 0(a2)
    mulhu s1, t2, a3
    snez  a3, t0
    add   s1, s1, a3
    ld    a3, 8(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 8(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 8(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 0(sp)
    ld    a3, 16(a0)
    mulhu a5, a3, t1
    mul   a3, a3, t1
    ld    t0, 16(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 16(a2)
    mulhu s1, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    add   t0, t0, a4
    sltu  a3, t0, a4
    add   s1, s1, a3
    add   t0, t0, s0
    sltu  a3, t0, s0
    add   s1, s1, a3
    sd    t0, 8(sp)
    ld    a3, 24(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 24(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 24(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 16(sp)
    ld    a3, 32(a0)
    mulhu a5, a3, t1
    mul   a3, a3, t1
    ld    t0, 32(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 32(a2)
    mulhu s1, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    add   t0, t0, a4
    sltu  a3, t0, a4
    add   s1, s1, a3
    add   t0, t0, s0
    sltu  a3, t0, s0
    add   s1, s1, a3
    sd    t0, 24(sp)
    ld    a3, 40(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 40(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 40(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 32(sp)
    add   t0, a4, s0
    sd    t0, 40(sp)
    // --- b[4]: a*b and m*p folded into one pass over the limbs ---
    ld    t1, 32(a1)
    ld    a3, 0(a0)
    mulhu a5, a3, t1
    ld    t0, 0(sp)
    mul   a3, a3, t1
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 48(a2)
    mul   t2, t0, a3
    ld    a3, 0(a2)
    mulhu s1, t2, a3
    snez  a3, t0
    add   s1, s1, a3
    ld    a3, 8(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 8(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 8(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 0(sp)
    ld    a3, 16(a0)
    mulhu a5, a3, t1
    mul   a3, a3, t1
    ld    t0, 16(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 16(a2)
    mulhu s1, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    add   t0, t0, a4
    sltu  a3, t0, a4
    add   s1, s1, a3
    add   t0, t0, s0
    sltu  a3, t0, s0
    add   s1, s1, a3
    sd    t0, 8(sp)
    ld    a3, 24(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 24(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 24(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 16(sp)
    ld    a3, 32(a0)
    mulhu a5, a3, t1
    mul   a3, a3, t1
    ld    t0, 32(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 32(a2)
    mulhu s1, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    add   t0, t0, a4
    sltu  a3, t0, a4
    add   s1, s1, a3
    add   t0, t0, s0
    sltu  a3, t0, s0
    add   s1, s1, a3
    sd    t0, 24(sp)
    ld    a3, 40(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 40(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 40(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 32(sp)
    add   t0, a4, s0
    sd    t0, 40(sp)
    // --- b[5]: a*b and m*p folded into one pass over the limbs ---
    ld    t1, 40(a1)
    ld    a3, 0(a0)
    mulhu a5, a3, t1
    ld    t0, 0(sp)
    mul   a3, a3, t1
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 48(a2)
    mul   t2, t0, a3
    ld    a3, 0(a2)
    mulhu s1, t2, a3
    snez  a3, t0
    add   s1, s1, a3
    ld    a3, 8(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 8(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 8(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 0(sp)
    ld    a3, 16(a0)
    mulhu a5, a3, t1
    mul   a3, a3, t1
    ld    t0, 16(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 16(a2)
    mulhu s1, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    add   t0, t0, a4
    sltu  a3, t0, a4
    add   s1, s1, a3
    add   t0, t0, s0
    sltu  a3, t0, s0
    add   s1, s1, a3
    sd    t0, 8(sp)
    ld    a3, 24(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 24(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 24(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 16(sp)
    ld    a3, 32(a0)
    mulhu a5, a3, t1
    mul   a3, a3, t1
    ld    t0, 32(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    ld    a3, 32(a2)
    mulhu s1, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a5, a5, a3
    add   t0, t0, a4
    sltu  a3, t0, a4
    add   s1, s1, a3
    add   t0, t0, s0
    sltu  a3, t0, s0
    add   s1, s1, a3
    sd    t0, 24(sp)
    ld    a3, 40(a0)
    mulhu a4, a3, t1
    mul   a3, a3, t1
    ld    t0, 40(sp)
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    ld    a3, 40(a2)
    mulhu s0, t2, a3
    mul   a3, t2, a3
    add   t0, t0, a3
    sltu  a3, t0, a3
    add   a4, a4, a3
    add   t0, t0, a5
    sltu  a3, t0, a5
    add   s0, s0, a3
    add   t0, t0, s1
    sltu  a3, t0, s1
    add   s0, s0, a3
    sd    t0, 32(sp)
    add   t0, a4, s0
    sd    t0, 40(sp)
    // --- t < 2p: one conditional subtraction, comparing from the top ---
    ld    a1, 0(sp)
    ld    a4, 8(sp)
    ld    a5, 16(sp)
    ld    s0, 24(sp)
    ld    s1, 32(sp)
    ld    t1, 40(sp)
    ld    a3, 40(a2)
    bltu  t1, a3, .Lstore
    bne   t1, a3, .Lsub
    ld    a3, 32(a2)
    bltu  s1, a3, .Lstore
    bne   s1, a3, .Lsub
    ld    a3, 24(a2)
    bltu  s0, a3, .Lstore
    bne   s0, a3, .Lsub
    ld    a3, 16(a2)
    bltu  a5, a3, .Lstore
    bne   a5, a3, .Lsub
    ld    a3, 8(a2)
    bltu  a4, a3, .Lstore
    bne   a4, a3, .Lsub
    ld    a3, 0(a2)
    bltu  a1, a3, .Lstore
.Lsub:
    ld    a3, 0(a2)
    sltu  t2, a1, a3
    sub   a1, a1, a3
    ld    a3, 8(a2)
    sltu  t0, a4, a3
    sub   a4, a4, a3
    sltu  a3, a4, t2
    sub   a4, a4, t2
    add   t2, t0, a3
    ld    a3, 16(a2)
    sltu  t0, a5, a3
    sub   a5, a5, a3
    sltu  a3, a5, t2
    sub   a5, a5, t2
    add   t2, t0, a3
    ld    a3, 24(a2)
    sltu  t0, s0, a3
    sub   s0, s0, a3
    sltu  a3, s0, t2
    sub   s0, s0, t2
    add   t2, t0, a3
    ld    a3, 32(a2)
    sltu  t0, s1, a3
    sub   s1, s1, a3
    sltu  a3, s1, t2
    sub   s1, s1, t2
    add   t2, t0, a3
    ld    a3, 40(a2)
    sub   t1, t1, a3
    sub   t1, t1, t2
.Lstore:
    sd    a1, 0(a0)
    sd    a4, 8(a0)
    sd    a5, 16(a0)
    sd    s0, 24(a0)
    sd    s1, 32(a0)
    sd    t1, 40(a0)
    ld    s0, 48(sp)
    ld    s1, 56(sp)
    addi  sp, sp, 64
    ret
