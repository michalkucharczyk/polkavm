//! Correctness tests for the 256-bit wide-arithmetic backend.
//!
//! Executes curve25519 arithmetic and signature verification inside a real PVM
//! instance — under **both** the interpreter and the recompiler — and compares
//! against a host-side reference (stock 5×51 dalek).
//!
//! What this covers that nothing else does:
//!
//! - the **composition**: `crates/polkavm`'s own `wide_arith_*` tests check the
//!   instructions against their reference semantics, but nothing checked that
//!   point/scalar arithmetic *built out of* them produces right answers on
//!   recompiled code.
//! - **rejection**: every signature benchmark verifies a valid vector and
//!   `.unwrap()`s it, so a field implementation broken in a collapsing way
//!   (everything zero, comparisons always true) would pass while accepting
//!   forgeries.
//! - **redc vs generic route**: the fused fold opcodes against the unfused
//!   instruction sequences, on target, with no host reference involved.
//!
//! Not covered: the **Linux sandbox** backend. Its `clone` is blocked in a
//! container, so only the interpreter and the generic-sandbox recompiler run
//! here — the same gap `crates/polkavm`'s tests have.

// The host reference is only a reference if it is not itself the pvm backend.
// Backend selection comes from RUSTFLAGS, so running this suite with the
// guest's flags in the environment would compare pvm64 against pvm64 and pass
// vacuously. Refuse to build in that case.
#[cfg(curve25519_dalek_backend = "pvm")]
compile_error!(
    "the host-side reference must not use the pvm backend: unset \
     RUSTFLAGS='--cfg curve25519_dalek_backend=\"pvm\"' before running these tests"
);

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// Which guest configuration to build. One source, three backends.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Route {
    /// Stock 5×51 dalek — no wide-arithmetic instructions at all.
    Off,
    /// 4×64 backend, reduction via `mul256_by_u64` + `add256` folds.
    NoRedc,
    /// 4×64 backend with `redc256` and the fused fold opcodes.
    Redc,
}

impl Route {
    fn rustflags(self) -> &'static str {
        match self {
            Route::Off => "",
            Route::NoRedc => r#"--cfg curve25519_dalek_backend="pvm""#,
            Route::Redc => r#"--cfg curve25519_dalek_backend="pvm" --cfg pvm_redc"#,
        }
    }
}

/// Builds the guest and links it into a blob. Mirrors
/// `tools/hash-asm-tests/tests/pvm_asm.rs`, including the environment scrubbing
/// — without it the guest would inherit this test's toolchain instead of the
/// nightly pinned by `guest-programs/rust-toolchain.toml`, which is what
/// `-Zbuild-std` needs.
fn build_guest(route: Route) -> Vec<u8> {
    let guest_programs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../guest-programs");

    let mut args = polkavm_linker::TargetJsonArgs::default();
    args.is_64_bit = true;
    args.rustc_version = polkavm_linker::RustcVersion::Legacy;
    let target_json = polkavm_linker::target_json_path(args).unwrap();

    // Each route needs its own target dir, otherwise cargo rebuilds from
    // scratch every time the cfg changes and the three configurations thrash
    // each other. Kept under `target/`, which is already gitignored.
    let target_dir = guest_programs.join(format!("target/test-25519-{route:?}").to_lowercase());

    let mut cmd = Command::new("cargo");
    for (key, _) in std::env::vars() {
        if key.contains("CARGO") || key.contains("RUSTC") || key == "RUSTUP_TOOLCHAIN" {
            cmd.env_remove(&key);
        }
    }
    let output = cmd
        .env("RUSTFLAGS", route.rustflags())
        .env("CARGO_TARGET_DIR", &target_dir)
        .args(["build", "-q", "--release", "-p", "test-25519-pvm", "--bin", "test-25519-pvm"])
        .arg("--target")
        .arg(&target_json)
        .arg("-Zbuild-std=core,alloc")
        .current_dir(&guest_programs)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "guest build failed for {route:?}:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let elf = std::fs::read(target_dir.join("riscv64emac-unknown-none-polkavm/release/test-25519-pvm")).unwrap();
    polkavm_linker::program_from_elf(
        polkavm_linker::Config::default(),
        polkavm_linker::TargetInstructionSet::Latest,
        &elf,
    )
    .unwrap()
}

