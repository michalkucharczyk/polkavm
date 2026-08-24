// Shared body for `bench-fq381mul` (LLVM codegen) and `bench-fq381mul-asm`
// (hand-scheduled RISC-V kernel). Each crate defines `fq_mul(&mut Fq, &Fq)` -
// the kernel under test - and includes this file.
//
// One run() is a chain of `count` bls12-381 base-field multiplications. The
// chain is data-dependent, so nothing can be hoisted; `count` is settable via
// `benchmark_set_size`, which makes the per-multiply cost the *slope* of gas
// against size and so independent of the harness's fixed overhead.

use ark_ff::{BigInt, One, Zero};

const DEFAULT_COUNT: usize = 1000;

struct State {
    count: usize,
}

define_benchmark! {
    heap_size = 64 * 1024,
    state = State { count: DEFAULT_COUNT },
}

const P: [u64; 6] = <ark_bls12_381::FqConfig as ark_ff::MontConfig<6>>::MODULUS.0;

/// Raw Montgomery limb patterns the kernel has to survive, all `< p`: zero,
/// one, p-1, p-2, the largest value sharing p's top limb minus one, and
/// alternating all-ones/all-zeros limbs. `P[0]` is odd, so p-1 and p-2 need no
/// borrow out of limb 0.
const EDGE: [[u64; 6]; 8] = [
    [0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0],
    [P[0] - 1, P[1], P[2], P[3], P[4], P[5]],
    [P[0] - 2, P[1], P[2], P[3], P[4], P[5]],
    [u64::MAX, u64::MAX, u64::MAX, u64::MAX, u64::MAX, P[5] - 1],
    [0, 0, 0, 0, 0, P[5] - 1],
    [u64::MAX, 0, u64::MAX, 0, u64::MAX, 0],
    [0, u64::MAX, 0, u64::MAX, 0, 1],
];

fn edge(index: usize) -> Fq {
    Fq::new_unchecked(BigInt(EDGE[index]))
}

fn benchmark_initialize(_state: &mut State) {
    let one = Fq::one();
    let zero = Fq::zero();

    // Products no incorrect kernel can produce by accident.
    let mut small = Fq::from(2u64);
    fq_mul(&mut small, &Fq::from(3u64));
    assert!(small == Fq::from(6u64));

    let mut minus_one = -one;
    fq_mul(&mut minus_one, &(-one));
    assert!(minus_one == one);

    for i in 0..EDGE.len() {
        let mut times_one = edge(i);
        fq_mul(&mut times_one, &one);
        assert!(times_one == edge(i));

        let mut times_zero = edge(i);
        fq_mul(&mut times_zero, &zero);
        assert!(times_zero == zero);

        for j in 0..EDGE.len() {
            let mut forward = edge(i);
            fq_mul(&mut forward, &edge(j));

            let mut reverse = edge(j);
            fq_mul(&mut reverse, &edge(i));
            assert!(forward == reverse);

            // Differential against ark-ff's own multiply. Vacuous in
            // `bench-fq381mul`, whose kernel *is* ark-ff's; the point of it is
            // `bench-fq381mul-asm`, where the two are different code.
            let mut reference = edge(i);
            reference *= edge(j);
            assert!(forward == reference);
        }
    }
}

fn benchmark_run(state: &mut State) {
    use core::hint::black_box;

    let a = unsafe { &mut *core::ptr::addr_of_mut!(CHAIN_A) };
    let b = unsafe { &*core::ptr::addr_of!(CHAIN_B) };
    for _ in 0..state.count {
        fq_mul(black_box(a), black_box(b));
    }
}

static mut CHAIN_A: Fq = ark_ff::MontFp!("2");
static mut CHAIN_B: Fq = ark_ff::MontFp!("3");

#[cfg_attr(target_env = "polkavm", polkavm_derive::polkavm_export)]
#[no_mangle]
pub extern "C" fn benchmark_set_size(size: u64) {
    assert!(size > 0);
    let state = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
    state.count = size as usize;
}

/// Scratch for `tools/fqmul-asm-tests`: the host writes the raw Montgomery
/// limbs of a (offsets 0..48) and b (48..96) here, calls `mul_pair`, and reads
/// the product back out of a. Exercises the kernel on limb patterns the
/// chained benchmark never reaches.
static mut PAIR: [u64; 12] = [0; 12];

#[cfg_attr(target_env = "polkavm", polkavm_derive::polkavm_export)]
#[no_mangle]
pub extern "C" fn pair_ptr() -> u64 {
    core::ptr::addr_of!(PAIR) as u64
}

#[cfg_attr(target_env = "polkavm", polkavm_derive::polkavm_export)]
#[no_mangle]
pub extern "C" fn mul_pair() {
    let pair = unsafe { &mut *core::ptr::addr_of_mut!(PAIR) };
    let mut a = Fq::new_unchecked(BigInt(pair[..6].try_into().unwrap()));
    let b = Fq::new_unchecked(BigInt(pair[6..].try_into().unwrap()));
    fq_mul(&mut a, &b);
    pair[..6].copy_from_slice(&a.0 .0);
}
