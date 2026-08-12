//! Measures the overhead of dispatching a host call from parachain code
//! through two VM boundaries.
//!
//! In JAM, parachain code runs in an inner PVM that cannot reach the node: a
//! parachain service (itself a PVM guest) mediates every host call the inner
//! program makes. This tool measures what that mediation costs, decomposed
//! into a fixed cost per boundary crossing and a per-byte copy cost. There is
//! no crypto anywhere: the node-side "crypto" is a stub that folds the payload
//! into a `u64`.
//!
//! The mediation protocol (`machine`/`peek`/`poke`/`invoke`) mirrors the shape
//! and the copy semantics of the real node's implementation in
//! `polkajam/crates/node/src/chain/exec/vm/host.rs` — including the two copies
//! per peek and per poke, and the 112-byte `invoke` argument block — but does
//! not import any of its code.
//!
//! Numbers of record require `--linux` on the bare-metal rig. The generic
//! sandbox runs in-process, so it has no crossing cost worth the name; in a
//! container it is a correctness and copy-slope pre-screen only.
//!
//! See designs/parachain-service-on-jam/nested-call-overhead-handoff.md.

use polkavm::{
    Config, Engine, GasMeteringKind, InterruptKind, MemoryProtection, Module, ModuleConfig, ProgramBlob, ProgramCounter,
    RawInstance, Reg,
};
use std::time::Instant;

/// Matches `MAX_PAYLOAD` in both guest programs.
const MAX_PAYLOAD: u64 = 65536;

/// Matches `MAX_RESULT` in both guest programs.
const MAX_RESULT: u64 = 32;

/// Matches `FOLD_SEED` in `bench-nested-caller`.
const FOLD_SEED: u64 = 0x9e37_79b9_7f4a_7c15;

/// `invoke`'s argument block: gas plus the 13 registers.
const INVOKE_ARGS_BYTESIZE: u32 = 8 + 8 * 13;

/// Gas handed to every guest run; the harness is not measuring gas.
const GAS_LIMIT: i64 = i64::MAX / 4;

/// Field offsets in `bench-nested-caller`'s `Header`.
mod caller {
    // 0: payload_ptr and 8: scratch_ptr are for the guest's own use.
    pub const CALLS_PER_RUN: u32 = 16;
    pub const PAYLOAD_BYTES: u32 = 24;
    pub const RESULT: u32 = 32;
    pub const COMPUTE_ITERATIONS: u32 = 40;
    pub const COMPUTE_RESULT: u32 = 48;
}

/// Field offsets in `bench-nested-mediator`'s `Header`.
mod mediator {
    pub const GAS: u32 = 8;
    pub const INNER_RA: u32 = 16;
    pub const INNER_SP: u32 = 24;
    pub const RESULT_BYTES: u32 = 32;
    pub const PEEK_MODE: u32 = 40;
    pub const MEDIATED: u32 = 48;
    pub const OUTCOME: u32 = 56;
}

// ---------------------------------------------------------------------------
// Command line
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    OneJump,
    TwoJump,
    NestingTax,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Peeks {
    Packed,
    PerArg,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sweep {
    Calls,
    Payload,
}

/// What a single measurement runs: with or without the mediator, and which of
/// the caller's entry points.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Variant {
    mediated: bool,
    compute: bool,
}

#[derive(Clone)]
struct Args {
    mode: Mode,
    payload: u64,
    result_bytes: u64,
    calls_per_run: u64,
    compute_iterations: u64,
    peeks: Peeks,
    linux_sandbox: bool,
    nested_instantiate: bool,
    inner_dynamic_paging: bool,
    phases: bool,
    batch: u32,
    batches: u32,
    sweep: Option<Sweep>,
    selftest: bool,
    caller_blob: String,
    mediator_blob: String,
}

const USAGE: &str = "\
usage: nested-call [options] <bench-nested-caller.polkavm> <bench-nested-mediator.polkavm>

  --mode one-jump|two-jump|nesting-tax   what to run (default: two-jump)
  --payload N                            argument bytes per stub call (default: 128)
  --result-bytes R                       result bytes copied back, 8..=32 (default: 8)
  --calls-per-run K                      stub calls per guest run (default: 16)
  --compute-iterations N                 nesting-tax compute loop length (default: 100000)
  --peeks packed|per-arg                 one peek or three (default: packed)
  --linux | --generic                    sandbox (default: generic)
  --plain-instantiate                    inner VM via instantiate() instead of instantiate_nested()
  --inner-dynamic-paging                 dynamic paging for the inner VM, as the node does
  --phases                               per-phase timing inside the mediation path
  --batch N / --batches N                timing knobs (default: 200 / 30)
  --sweep calls|payload                  run the standard sweep and fit a line
  --selftest                             run the whole matrix once and check the checksums
";