fn blob(route: Route) -> &'static [u8] {
    static CACHE: OnceLock<[Vec<u8>; 3]> = OnceLock::new();
    let cache = CACHE.get_or_init(|| [build_guest(Route::Off), build_guest(Route::NoRedc), build_guest(Route::Redc)]);
    match route {
        Route::Off => &cache[0],
        Route::NoRedc => &cache[1],
        Route::Redc => &cache[2],
    }
}

/// A guest instance plus the addresses of its I/O buffers.
struct Guest {
    instance: polkavm::Instance,
    input_ptr: u32,
    output_ptr: u32,
}

impl Guest {
    fn new(route: Route, backend: polkavm::BackendKind) -> Self {
        let mut config = polkavm::Config::from_env().unwrap();
        config.set_backend(Some(backend));
        if backend == polkavm::BackendKind::Compiler {
            config.set_sandbox(Some(polkavm::SandboxKind::Generic));
            config.set_allow_experimental(true);
        }

        let engine = polkavm::Engine::new(&config).unwrap();
        let parsed = polkavm::ProgramBlob::parse(blob(route).to_vec().into()).unwrap();
        let module = polkavm::Module::from_blob(&engine, &polkavm::ModuleConfig::default(), parsed).unwrap();
        let linker = polkavm::Linker::<()>::new();
        let mut instance = linker.instantiate_pre(&module).unwrap().instantiate().unwrap();

        let input_ptr: u64 = instance.call_typed_and_get_result(&mut (), "input_ptr", ()).unwrap();
        let output_ptr: u64 = instance.call_typed_and_get_result(&mut (), "output_ptr", ()).unwrap();
        Guest {
            instance,
            input_ptr: input_ptr as u32,
            output_ptr: output_ptr as u32,
        }
    }

    fn write_input(&mut self, offset: u32, bytes: &[u8]) {
        self.instance.write_memory(self.input_ptr + offset, bytes).unwrap();
    }

    fn output32(&mut self) -> [u8; 32] {
        let bytes = self.instance.read_memory(self.output_ptr, 32).unwrap();
        bytes.try_into().unwrap()
    }

    /// Runs `point_op`/`scalar_op`, returning the status code and (on success)
    /// the 32-byte result.
    fn call_op(&mut self, export: &str, op: u64) -> (u64, [u8; 32]) {
        let status: u64 = self.instance.call_typed_and_get_result(&mut (), export, (op,)).unwrap();
        if status != 0 {
            return (status, [0; 32]);
        }
        (0, self.output32())
    }
}

const BACKENDS: [polkavm::BackendKind; 2] = [polkavm::BackendKind::Interpreter, polkavm::BackendKind::Compiler];

/// Deterministic pseudo-random bytes, distinct per (seed, index).
fn pseudo_random(seed: u64, index: usize) -> [u8; 32] {
    let mut state = seed ^ ((index as u64).wrapping_mul(0x9e3779b97f4a7c15));
    core::array::from_fn(|_| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state as u8
    })
}

/// The operand set every arithmetic test runs over: pseudo-random values plus
/// the structured cases where point code tends to break.
fn point_cases() -> Vec<([u8; 32], [u8; 32], [u8; 32], [u8; 32])> {
    use curve25519_dalek::constants::ED25519_BASEPOINT_POINT;
    use curve25519_dalek::edwards::EdwardsPoint;
    use curve25519_dalek::scalar::Scalar;

    let identity = EdwardsPoint::default().compress().to_bytes();
    let basepoint = ED25519_BASEPOINT_POINT.compress().to_bytes();
    // 2^252 + 27742317777372353535851937790883648493 - 1, i.e. l - 1: the
    // largest canonical scalar.
    let l_minus_one = (Scalar::ZERO - Scalar::ONE).to_bytes();

    let mut cases = Vec::new();
    for index in 0..24 {
        cases.push((
            pseudo_random(0xa1, index),
            pseudo_random(0xb2, index),
            pseudo_random(0xc3, index),
            pseudo_random(0xd4, index),
        ));
    }
    // Structured: identity, basepoint, and extreme scalars in every position.
    cases.push((identity, basepoint, [0; 32], l_minus_one));
    cases.push((basepoint, identity, l_minus_one, [0; 32]));
    cases.push((basepoint, basepoint, l_minus_one, l_minus_one));
    cases.push((identity, identity, [1; 32], [0; 32]));
    cases.push((basepoint, basepoint, [0; 32], [0; 32]));
    cases
}

