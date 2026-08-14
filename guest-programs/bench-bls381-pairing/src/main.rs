#![no_std]
#![no_main]

include!("../../bench-common.rs");

// Fq12 towers plus the Miller loop's line-coefficient tables need far more than
// the default 8 KiB of stack.
#[cfg(target_env = "polkavm")]
polkavm_derive::min_stack_size!(256 * 1024);

// The `bls12_381_multi_miller_loop` and `bls12_381_final_exponentiation` host
// functions back to back over one pair: a full pairing, which is the unit a BLS
// signature or ring-VRF verification pays. Splitting them would measure two
// halves nobody calls in isolation.
#[path = "../../ec-fixtures.rs"]
mod fixtures;

use ark_bls12_381::{Bls12_381, Fr, G1Affine, G2Affine};
use ark_ec::{
    pairing::{Pairing, PairingOutput},
    AffineRepr, CurveGroup,
};

struct State;
define_benchmark! {
    heap_size = 1024 * 1024,
    state = State,
}

fn pairing_once(g1: G1Affine, g2: G2Affine) -> PairingOutput<Bls12_381> {
    Bls12_381::final_exponentiation(Bls12_381::multi_miller_loop([g1], [g2])).unwrap()
}

fn benchmark_initialize(_state: &mut State) {
    // Bilinearity: e(aP, Q) == e(P, aQ). A pairing that is fast but wrong would
    // otherwise sail through the benchmark unnoticed.
    let scalar = fixtures::seed_limbs::<Fr>();
    let g1 = G1Affine::generator();
    let g2 = G2Affine::generator();

    assert!(
        pairing_once(g1.mul_bigint(scalar).into_affine(), g2) == pairing_once(g1, g2.mul_bigint(scalar).into_affine())
    );
}

fn benchmark_run(_state: &mut State) {
    use core::hint::black_box;

    let _ = black_box(pairing_once(black_box(G1Affine::generator()), black_box(G2Affine::generator())));
}