fn parse_args() -> Args {
    let mut args = Args {
        mode: Mode::TwoJump,
        payload: 128,
        result_bytes: 8,
        calls_per_run: 16,
        compute_iterations: 100_000,
        peeks: Peeks::Packed,
        linux_sandbox: false,
        nested_instantiate: true,
        inner_dynamic_paging: false,
        phases: false,
        batch: 200,
        batches: 30,
        sweep: None,
        selftest: false,
        caller_blob: String::new(),
        mediator_blob: String::new(),
    };

    let mut positional = Vec::new();
    let mut raw = std::env::args().skip(1);
    while let Some(arg) = raw.next() {
        let mut value = || raw.next().unwrap_or_else(|| fail(&format!("{arg} needs a value")));
        match arg.as_str() {
            "--mode" => {
                args.mode = match value().as_str() {
                    "one-jump" => Mode::OneJump,
                    "two-jump" => Mode::TwoJump,
                    "nesting-tax" => Mode::NestingTax,
                    other => fail(&format!("unknown mode: {other}")),
                }
            }
            "--payload" => args.payload = parse_u64(&value()),
            "--result-bytes" => args.result_bytes = parse_u64(&value()),
            "--calls-per-run" => args.calls_per_run = parse_u64(&value()),
            "--compute-iterations" => args.compute_iterations = parse_u64(&value()),
            "--peeks" => {
                args.peeks = match value().as_str() {
                    "packed" => Peeks::Packed,
                    "per-arg" => Peeks::PerArg,
                    other => fail(&format!("unknown peek strategy: {other}")),
                }
            }
            "--linux" => args.linux_sandbox = true,
            "--generic" => args.linux_sandbox = false,
            "--plain-instantiate" => args.nested_instantiate = false,
            "--nested-instantiate" => args.nested_instantiate = true,
            "--inner-dynamic-paging" => args.inner_dynamic_paging = true,
            "--phases" => args.phases = true,
            "--batch" => args.batch = parse_u64(&value()) as u32,
            "--batches" => args.batches = parse_u64(&value()) as u32,
            "--sweep" => {
                args.sweep = Some(match value().as_str() {
                    "calls" | "k" => Sweep::Calls,
                    "payload" | "n" => Sweep::Payload,
                    other => fail(&format!("unknown sweep: {other}")),
                })
            }
            "--selftest" => args.selftest = true,
            "--help" | "-h" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other if other.starts_with("--") => fail(&format!("unknown option: {other}")),
            other => positional.push(other.to_owned()),
        }
    }

    if positional.len() != 2 {
        eprint!("{USAGE}");
        std::process::exit(1);
    }

    args.caller_blob = positional[0].clone();
    args.mediator_blob = positional[1].clone();

    if args.payload < 8 || args.payload > MAX_PAYLOAD {
        fail("--payload must be between 8 and 65536");
    }
    if args.result_bytes < 8 || args.result_bytes > MAX_RESULT {
        fail("--result-bytes must be between 8 and 32");
    }
    if args.calls_per_run == 0 {
        fail("--calls-per-run must be at least 1");
    }
    if args.batch == 0 || args.batches == 0 {
        fail("--batch and --batches must be at least 1");
    }

    args
}

fn parse_u64(value: &str) -> u64 {
    value.parse().unwrap_or_else(|_| fail(&format!("not a number: {value}")))
}

fn fail(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(1);
}

fn main() {
    env_logger::init();
    let args = parse_args();

    if args.selftest {
        selftest(&args);
        return;
    }

    print_configuration(&args);

    match (args.sweep, args.mode) {
        (Some(Sweep::Calls), _) => sweep_calls(&args),
        (Some(Sweep::Payload), _) => sweep_payload(&args),
        (None, Mode::NestingTax) => nesting_tax(&args),
        (None, mode) => {
            let variant = Variant {
                mediated: mode == Mode::TwoJump,
                compute: false,
            };
            let measurement = measure(&args, variant);
            println!();
            println!(
                "best {:.0} ns/run, mean {:.0} ns/run over {} batches of {}",
                measurement.best_ns, measurement.mean_ns, args.batches, args.batch
            );
            println!(
                "     {:.1} ns/call, {:.3} us/call  ({} calls per run)",
                measurement.per_call_ns(),
                measurement.per_call_ns() / 1000.0,
                args.calls_per_run
            );
            if args.phases {
                measurement.print_phases(args.calls_per_run * u64::from(args.batch) * u64::from(args.batches));
            }
        }
    }
}

fn print_configuration(args: &Args) {
    println!(
        "mode {:?}, sandbox {}, payload {} B, result {} B, K {}, peeks {:?}, {}{}",
        args.mode,
        if args.linux_sandbox { "linux" } else { "generic" },
        args.payload,
        args.result_bytes,
        args.calls_per_run,
        args.peeks,
        if args.nested_instantiate {
            "instantiate_nested"
        } else {
            "instantiate"
        },
        if args.inner_dynamic_paging {
            ", inner dynamic paging"
        } else {
            ""
        },
    );
    if !args.linux_sandbox {
        println!("NOTE: the generic sandbox runs in-process; its crossing costs are not the rig's.");
    }
    if args.phases {
        println!(
            "NOTE: --phases instruments the measured loop; timer pair costs {:.1} ns. Headline wall numbers should come from a run without it.",
            timer_overhead_ns()
        );
    }
}

