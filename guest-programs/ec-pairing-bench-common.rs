// Shared implementation for the pairing benchmarks (`bench-bls381-pairing`,
// `bench-bls377-pairing`, `bench-bw6761-pairing`).
//
// Each crate defines `Curve` (the `Pairing` implementation) and includes this
// file. One `run()` is `multi_miller_loop` over one pair followed by
// `final_exponentiation` — a full pairing, which is the unit a BLS signature or
// ring-VRF verification pays, and which is what the two `sp_crypto_ec_utils`
// host functions do back to back. Splitting them would measure two halves nobody
// calls in isolation.

// Relative to this file, not to the crate that includes it.
#[path = "ec-fixtures.rs"]
mod fixtures;

use ark_ec::{
    pairing::{Pairing, PairingOutput},
    AffineRepr, CurveGroup,
};

type Scalar = <Curve as Pairing>::ScalarField;
type G1 = <Curve as Pairing>::G1Affine;
type G2 = <Curve as Pairing>::G2Affine;

struct State;
define_benchmark! {
    heap_size = 1024 * 1024,
    state = State,
}

fn pairing_once(g1: G1, g2: G2) -> PairingOutput<Curve> {
    Curve::final_exponentiation(Curve::multi_miller_loop([g1], [g2])).unwrap()
}

fn benchmark_initialize(_state: &mut State) {
    // Bilinearity: e(aP, Q) == e(P, aQ). A pairing that is fast but wrong would
    // otherwise sail through the benchmark unnoticed.
    let scalar = fixtures::seed_limbs::<Scalar>();
    let g1 = G1::generator();
    let g2 = G2::generator();

    assert!(
        pairing_once(g1.mul_bigint(scalar).into_affine(), g2) == pairing_once(g1, g2.mul_bigint(scalar).into_affine())
    );
}

fn benchmark_run(_state: &mut State) {
    use core::hint::black_box;

    let _ = black_box(pairing_once(black_box(G1::generator()), black_box(G2::generator())));
}