/// The host-side reference for `point_op`, computed with stock 5×51 dalek.
/// Returns `None` when an operand is not a curve point — the guest reports that
/// as a nonzero status, and the two must agree.
fn reference_point_op(op: u64, a: &[u8; 32], b: &[u8; 32], s: &[u8; 32], t: &[u8; 32]) -> Option<[u8; 32]> {
    use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
    use curve25519_dalek::scalar::Scalar;

    let a_point = CompressedEdwardsY(*a).decompress()?;
    let s_scalar = Scalar::from_bytes_mod_order(*s);

    let result = match op {
        1 => a_point + a_point,
        2 => a_point * s_scalar,
        3 => EdwardsPoint::mul_base(&s_scalar),
        4 => EdwardsPoint::mul_base_clamped(*s),
        _ => {
            let b_point = CompressedEdwardsY(*b).decompress()?;
            let t_scalar = Scalar::from_bytes_mod_order(*t);
            match op {
                0 => a_point + b_point,
                5 => a_point - b_point,
                6 => a_point * s_scalar + b_point * t_scalar,
                7 => {
                    let mut acc = a_point;
                    for _ in 0..64 {
                        acc = acc + acc;
                    }
                    acc + b_point
                }
                _ => unreachable!("unknown point op {op}"),
            }
        }
    };
    Some(result.compress().to_bytes())
}

const POINT_OPS: [u64; 8] = [0, 1, 2, 3, 4, 5, 6, 7];

// ---------------------------------------------------------------------------
// Signature test vectors.
//
// ed25519: RFC 8032 test vector 3 (the same one the benchmarks use, so a
// failure here and a failure there mean the same thing). sr25519: the
// schnorrkel vector from bench-sr25519.
// ---------------------------------------------------------------------------

const ED_PUBLIC_KEY: [u8; 32] = [
    0xfc, 0x51, 0xcd, 0x8e, 0x62, 0x18, 0xa1, 0xa3, 0x8d, 0xa4, 0x7e, 0xd0, 0x02, 0x30, 0xf0, 0x58,
    0x08, 0x16, 0xed, 0x13, 0xba, 0x33, 0x03, 0xac, 0x5d, 0xeb, 0x91, 0x15, 0x48, 0x90, 0x80, 0x25,
];

const ED_SIGNATURE: [u8; 64] = [
    0x62, 0x91, 0xd6, 0x57, 0xde, 0xec, 0x24, 0x02, 0x48, 0x27, 0xe6, 0x9c, 0x3a, 0xbe, 0x01, 0xa3,
    0x0c, 0xe5, 0x48, 0xa2, 0x84, 0x74, 0x3a, 0x44, 0x5e, 0x36, 0x80, 0xd7, 0xdb, 0x5a, 0xc3, 0xac,
    0x18, 0xff, 0x9b, 0x53, 0x8d, 0x16, 0xf2, 0x90, 0xae, 0x67, 0xf7, 0x60, 0x98, 0x4d, 0xc6, 0x59,
    0x4a, 0x7c, 0x15, 0xe9, 0x71, 0x6e, 0xd2, 0x8d, 0xc0, 0x27, 0xbe, 0xce, 0xea, 0x1e, 0xc4, 0x0a,
];

const ED_MESSAGE: [u8; 2] = [0xaf, 0x82];

const SR_PUBLIC_KEY: [u8; 32] = [
    0x18, 0x9d, 0xac, 0x29, 0x29, 0x6d, 0x31, 0x81, 0x4d, 0xc8, 0xc5, 0x6c, 0xf3, 0xd3, 0x6a, 0x05,
    0x43, 0x37, 0x2b, 0xba, 0x75, 0x38, 0xfa, 0x32, 0x2a, 0x4a, 0xeb, 0xfe, 0xbc, 0x39, 0xe0, 0x56,
];