// ---------------------------------------------------------------------------
// Measurement
// ---------------------------------------------------------------------------

/// Per-phase totals over a measured run, in nanoseconds, with the number of
/// `Instant` pairs each was built from so the timer's own cost is visible.
#[derive(Default, Clone)]
struct Phases {
    enabled: bool,
    outer_guest: u128,
    outer_guest_n: u64,
    invoke: u128,
    invoke_n: u64,
    inner_guest: u128,
    inner_guest_n: u64,
    peek: u128,
    peek_n: u64,
    poke: u128,
    poke_n: u64,
    stub: u128,
    stub_n: u64,
}

impl Phases {
    fn reset(&mut self) {
        let enabled = self.enabled;
        *self = Phases::default();
        self.enabled = enabled;
    }
}

struct Measurement {
    best_ns: f64,
    mean_ns: f64,
    calls_per_run: u64,
    phases: Phases,
    compute_result: u64,
}

impl Measurement {
    fn per_call_ns(&self) -> f64 {
        self.best_ns / self.calls_per_run as f64
    }

    fn print_phases(&self, total_calls: u64) {
        let overhead = timer_overhead_ns();
        let calls = total_calls as f64;
        println!();
        println!("phase decomposition, per mediated call (timer pair costs {overhead:.1} ns):");
        println!("  {:<26} {:>9} {:>11} {:>13}", "phase", "per call", "raw ns", "corrected ns");
        let row = |name: &str, total: u128, count: u64| {
            if count == 0 {
                return;
            }
            let raw = total as f64 / calls;
            let corrected = (total as f64 - count as f64 * overhead) / calls;
            let note = if corrected <= 0.0 { "  (below timer resolution)" } else { "" };
            println!(
                "  {name:<26} {:>9.2} {:>11.1} {:>13.1}{note}",
                count as f64 / calls,
                raw,
                corrected
            );
        };
        row("outer run (crossing+guest)", self.phases.outer_guest, self.phases.outer_guest_n);
        row("invoke (total)", self.phases.invoke, self.phases.invoke_n);
        row("  of which inner run", self.phases.inner_guest, self.phases.inner_guest_n);
        row("peek", self.phases.peek, self.phases.peek_n);
        row("poke", self.phases.poke, self.phases.poke_n);
        row("stub", self.phases.stub, self.phases.stub_n);
        println!(
            "  \"per call\" for the outer run is the number of service<->node crossings per mediated call."
        );
    }
}

fn measure(args: &Args, variant: Variant) -> Measurement {
    let engine = build_engine(args);
    let mut harness = Harness::new(&engine, args, variant);

    for _ in 0..args.batch {
        harness.iterate();
    }
    harness.verify(args, variant);

    harness.phases.reset();
    let mut best_ns = f64::INFINITY;
    let mut sum_ns = 0.0;
    for _ in 0..args.batches {
        let start = Instant::now();
        for _ in 0..args.batch {
            harness.iterate();
        }
        let ns = start.elapsed().as_nanos() as f64 / f64::from(args.batch);
        best_ns = best_ns.min(ns);
        sum_ns += ns;
    }

    // Nothing drifted while it was being timed.
    harness.verify(args, variant);

    Measurement {
        best_ns,
        mean_ns: sum_ns / f64::from(args.batches),
        calls_per_run: if variant.compute { 1 } else { args.calls_per_run },
        phases: harness.phases.clone(),
        compute_result: harness.compute_result(),
    }
}

/// The cost of one `Instant::now()` + `elapsed()` pair, so phase sums can be
/// corrected rather than trusted. Measured once per process, because the two
/// places that report it must not disagree.
fn timer_overhead_ns() -> f64 {
    static OVERHEAD: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *OVERHEAD.get_or_init(|| {
        const N: u32 = 200_000;
        let mut acc = 0u128;
        let start = Instant::now();
        for _ in 0..N {
            let t = Instant::now();
            acc += t.elapsed().as_nanos();
        }
        let total = start.elapsed().as_nanos() as f64;
        std::hint::black_box(acc);
        total / f64::from(N)
    })
}

fn nesting_tax(args: &Args) {
    let flat = measure(
        args,
        Variant {
            mediated: false,
            compute: true,
        },
    );
    let nested = measure(
        args,
        Variant {
            mediated: true,
            compute: true,
        },
    );

    assert_eq!(
        flat.compute_result, nested.compute_result,
        "the compute loop produced different results flat vs nested"
    );

    let delta = nested.best_ns - flat.best_ns;
    println!();
    println!("flat    {:>10.0} ns/run (mean {:.0})", flat.best_ns, flat.mean_ns);
    println!("nested  {:>10.0} ns/run (mean {:.0})", nested.best_ns, nested.mean_ns);
    println!(
        "tax     {:>10.0} ns/run ({:+.2}%), one invoke per run, {} compute iterations",
        delta,
        100.0 * delta / flat.best_ns,
        args.compute_iterations
    );
}

