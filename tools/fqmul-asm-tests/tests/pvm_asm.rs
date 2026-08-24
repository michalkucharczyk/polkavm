//! Executes the bls12-381 base-field Montgomery multiply of both benchmark
//! blobs inside a real PVM instance (interpreter backend) and requires the raw
//! output limbs to be byte-identical to ark-ff's on the host.
//!
//! `bench-fq381mul-asm` is the hand-written kernel under test;
//! `bench-fq381mul` is LLVM's codegen for the same thing and is run through the
//! same vectors, so a failure tells us whether the assembly or the harness is
//! at fault.
//!
//! The blobs are built by this test with the toolchain/target the benchmarks
//! use, mirroring how crates/polkavm's own tests build their test blobs.
//!
//! Vectors are raw limb patterns, not field elements: the kernel is defined on
//! `[u64; 6]`, and the contract is bit-identity with ark-ff, not just
//! congruence mod p.

use ark_bls12_381::{Fq, FqConfig};
use ark_ff::{BigInt, MontConfig, PrimeField};
use std::path::PathBuf;
use std::process::Command;

const P: [u64; 6] = FqConfig::MODULUS.0;

fn build_guest_blob(package: &str) -> Vec<u8> {
    let guest_programs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../guest-programs");

    let mut args = polkavm_linker::TargetJsonArgs::default();
    args.is_64_bit = true;
    args.rustc_version = polkavm_linker::RustcVersion::Legacy;
    let target_json = polkavm_linker::target_json_path(args).unwrap();

    let mut cmd = Command::new("cargo");
    // Scrub the outer invocation's toolchain pins so the guest builds with the
    // toolchain pinned by guest-programs/rust-toolchain.toml (a nightly with
    // rust-src, required by -Zbuild-std).
    for (key, _) in std::env::vars() {
        if key.contains("CARGO") || key.contains("RUSTC") || key == "RUSTUP_TOOLCHAIN" {
            cmd.env_remove(&key);
        }
    }
    let output = cmd
        .args(["build", "-q", "--release", "-p", package, "--bin", package])
        .arg("--target")
        .arg(&target_json)
        .arg("-Zbuild-std=core,alloc")
        .current_dir(&guest_programs)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "guest build of {package} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let elf = std::fs::read(
        guest_programs
            .join("target/riscv64emac-unknown-none-polkavm/release")
            .join(package),
    )
    .unwrap();
    polkavm_linker::program_from_elf(
        polkavm_linker::Config::default(),
        polkavm_linker::TargetInstructionSet::Latest,
        &elf,
    )
    .unwrap()
}

/// Limb patterns worth naming. Everything up to `EDGE_IN_RANGE` is `< p`, which
/// is ark-ff's contract; the rest are in `[p, 2p)`, the range a Montgomery
/// reduction produces before its conditional subtraction, and are here because
/// bit-identity should not depend on the caller having reduced. `P[0]` is odd,
/// so p-1 and p-2 need no borrow out of limb 0.
const EDGE: [[u64; 6]; 12] = [
    [0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0],
    [P[0] - 1, P[1], P[2], P[3], P[4], P[5]],
    [P[0] - 2, P[1], P[2], P[3], P[4], P[5]],
    [u64::MAX, u64::MAX, u64::MAX, u64::MAX, u64::MAX, P[5] - 1],
    [0, 0, 0, 0, 0, P[5] - 1],
    [u64::MAX, 0, u64::MAX, 0, u64::MAX, 0],
    [0, u64::MAX, 0, u64::MAX, 0, 1],
    [u64::MAX, u64::MAX, u64::MAX, u64::MAX, u64::MAX, 0],
    // Out of ark-ff's contract, still required to agree bit for bit:
    [P[0], P[1], P[2], P[3], P[4], P[5]],
    [u64::MAX, u64::MAX, u64::MAX, u64::MAX, u64::MAX, P[5]],
    [0, 0, 0, 0, 0, 2 * P[5]],
];

/// Deterministic 384-bit values reduced mod p, from a 64-bit LCG (Knuth's
/// MMIX constants). Reduced, so these are inside ark-ff's contract; the raw
/// limbs still cover the whole `[0, p)` range including both ends of the
/// conditional-subtraction branch.
fn lcg_vectors(count: usize) -> Vec<[u64; 6]> {
    let mut state = 0x0123_4567_89ab_cdefu64;
    let mut next = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        state
    };
    (0..count)
        .map(|_| {
            let mut bytes = [0u8; 48];
            for chunk in bytes.chunks_exact_mut(8) {
                chunk.copy_from_slice(&next().to_le_bytes());
            }
            Fq::from_le_bytes_mod_order(&bytes).0 .0
        })
        .collect()
}

fn expected(a: [u64; 6], b: [u64; 6]) -> [u64; 6] {
    let mut a = Fq::new_unchecked(BigInt(a));
    a *= Fq::new_unchecked(BigInt(b));
    a.0 .0
}

fn limbs_to_bytes(limbs: &[u64]) -> Vec<u8> {
    limbs.iter().flat_map(|limb| limb.to_le_bytes()).collect()
}

fn check_blob(package: &str) {
    let blob = build_guest_blob(package);

    let mut config = polkavm::Config::from_env().unwrap();
    // Deterministic and available everywhere (no sandbox requirements).
    config.set_backend(Some(polkavm::BackendKind::Interpreter));
    let engine = polkavm::Engine::new(&config).unwrap();
    let blob = polkavm::ProgramBlob::parse(blob.into()).unwrap();
    let module = polkavm::Module::from_blob(&engine, &polkavm::ModuleConfig::default(), blob).unwrap();
    let linker = polkavm::Linker::<()>::new();
    let mut instance = linker.instantiate_pre(&module).unwrap().instantiate().unwrap();

    // The blob's own self-check; it traps rather than returning a status.
    instance.call_typed(&mut (), "initialize", ()).unwrap();

    let pair_ptr: u64 = instance.call_typed_and_get_result(&mut (), "pair_ptr", ()).unwrap();

    let mut vectors: Vec<[u64; 6]> = EDGE.to_vec();
    vectors.extend(lcg_vectors(1024));

    let mut checked = 0usize;
    // Every edge x edge pair, then the LCG stream against itself pairwise and
    // against each edge value.
    let pairs = EDGE
        .iter()
        .flat_map(|a| EDGE.iter().map(move |b| (*a, *b)))
        .chain(vectors.windows(2).map(|w| (w[0], w[1])))
        .chain(
            EDGE.iter()
                .flat_map(|a| vectors.iter().map(move |b| (*a, *b))),
        );

    for (a, b) in pairs {
        instance
            .write_memory(pair_ptr as u32, &limbs_to_bytes(&a))
            .unwrap();
        instance
            .write_memory(pair_ptr as u32 + 48, &limbs_to_bytes(&b))
            .unwrap();
        instance.call_typed(&mut (), "mul_pair", ()).unwrap();
        let got = instance.read_memory(pair_ptr as u32, 48).unwrap();

        assert_eq!(
            got,
            limbs_to_bytes(&expected(a, b)),
            "{package}: mismatch for a={a:016x?} b={b:016x?}"
        );
        checked += 1;
    }
    assert!(checked >= 1000, "{package}: only {checked} vectors checked");
}

#[test]
fn asm_fq_mul_in_pvm_matches_ark_ff() {
    check_blob("bench-fq381mul-asm");
}

#[test]
fn llvm_fq_mul_in_pvm_matches_ark_ff() {
    check_blob("bench-fq381mul");
}