const SR_SIGNATURE: [u8; 64] = [
    0xb8, 0x8b, 0xbd, 0x69, 0xd3, 0x3b, 0xcd, 0xc7, 0xab, 0xd6, 0x87, 0xcb, 0x67, 0x52, 0x7d, 0xab,
    0x86, 0x76, 0x45, 0x43, 0x9a, 0x79, 0xd2, 0xd1, 0x79, 0x94, 0x74, 0x5b, 0x79, 0xed, 0x2a, 0x22,
    0x8a, 0x60, 0xe0, 0xf0, 0xc2, 0x4e, 0x5e, 0x98, 0x48, 0x34, 0xfd, 0x31, 0x7c, 0x3d, 0xcf, 0x19,
    0x47, 0x93, 0x6f, 0xf1, 0x59, 0x59, 0x0b, 0xfa, 0x92, 0x5e, 0x16, 0x2a, 0x75, 0x28, 0x56, 0x88,
];

const SR_MESSAGE: [u8; 32] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];

const SR_CONTEXT: &[u8] = b"substrate";

/// Counts how many times each wide-arithmetic opcode appears in a blob.
fn wide_opcode_counts(route: Route) -> std::collections::BTreeMap<&'static str, usize> {
    let parsed = polkavm::ProgramBlob::parse(blob(route).to_vec().into()).unwrap();
    let mut counts = std::collections::BTreeMap::new();
    for instruction in parsed.instructions() {
        let name = instruction.kind.opcode().name();
        if name.contains("256") {
            *counts.entry(name).or_insert(0) += 1;
        }
    }
    counts
}

/// The suite's own smoke test, and the reason it can be trusted at all.
///
/// If `RUSTFLAGS` ever stopped reaching the guest build, `Route::Redc` would
/// silently be stock dalek, every comparison in this file would be
/// dalek-vs-dalek, and the whole suite would pass while testing nothing. Assert
/// the instructions are actually in there.
#[test]
fn guest_routes_contain_the_expected_instructions() {
    let off = wide_opcode_counts(Route::Off);
    assert!(
        off.is_empty(),
        "the stock-dalek route must contain no wide-arithmetic instructions, found {off:?}"
    );

    let noredc = wide_opcode_counts(Route::NoRedc);
    for expected in ["mul256", "add256", "mul256_by_u64"] {
        assert!(
            noredc.contains_key(expected),
            "the generic route should use {expected}, found {noredc:?}"
        );
    }
    assert!(
        !noredc.contains_key("redc256") && !noredc.contains_key("mul256_redc256"),
        "the generic route must not use redc256, found {noredc:?}"
    );

    let redc = wide_opcode_counts(Route::Redc);
    for expected in ["mul256_redc256", "add256_redc256", "sub256_redc256"] {
        assert!(
            redc.contains_key(expected),
            "the redc route should use the fused {expected}, found {redc:?}"
        );
    }
}

#[test]
fn point_ops_match_host_reference() {
    let cases = point_cases();
    for backend in BACKENDS {
        let mut guest = Guest::new(Route::Redc, backend);
        for (index, (a, b, s, t)) in cases.iter().enumerate() {
            guest.write_input(0, a);
            guest.write_input(32, b);
            guest.write_input(64, s);
            guest.write_input(96, t);

            for op in POINT_OPS {
                let (status, actual) = guest.call_op("point_op", op);
                match reference_point_op(op, a, b, s, t) {
                    Some(expected) => {
                        assert_eq!(status, 0, "{backend:?} op {op} case {index}: guest rejected a valid point");
                        assert_eq!(actual, expected, "{backend:?} op {op} case {index}: wrong result");
                    }
                    None => assert_ne!(
                        status, 0,
                        "{backend:?} op {op} case {index}: guest accepted a non-curve point"
                    ),
                }
            }
        }
    }
}

impl Guest {
    /// Loads an ed25519 verification case and returns whether the guest
    /// accepted it.
    fn verify_ed25519(&mut self, export: &str, key: &[u8; 32], sig: &[u8; 64], msg: &[u8]) -> bool {
        self.write_input(0, key);
        self.write_input(32, sig);
        self.write_input(96, msg);
        let accepted: u32 = self
            .instance
            .call_typed_and_get_result(&mut (), export, (msg.len() as u64,))
            .unwrap();
        accepted == 1
    }

    fn verify_sr25519(&mut self, key: &[u8; 32], sig: &[u8; 64], msg: &[u8], ctx: &[u8]) -> bool {
        self.write_input(0, key);
        self.write_input(32, sig);
        self.write_input(96, msg);
        self.write_input(160, ctx);
        let accepted: u32 = self
            .instance
            .call_typed_and_get_result(&mut (), "verify_sr25519", (msg.len() as u64, ctx.len() as u64))
            .unwrap();
        accepted == 1
    }
}