fn sweep_calls(args: &Args) {
    const CALLS: [u64; 7] = [1, 2, 4, 8, 16, 32, 64];
    let variant = Variant {
        mediated: args.mode == Mode::TwoJump,
        compute: false,
    };

    println!();
    println!("{:>6}  {:>12}  {:>12}", "K", "ns/run", "ns/call");
    let mut points = Vec::new();
    for calls in CALLS {
        let mut args = args.clone();
        args.calls_per_run = calls;
        let measurement = measure(&args, variant);
        println!(
            "{calls:>6}  {:>12.0}  {:>12.1}",
            measurement.best_ns,
            measurement.best_ns / calls as f64
        );
        points.push((calls as f64, measurement.best_ns));
    }

    let (slope, intercept) = least_squares(&points);
    println!();
    println!("fit: {slope:.1} ns per mediated call + {intercept:.0} ns fixed per run");
    println!("     = {:.3} us per mediated call", slope / 1000.0);
}

fn sweep_payload(args: &Args) {
    const PAYLOADS: [u64; 6] = [32, 96, 128, 1024, 4096, 65536];
    let variant = Variant {
        mediated: args.mode == Mode::TwoJump,
        compute: false,
    };

    println!();
    println!("{:>8}  {:>12}  {:>12}", "payload", "ns/run", "ns/call");
    let mut points = Vec::new();
    for payload in PAYLOADS {
        let mut args = args.clone();
        args.payload = payload;
        let measurement = measure(&args, variant);
        println!(
            "{payload:>8}  {:>12.0}  {:>12.1}",
            measurement.best_ns,
            measurement.per_call_ns()
        );
        points.push((payload as f64, measurement.per_call_ns()));
    }

    let (slope, intercept) = least_squares(&points);
    println!();
    println!("fit over the whole sweep: {:.4} ns per byte + {intercept:.1} ns fixed per call", slope);
    let small = &points[..3];
    let (slope, intercept) = least_squares(small);
    println!("fit over 32..128 B only:  {:.4} ns per byte + {intercept:.1} ns fixed per call", slope);
}

fn least_squares(points: &[(f64, f64)]) -> (f64, f64) {
    let n = points.len() as f64;
    let mean_x = points.iter().map(|(x, _)| x).sum::<f64>() / n;
    let mean_y = points.iter().map(|(_, y)| y).sum::<f64>() / n;
    let covariance: f64 = points.iter().map(|(x, y)| (x - mean_x) * (y - mean_y)).sum();
    let variance: f64 = points.iter().map(|(x, _)| (x - mean_x).powi(2)).sum();
    let slope = covariance / variance;
    (slope, mean_y - slope * mean_x)
}

/// Runs the whole matrix once with a tiny batch and checks every checksum.
/// One command to run before every measurement session; also the stale-blob
/// tell.
fn selftest(args: &Args) {
    let mut args = args.clone();
    args.batch = 2;
    args.batches = 2;
    args.compute_iterations = 1000;

    for &payload in &[32u64, 96, 128, 1024, 4096, 65536] {
        for &peeks in &[Peeks::Packed, Peeks::PerArg] {
            for &result_bytes in &[8u64, 32] {
                for &calls in &[1u64, 5] {
                    let mut args = args.clone();
                    args.payload = payload;
                    args.peeks = peeks;
                    args.result_bytes = result_bytes;
                    args.calls_per_run = calls;

                    for (label, mediated) in [("one-jump", false), ("two-jump", true)] {
                        args.mode = if mediated { Mode::TwoJump } else { Mode::OneJump };
                        measure(
                            &args,
                            Variant {
                                mediated,
                                compute: false,
                            },
                        );
                        println!("ok  {label:<9} payload {payload:>5} B, {peeks:?}, result {result_bytes} B, K {calls}");
                    }
                }
            }
        }
    }

    let flat = measure(
        &args,
        Variant {
            mediated: false,
            compute: true,
        },
    );
    let nested = measure(
        &args,
        Variant {
            mediated: true,
            compute: true,
        },
    );
    assert_eq!(flat.compute_result, nested.compute_result, "compute result differs");
    println!("ok  nesting-tax compute result matches flat vs nested");
    println!("\nself-test passed");
}

// ---------------------------------------------------------------------------
// The harness
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum HostCall {
    Machine,
    Peek,
    Poke,
    Invoke,
    Stub,
}

