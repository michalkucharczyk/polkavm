// Shared implementation for the sized multi-scalar-multiplication benchmarks
// (`bench-bls381-msm-g1`, `bench-bls381-msm-g2`, `bench-bander-msm`).
//
// Each crate defines `Base` (the affine point type) plus `msm_once` and
// `mul_once` — which call the exact arkworks entry points the matching
// `sp_crypto_ec_utils` host function calls — and includes this file. One `run()`
// performs one MSM over `count` bases; the harness overrides `count` through
// the `benchmark_set_size` export (`--size`).
//
// Unlike the hash benchmarks there is no amortization loop: a single MSM is
// already orders of magnitude above the harness's per-call overhead.

// Relative to this file, not to the crate that includes it.
#[path = "ec-fixtures.rs"]
mod fixtures;

use ark_ec::AffineRepr;
use ark_ff::PrimeField;

type Scalar = <Base as AffineRepr>::ScalarField;

const MAX_COUNT: usize = 1024;
const DEFAULT_COUNT: usize = 256;

struct State {
    bases: alloc::vec::Vec<Base>,
    scalars: alloc::vec::Vec<Scalar>,
    count: usize,
}

// Generous on purpose. Measured on the worst case (G2 at MAX_COUNT): 512 KiB
// traps, and at 768 KiB the gas per run *changes* (+49k) because picoalloc
// starts working harder - so anything under 1 MiB silently taxes the numbers
// rather than failing. The floor is close enough that a larger size grid or a
// bigger field (bw6-761) would land in it, and an untouched static heap costs
// nothing at runtime.
define_benchmark! {
    heap_size = 16 * 1024 * 1024,
    state = State {
        bases: alloc::vec::Vec::new(),
        scalars: alloc::vec::Vec::new(),
        count: DEFAULT_COUNT,
    },
}

fn benchmark_initialize(state: &mut State) {
    state.bases = fixtures::bases(MAX_COUNT);
    state.scalars = fixtures::scalars(MAX_COUNT);

    // An MSM over two points must equal the two scalar multiplications it
    // stands for. This catches a mis-wired kernel or broken codegen before any
    // timing gets published.
    let first = state.scalars[0].into_bigint();
    let second = state.scalars[1].into_bigint();
    let expected = mul_once(&state.bases[0], first.as_ref()).into_group()
        + mul_once(&state.bases[1], second.as_ref()).into_group();

    assert!(msm_once(&state.bases[..2], &state.scalars[..2]).into_group() == expected);
}

fn benchmark_run(state: &mut State) {
    use core::hint::black_box;

    let count = state.count;
    let _ = black_box(msm_once(black_box(&state.bases[..count]), black_box(&state.scalars[..count])));
}

#[cfg_attr(target_env = "polkavm", polkavm_derive::polkavm_export)]
#[no_mangle]
pub extern "C" fn benchmark_set_size(size: u64) {
    assert!(size > 0 && size as usize <= MAX_COUNT);
    let state = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
    state.count = size as usize;
}