/// Every single-byte mutation of a 64-byte signature that we bother to try:
/// one bit flipped in each byte. Exhaustive over positions, cheap in count.
fn signature_mutations(sig: &[u8; 64]) -> Vec<(String, [u8; 64])> {
    (0..64)
        .map(|i| {
            let mut mutated = *sig;
            mutated[i] ^= 1;
            (format!("signature byte {i}"), mutated)
        })
        .collect()
}

/// **The test the benchmarks cannot be**: a valid signature must verify, and
/// every tampered variant must be *rejected*.
///
/// Without this, field arithmetic broken in a collapsing way — everything
/// returning zero, or a final comparison that always succeeds — passes every
/// other test in this repository while accepting forgeries.
#[test]
fn ed25519_accepts_valid_and_rejects_tampered() {
    for backend in BACKENDS {
        for export in ["verify_ed25519_zebra", "verify_ed25519_dalek"] {
            let mut guest = Guest::new(Route::Redc, backend);
            let what = format!("{backend:?}/{export}");

            assert!(
                guest.verify_ed25519(export, &ED_PUBLIC_KEY, &ED_SIGNATURE, &ED_MESSAGE),
                "{what}: the valid RFC 8032 vector must verify"
            );

            for (label, mutated) in signature_mutations(&ED_SIGNATURE) {
                assert!(
                    !guest.verify_ed25519(export, &ED_PUBLIC_KEY, &mutated, &ED_MESSAGE),
                    "{what}: accepted a signature with {label} flipped"
                );
            }

            for i in 0..32 {
                let mut key = ED_PUBLIC_KEY;
                key[i] ^= 1;
                assert!(
                    !guest.verify_ed25519(export, &key, &ED_SIGNATURE, &ED_MESSAGE),
                    "{what}: accepted the signature under a key with byte {i} flipped"
                );
            }

            for i in 0..ED_MESSAGE.len() {
                let mut message = ED_MESSAGE;
                message[i] ^= 1;
                assert!(
                    !guest.verify_ed25519(export, &ED_PUBLIC_KEY, &ED_SIGNATURE, &message),
                    "{what}: accepted the signature over a message with byte {i} flipped"
                );
            }

            // Truncated and extended messages.
            assert!(
                !guest.verify_ed25519(export, &ED_PUBLIC_KEY, &ED_SIGNATURE, &ED_MESSAGE[..1]),
                "{what}: accepted the signature over a truncated message"
            );
            assert!(
                !guest.verify_ed25519(export, &ED_PUBLIC_KEY, &ED_SIGNATURE, &[0xaf, 0x82, 0x00]),
                "{what}: accepted the signature over an extended message"
            );

            // An all-zero key is the identity point: a degenerate public key
            // that must not validate this signature.
            assert!(
                !guest.verify_ed25519(export, &[0u8; 32], &ED_SIGNATURE, &ED_MESSAGE),
                "{what}: accepted the signature under an all-zero key"
            );
            // An all-zero signature likewise.
            assert!(
                !guest.verify_ed25519(export, &ED_PUBLIC_KEY, &[0u8; 64], &ED_MESSAGE),
                "{what}: accepted an all-zero signature"
            );
        }
    }
}

#[test]
fn sr25519_accepts_valid_and_rejects_tampered() {
    for backend in BACKENDS {
        let mut guest = Guest::new(Route::Redc, backend);
        let what = format!("{backend:?}/sr25519");

        assert!(
            guest.verify_sr25519(&SR_PUBLIC_KEY, &SR_SIGNATURE, &SR_MESSAGE, SR_CONTEXT),
            "{what}: the valid vector must verify"
        );

        for (label, mutated) in signature_mutations(&SR_SIGNATURE) {
            assert!(
                !guest.verify_sr25519(&SR_PUBLIC_KEY, &mutated, &SR_MESSAGE, SR_CONTEXT),
                "{what}: accepted a signature with {label} flipped"
            );
        }

        for i in 0..32 {
            let mut key = SR_PUBLIC_KEY;
            key[i] ^= 1;
            assert!(
                !guest.verify_sr25519(&key, &SR_SIGNATURE, &SR_MESSAGE, SR_CONTEXT),
                "{what}: accepted the signature under a key with byte {i} flipped"
            );
        }

        for i in [0usize, 15, 31] {
            let mut message = SR_MESSAGE;
            message[i] ^= 1;
            assert!(
                !guest.verify_sr25519(&SR_PUBLIC_KEY, &SR_SIGNATURE, &message, SR_CONTEXT),
                "{what}: accepted the signature over a message with byte {i} flipped"
            );
        }

        // The signing context is bound into the transcript, so a different
        // context must not validate.
        assert!(
            !guest.verify_sr25519(&SR_PUBLIC_KEY, &SR_SIGNATURE, &SR_MESSAGE, b"substrata"),
            "{what}: accepted the signature under a different signing context"
        );
    }
}