struct Harness {
    inner: RawInstance,
    outer: Option<RawInstance>,
    inner_imports: Vec<Option<HostCall>>,
    outer_imports: Vec<Option<HostCall>>,
    inner_header: u32,
    outer_header: u32,
    inner_sp: u64,
    outer_sp: u64,
    inner_entry_pc: ProgramCounter,
    outer_run_pc: ProgramCounter,
    result_bytes: u64,
    dynamic_paging: bool,
    /// Reused so the measured loop allocates no more than the mediation
    /// protocol itself does.
    result_buffer: Vec<u8>,
    phases: Phases,
}

fn build_engine(args: &Args) -> Engine {
    let mut config = Config::from_env().expect("invalid config");
    config.set_backend(Some(polkavm::BackendKind::Compiler));
    if args.linux_sandbox {
        config.set_sandbox(Some(polkavm::SandboxKind::Linux));
    } else {
        config.set_sandbox(Some(polkavm::SandboxKind::Generic));
        config.set_allow_experimental(true);
    }
    if args.inner_dynamic_paging {
        config.set_allow_dynamic_paging(true);
    }
    Engine::new(&config).unwrap_or_else(|error| {
        if args.linux_sandbox {
            fail(&format!(
                "{error}\nthe Linux sandbox needs `clone`, which dev containers don't allow; \
                 pre-screen with --generic, but every number of record needs --linux on the rig"
            ));
        }
        fail(&format!("failed to create engine: {error}"))
    })
}

fn load_module(engine: &Engine, path: &str, dynamic_paging: bool) -> Module {
    let raw = std::fs::read(path).unwrap_or_else(|error| fail(&format!("failed to read {path}: {error}")));
    let blob = ProgramBlob::parse(raw.into()).unwrap_or_else(|error| fail(&format!("failed to parse {path}: {error}")));

    // Sync gas metering on both VMs, matching the node's configuration for
    // services and for the machines they spawn.
    let mut module_config = ModuleConfig::new();
    module_config.set_gas_metering(Some(GasMeteringKind::Sync));
    module_config.set_dynamic_paging(dynamic_paging);
    Module::from_blob(engine, &module_config, blob).unwrap_or_else(|error| fail(&format!("failed to compile {path}: {error}")))
}

fn export_pc(module: &Module, name: &str) -> ProgramCounter {
    module
        .exports()
        .find(|export| export.symbol().as_bytes() == name.as_bytes())
        .map(|export| export.program_counter())
        .unwrap_or_else(|| fail(&format!("export not found: {name}")))
}

/// Maps ecalli indices to host calls by symbol, the way the node's dispatch
/// table does.
fn import_table(module: &Module) -> Vec<Option<HostCall>> {
    module
        .imports()
        .iter()
        .map(|symbol| {
            symbol.and_then(|symbol| match symbol.as_bytes() {
                b"machine" => Some(HostCall::Machine),
                b"peek" => Some(HostCall::Peek),
                b"poke" => Some(HostCall::Poke),
                b"invoke" => Some(HostCall::Invoke),
                b"stub" => Some(HostCall::Stub),
                _ => None,
            })
        })
        .collect()
}

impl Harness {
    fn new(engine: &Engine, args: &Args, variant: Variant) -> Harness {
        let caller_module = load_module(engine, &args.caller_blob, args.inner_dynamic_paging);
        let inner_sp = caller_module.default_sp();
        let inner_entry_pc = if variant.compute {
            export_pc(&caller_module, "compute")
        } else {
            export_pc(&caller_module, "run")
        };

        let mediator_module = variant
            .mediated
            .then(|| load_module(engine, &args.mediator_blob, false));

        let mut outer = mediator_module
            .as_ref()
            .map(|module| module.instantiate().expect("failed to instantiate the mediator"));

        // The inner VM is created up front so that `instantiate_nested` can see
        // the outer instance; the mediator's `machine` host call then just
        // hands back its handle. Spawning is not on the measured path.
        let mut inner = match (&mut outer, args.nested_instantiate) {
            (Some(outer), true) => caller_module
                .instantiate_nested(outer)
                .expect("failed to instantiate the caller"),
            _ => caller_module.instantiate().expect("failed to instantiate the caller"),
        };

        let dynamic_paging = args.inner_dynamic_paging;

        // Fill the payload and learn where the guest put its control block.
        let inner_header = {
            let initialize_pc = export_pc(&caller_module, "initialize");
            prepare_call(&mut inner, initialize_pc, inner_sp);
            run_to_completion(&mut inner, dynamic_paging);
            inner.reg(Reg::A0) as u32
        };

        let (outer_run_pc, outer_sp) = match mediator_module.as_ref() {
            Some(module) => (export_pc(module, "run"), module.default_sp()),
            None => (inner_entry_pc, inner_sp),
        };

        let mut harness = Harness {
            inner,
            outer,
            inner_imports: import_table(&caller_module),
            outer_imports: mediator_module.as_ref().map(import_table).unwrap_or_default(),
            inner_header,
            outer_header: 0,
            inner_sp,
            outer_sp,
            inner_entry_pc,
            outer_run_pc,
            result_bytes: args.result_bytes,
            dynamic_paging,
            result_buffer: vec![0; MAX_RESULT as usize],
            phases: Phases {
                enabled: args.phases,
                ..Phases::default()
            },
        };

        if let Some(module) = mediator_module.as_ref() {
            harness.initialize_mediator(module, args);
        }
        harness.configure(args);
        harness
    }

