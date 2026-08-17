// Shared implementation for the single-scalar-multiplication benchmarks
// (`bench-bls381-mul-g1`, `bench-bls381-mul-g2`, `bench-bander-mul`,
// `bench-pallas-mul`, `bench-vesta-mul`).
//
// Each crate defines `Base` (the affine point type) and `mul_once` — which calls
// the exact arkworks entry point the matching `sp_crypto_ec_utils` host function
// calls — and includes this file. One `run()` is one scalar multiplication, the
// unsized counterpart to `ec-bench-common.rs`.
//
// Note the scalar arrives as `u64` limbs, not as a field element: that is the
// shape `mul_affine` takes, and what `utils::mul_sw` / `utils::mul_te` pass it.

// Relative to this file, not to the crate that includes it.
#[path = "ec-fixtures.rs"]
mod fixtures;

use ark_ec::AffineRepr;
use ark_ff::PrimeField;

type Scalar = <Base as AffineRepr>::ScalarField;

struct State;
define_benchmark! {
    heap_size = 64 * 1024,
    state = State,
}

fn benchmark_initialize(_state: &mut State) {
    // Two checks a double-and-add ladder cannot pass by accident: a known small
    // multiple, and the group order annihilating the generator.
    let generator = Base::generator();
    assert!(mul_once(&generator, &[2]).into_group() == generator.into_group() + generator);
    assert!(mul_once(&generator, Scalar::MODULUS.as_ref()) == Base::zero());
}

fn benchmark_run(_state: &mut State) {
    use core::hint::black_box;

    let scalar = fixtures::seed_limbs::<Scalar>();
    let _ = black_box(mul_once(black_box(&Base::generator()), black_box(scalar.as_ref())));
}