const SCALAR_OPS: [u64; 4] = [0, 1, 2, 3];

/// The host-side reference for `scalar_op`.
fn reference_scalar_op(op: u64, a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    use curve25519_dalek::scalar::Scalar;
    let a = Scalar::from_bytes_mod_order(*a);
    let b = Scalar::from_bytes_mod_order(*b);
    let result = match op {
        0 => a + b,
        1 => a - b,
        2 => a * b,
        3 => a * b.invert(),
        _ => unreachable!("unknown scalar op {op}"),
    };
    result.to_bytes()
}

#[test]
fn scalar_ops_match_host_reference() {
    let cases = point_cases();
    for backend in BACKENDS {
        let mut guest = Guest::new(Route::Redc, backend);
        for (index, (a, b, _, _)) in cases.iter().enumerate() {
            guest.write_input(0, a);
            guest.write_input(32, b);
            for op in SCALAR_OPS {
                let (status, actual) = guest.call_op("scalar_op", op);
                assert_eq!(status, 0, "{backend:?} scalar op {op} case {index}: unexpected status");
                assert_eq!(
                    actual,
                    reference_scalar_op(op, a, b),
                    "{backend:?} scalar op {op} case {index}: wrong result"
                );
            }
        }
    }
}

/// The two reduction routes must agree with each other, on target, with no host
/// reference involved.
///
/// This is the only check that pits the **fused** fold opcodes
/// (`mul256_redc256`, `add256_redc256`, `sub256_redc256`) directly against the
/// unfused instruction sequences computing the same field arithmetic. A bug
/// present in both routes would escape it — that is what the host-reference
/// tests above are for — but a bug in either route alone cannot hide here.
#[test]
fn redc_route_matches_generic_route() {
    let cases = point_cases();
    for backend in BACKENDS {
        let mut redc = Guest::new(Route::Redc, backend);
        let mut generic = Guest::new(Route::NoRedc, backend);

        for (index, (a, b, s, t)) in cases.iter().enumerate() {
            for guest in [&mut redc, &mut generic] {
                guest.write_input(0, a);
                guest.write_input(32, b);
                guest.write_input(64, s);
                guest.write_input(96, t);
            }

            for op in POINT_OPS {
                let left = redc.call_op("point_op", op);
                let right = generic.call_op("point_op", op);
                assert_eq!(
                    left, right,
                    "{backend:?} point op {op} case {index}: redc and generic routes disagree"
                );
            }
            for op in SCALAR_OPS {
                let left = redc.call_op("scalar_op", op);
                let right = generic.call_op("scalar_op", op);
                assert_eq!(
                    left, right,
                    "{backend:?} scalar op {op} case {index}: redc and generic routes disagree"
                );
            }
        }
    }
}

/// The stock-dalek route is a third independent implementation (5x51 limbs, no
/// wide instructions) running in the same VM. If it disagrees with the redc
/// route, the wide-arithmetic backend is wrong — and unlike the host reference,
/// this comparison is immune to anything the *host* toolchain does differently.
#[test]
fn redc_route_matches_stock_dalek_in_vm() {
    let cases = point_cases();
    for backend in BACKENDS {
        let mut redc = Guest::new(Route::Redc, backend);
        let mut stock = Guest::new(Route::Off, backend);

        for (index, (a, b, s, t)) in cases.iter().enumerate() {
            for guest in [&mut redc, &mut stock] {
                guest.write_input(0, a);
                guest.write_input(32, b);
                guest.write_input(64, s);
                guest.write_input(96, t);
            }
            for op in POINT_OPS {
                assert_eq!(
                    redc.call_op("point_op", op),
                    stock.call_op("point_op", op),
                    "{backend:?} point op {op} case {index}: redc route disagrees with stock dalek"
                );
            }
        }
    }
}