    /// Runs the mediator's `initialize` (which calls `machine`) and fills in
    /// its control block.
    fn initialize_mediator(&mut self, module: &Module, args: &Args) {
        let initialize_pc = export_pc(module, "initialize");
        let outer_sp = self.outer_sp;
        prepare_call(self.outer.as_mut().expect("no mediator"), initialize_pc, outer_sp);
        self.run_outer();

        let outer = self.outer.as_mut().expect("no mediator");
        let header = outer.reg(Reg::A0) as u32;
        outer.write_u64(header + mediator::GAS, GAS_LIMIT as u64).expect("bad header");
        outer
            .write_u64(header + mediator::INNER_RA, u64::from(polkavm::RETURN_TO_HOST))
            .expect("bad header");
        outer
            .write_u64(header + mediator::INNER_SP, self.inner_sp)
            .expect("bad header");
        outer
            .write_u64(header + mediator::RESULT_BYTES, args.result_bytes)
            .expect("bad header");
        self.outer_header = header;
    }

    /// Applies the knobs that can change between measured configurations.
    fn configure(&mut self, args: &Args) {
        self.result_bytes = args.result_bytes;

        let header = self.inner_header;
        self.inner
            .write_u64(header + caller::CALLS_PER_RUN, args.calls_per_run)
            .expect("bad header");
        self.inner
            .write_u64(header + caller::PAYLOAD_BYTES, args.payload)
            .expect("bad header");
        self.inner
            .write_u64(header + caller::COMPUTE_ITERATIONS, args.compute_iterations)
            .expect("bad header");

        let outer_header = self.outer_header;
        let peek_mode = u64::from(args.peeks == Peeks::PerArg);
        if let Some(outer) = self.outer.as_mut() {
            outer
                .write_u64(outer_header + mediator::RESULT_BYTES, args.result_bytes)
                .expect("bad header");
            outer
                .write_u64(outer_header + mediator::PEEK_MODE, peek_mode)
                .expect("bad header");
        }
    }

    /// One measured iteration: one guest `run()`.
    fn iterate(&mut self) {
        if self.outer.is_some() {
            self.iterate_mediated();
        } else {
            self.iterate_flat();
        }
    }

    /// `one-jump` and the flat half of `nesting-tax`: a single VM whose stub
    /// host call the node serves directly.
    fn iterate_flat(&mut self) {
        let pc = self.inner_entry_pc;
        let sp = self.inner_sp;
        prepare_call(&mut self.inner, pc, sp);
        loop {
            match self.inner.run().expect("run failed") {
                InterruptKind::Finished => break,
                InterruptKind::Ecalli(index) => {
                    match self.inner_imports.get(index as usize).copied().flatten() {
                        Some(HostCall::Stub) => {}
                        _ => panic!("unexpected host call from the caller: {index}"),
                    }
                    let start = self.phases.enabled.then(Instant::now);
                    let ptr = self.inner.reg(Reg::A0) as u32;
                    let len = self.inner.reg(Reg::A1) as u32;
                    let dst = self.inner.reg(Reg::A2) as u32;
                    let result_bytes = self.result_bytes;
                    host_stub(&mut self.inner, ptr, len, dst, result_bytes, &mut self.result_buffer);
                    if let Some(start) = start {
                        self.phases.stub += start.elapsed().as_nanos();
                        self.phases.stub_n += 1;
                    }
                }
                InterruptKind::Segfault(segfault) if self.dynamic_paging => allocate_page(&mut self.inner, segfault),
                other => panic!("unexpected interrupt from the caller: {other:?}"),
            }
        }
    }

    /// `two-jump` and the nested half of `nesting-tax`: the mediator drives the
    /// inner VM and the node serves the mediator.
    fn iterate_mediated(&mut self) {
        // A fresh inner run. This is the one place per iteration where the
        // inner VM's program counter is touched, i.e. the one place that pays
        // the sandbox's expensive re-entry path instead of the low-latency
        // resume; it is constant in the number of mediated calls and therefore
        // cancels out of the per-call slope.
        self.inner.set_next_program_counter(self.inner_entry_pc);

        let outer_run_pc = self.outer_run_pc;
        let outer_sp = self.outer_sp;
        prepare_call(self.outer.as_mut().expect("no mediator"), outer_run_pc, outer_sp);
        self.run_outer();
    }

    /// Drives the mediator to completion, serving its host calls.
    fn run_outer(&mut self) {
        loop {
            let start = self.phases.enabled.then(Instant::now);
            let interrupt = self.outer.as_mut().expect("no mediator").run().expect("run failed");
            if let Some(start) = start {
                self.phases.outer_guest += start.elapsed().as_nanos();
                self.phases.outer_guest_n += 1;
            }

            match interrupt {
                InterruptKind::Finished => break,
                InterruptKind::Ecalli(index) => self.serve_mediator_host_call(index),
                other => panic!("unexpected interrupt from the mediator: {other:?}"),
            }
        }
    }

    fn serve_mediator_host_call(&mut self, index: u32) {
        let call = self
            .outer_imports
            .get(index as usize)
            .copied()
            .flatten()
            .unwrap_or_else(|| panic!("unexpected host call from the mediator: {index}"));

        let result_bytes = self.result_bytes;
        let dynamic_paging = self.dynamic_paging;
        let Harness {
            inner,
            outer,
            result_buffer,
            phases,
            ..
        } = self;
        let outer = outer.as_mut().expect("no mediator");

        match call {
            // The handle is a formality: this harness runs exactly one inner
            // VM, created before the mediator started.
            HostCall::Machine => outer.set_reg(Reg::A0, 0),

            HostCall::Peek => {
                let start = phases.enabled.then(Instant::now);
                let dst = outer.reg(Reg::A1) as u32;
                let src = outer.reg(Reg::A2) as u32;
                let len = outer.reg(Reg::A3) as u32;
                // Two copies, as in the node: out of the inner VM into a fresh
                // allocation, then into the service's memory.
                let chunk = inner.read_memory(src, len).expect("peek: bad inner read");
                outer.write_memory(dst, &chunk).expect("peek: bad outer write");
                outer.set_reg(Reg::A0, 0);
                if let Some(start) = start {
                    phases.peek += start.elapsed().as_nanos();
                    phases.peek_n += 1;
                }
            }

            HostCall::Poke => {
                let start = phases.enabled.then(Instant::now);
                let src = outer.reg(Reg::A1) as u32;
                let dst = outer.reg(Reg::A2) as u32;
                let len = outer.reg(Reg::A3) as u32;
                let chunk = outer.read_memory(src, len).expect("poke: bad outer read");
                inner.write_memory(dst, &chunk).expect("poke: bad inner write");
                outer.set_reg(Reg::A0, 0);
                if let Some(start) = start {
                    phases.poke += start.elapsed().as_nanos();
                    phases.poke_n += 1;
                }
            }

            HostCall::Stub => {
                let start = phases.enabled.then(Instant::now);
                let ptr = outer.reg(Reg::A0) as u32;
                let len = outer.reg(Reg::A1) as u32;
                let dst = outer.reg(Reg::A2) as u32;
                host_stub(outer, ptr, len, dst, result_bytes, result_buffer);
                if let Some(start) = start {
                    phases.stub += start.elapsed().as_nanos();
                    phases.stub_n += 1;
                }
            }

            HostCall::Invoke => {
                let start = phases.enabled.then(Instant::now);
                let args_ptr = outer.reg(Reg::A1) as u32;
                let mut raw = outer
                    .read_memory(args_ptr, INVOKE_ARGS_BYTESIZE)
                    .expect("invoke: bad args read");

                inner.set_gas(i64::from_le_bytes(raw[0..8].try_into().unwrap()));
                for (i, reg) in Reg::ALL.into_iter().enumerate() {
                    let offset = 8 + i * 8;
                    inner.set_reg(reg, u64::from_le_bytes(raw[offset..offset + 8].try_into().unwrap()));
                }

                let inner_start = phases.enabled.then(Instant::now);
                let mut interrupt = inner.run().expect("invoke: inner run failed");
                while dynamic_paging {
                    // A real service handles page faults with the `pages` host
                    // call; the harness does it host-side, which is why the
                    // dynamic-paging knob is a fidelity check and not a
                    // measured configuration.
                    let InterruptKind::Segfault(segfault) = interrupt else {
                        break;
                    };
                    allocate_page(inner, segfault);
                    interrupt = inner.run().expect("invoke: inner run failed");
                }
                if let Some(inner_start) = inner_start {
                    phases.inner_guest += inner_start.elapsed().as_nanos();
                    phases.inner_guest_n += 1;
                }

                raw[0..8].copy_from_slice(&inner.gas().to_le_bytes());
                for (i, reg) in Reg::ALL.into_iter().enumerate() {
                    let offset = 8 + i * 8;
                    raw[offset..offset + 8].copy_from_slice(&inner.reg(reg).to_le_bytes());
                }
                outer.write_memory(args_ptr, &raw).expect("invoke: bad args write");

                let (outcome, detail) = match interrupt {
                    InterruptKind::Finished => (0, 0),
                    InterruptKind::Trap => (1, 0),
                    InterruptKind::Segfault(segfault) => (2, u64::from(segfault.page_address)),
                    InterruptKind::Ecalli(index) => (3, u64::from(index)),
                    InterruptKind::NotEnoughGas => (4, 0),
                    InterruptKind::Step => unreachable!("step tracing is not enabled"),
                };
                outer.set_reg(Reg::A0, outcome);
                outer.set_reg(Reg::A1, detail);

                if let Some(start) = start {
                    phases.invoke += start.elapsed().as_nanos();
                    phases.invoke_n += 1;
                }
            }
        }
    }

    /// The payload provably crossed and came back: the guest's end-to-end fold
    /// has to match a host-side computation of the same fold.
    fn verify(&mut self, args: &Args, variant: Variant) {
        if variant.compute {
            let result = self.compute_result();
            assert_ne!(result, 0, "the compute loop did not run");
            return;
        }

        let header = self.inner_header;
        let observed = self.inner.read_u64(header + caller::RESULT).expect("bad header");
        let expected = expected_fold(args.payload, args.calls_per_run);
        assert_eq!(
            observed, expected,
            "fold mismatch: the payload did not survive the round trip (payload {} B, K {}, {:?})",
            args.payload, args.calls_per_run, args.peeks
        );

        if variant.mediated {
            let outer_header = self.outer_header;
            let outer = self.outer.as_mut().expect("no mediator");
            let mediated = outer.read_u64(outer_header + mediator::MEDIATED).expect("bad header");
            let outcome = outer.read_u64(outer_header + mediator::OUTCOME).expect("bad header");
            assert_eq!(mediated, args.calls_per_run, "the mediator served the wrong number of calls");
            assert_eq!(outcome, 0, "the inner VM did not halt cleanly (outcome {outcome})");
        }
    }

    fn compute_result(&mut self) -> u64 {
        let header = self.inner_header;
        self.inner.read_u64(header + caller::COMPUTE_RESULT).expect("bad header")
    }
}

/// The node-side "crypto": read the payload out of guest memory, fold it into a
/// `u64` and write the result back. The fold touches every byte, so the copy
/// cost is real, and the guest consumes the result, so nothing can be elided.
fn host_stub(instance: &mut RawInstance, ptr: u32, len: u32, dst: u32, result_bytes: u64, buffer: &mut Vec<u8>) {
    let data = instance.read_memory(ptr, len).expect("stub: bad read");
    let fold = xor_fold(&data);

    buffer.clear();
    buffer.extend_from_slice(&fold.to_le_bytes());
    let mut filler = fold;
    while buffer.len() < result_bytes as usize {
        filler = filler.rotate_left(11) ^ 0x5851_f42d_4c95_7f2d;
        buffer.extend_from_slice(&filler.to_le_bytes());
    }
    buffer.truncate(result_bytes as usize);
    instance.write_memory(dst, buffer).expect("stub: bad write");
}

fn xor_fold(data: &[u8]) -> u64 {
    let mut acc = 0u64;
    let mut chunks = data.chunks_exact(8);
    for chunk in &mut chunks {
        acc ^= u64::from_le_bytes(chunk.try_into().unwrap());
        acc = acc.rotate_left(7);
    }
    for &byte in chunks.remainder() {
        acc = acc.rotate_left(1) ^ u64::from(byte);
    }
    acc
}

/// Independently computes what the guest's fold must come out to, from the
/// payload pattern `bench-nested-caller::initialize` writes.
fn expected_fold(payload_bytes: u64, calls: u64) -> u64 {
    let len = payload_bytes as usize;
    let mut payload: Vec<u8> = (0..len)
        .map(|i| ((i as u64).wrapping_mul(31).wrapping_add(7) ^ (i as u64 >> 3)) as u8)
        .collect();

    let mut fold = FOLD_SEED;
    for _ in 0..calls {
        payload[0..8].copy_from_slice(&fold.to_le_bytes());
        fold = fold.rotate_left(1) ^ xor_fold(&payload);
    }
    fold
}

fn prepare_call(instance: &mut RawInstance, pc: ProgramCounter, sp: u64) {
    instance.set_gas(GAS_LIMIT);
    instance.set_reg(Reg::RA, polkavm::RETURN_TO_HOST);
    instance.set_reg(Reg::SP, sp);
    instance.set_next_program_counter(pc);
}

fn run_to_completion(instance: &mut RawInstance, dynamic_paging: bool) {
    loop {
        match instance.run().expect("run failed") {
            InterruptKind::Finished => break,
            InterruptKind::Segfault(segfault) if dynamic_paging => allocate_page(instance, segfault),
            other => panic!("unexpected interrupt: {other:?}"),
        }
    }
}

fn allocate_page(instance: &mut RawInstance, segfault: polkavm::Segfault) {
    instance
        .zero_memory_with_memory_protection(segfault.page_address, segfault.page_size, MemoryProtection::ReadWrite)
        .expect("failed to allocate a page for the inner VM");
}
