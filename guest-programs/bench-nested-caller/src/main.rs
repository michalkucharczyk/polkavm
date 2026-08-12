//! The "parachain code" role of the nested host-call overhead harness.
//!
//! Makes `K` stub host calls per `run()`, each handing over an `N`-byte
//! argument buffer and consuming an `R`-byte result the host writes back. The
//! result is folded into a running value the host verifies afterwards, so the
//! payload provably crossed both ways and nothing can be elided.
//!
//! This blob is used both as a flat guest (`one-jump` mode, its stub host call
//! served directly by the node) and as the inner VM under
//! `bench-nested-mediator` (`two-jump` mode). It doesn't know which.
//!
//! See designs/parachain-service-on-jam/nested-call-overhead-handoff.md.

#![no_std]
#![no_main]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    unsafe {
        core::arch::asm!("unimp", options(noreturn));
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    unsafe {
        core::arch::asm!("ud2", options(noreturn));
    }
}

/// The largest payload the sweep uses (64 KiB).
const MAX_PAYLOAD: usize = 65536;

/// The largest result the host writes back.
const MAX_RESULT: usize = 32;

/// The fold's starting value; also what gets stamped into the payload at the
/// top of every call, so a `run()` is deterministic no matter how often it is
/// repeated.
const FOLD_SEED: u64 = 0x9e37_79b9_7f4a_7c15;

/// The control block. `initialize` returns its address; the host fills in the
/// knobs and reads `result` back out.
#[repr(C)]
struct Header {
    /// Address of the payload buffer (guest writes).
    payload_ptr: u64,
    /// Address of the result buffer the host pokes into (guest writes).
    scratch_ptr: u64,
    /// `K`: stub calls per `run()` (host writes).
    calls_per_run: u64,
    /// `N`: payload bytes per stub call (host writes).
    payload_bytes: u64,
    /// The end-to-end fold (guest writes).
    result: u64,
    /// Iteration count for the pure-compute export (host writes).
    compute_iterations: u64,
    /// The compute export's output (guest writes).
    compute_result: u64,
}

static mut HEADER: Header = Header {
    payload_ptr: 0,
    scratch_ptr: 0,
    calls_per_run: 1,
    payload_bytes: 128,
    result: 0,
    compute_iterations: 1024,
    compute_result: 0,
};

static mut PAYLOAD: [u8; MAX_PAYLOAD] = [0; MAX_PAYLOAD];
static mut SCRATCH: [u8; MAX_RESULT] = [0; MAX_RESULT];

#[polkavm_derive::polkavm_import]
extern "C" {
    /// The stub: fold `len` bytes at `ptr` into a `u64` and write it to
    /// `result_ptr`. Served by the node directly (`one-jump`) or mediated by
    /// the outer VM (`two-jump`).
    fn stub(ptr: u64, len: u64, result_ptr: u64);
}

/// Fills the payload with a deterministic pattern and publishes the header.
///
/// Returns the header's address so the host can drive the knobs without
/// knowing the blob's memory layout.
#[polkavm_derive::polkavm_export]
extern "C" fn initialize() -> u64 {
    unsafe {
        let mut i = 0;
        while i < MAX_PAYLOAD {
            PAYLOAD[i] = ((i as u64).wrapping_mul(31).wrapping_add(7) ^ (i as u64 >> 3)) as u8;
            i += 1;
        }

        HEADER.payload_ptr = PAYLOAD.as_ptr() as usize as u64;
        HEADER.scratch_ptr = SCRATCH.as_ptr() as usize as u64;
        core::ptr::addr_of!(HEADER) as usize as u64
    }
}

/// `K` stub calls, folding each result back in.
///
/// The fold is stamped into the head of the payload before every call, so the
/// bytes the host sees differ per iteration (nothing can be hoisted) while the
/// whole run stays reproducible.
#[polkavm_derive::polkavm_export]
extern "C" fn run() {
    unsafe {
        let calls = HEADER.calls_per_run;
        let len = HEADER.payload_bytes;
        let payload = HEADER.payload_ptr;
        let scratch = HEADER.scratch_ptr;

        let mut fold = FOLD_SEED;
        let mut i = 0;
        while i < calls {
            core::ptr::write_volatile(payload as usize as *mut u64, fold);
            stub(payload, len, scratch);
            let returned = core::ptr::read_volatile(scratch as usize as *const u64);
            fold = fold.rotate_left(1) ^ returned;
            i += 1;
        }

        HEADER.result = fold;
    }
}

/// A fixed pure-compute loop: no host calls, no memory traffic beyond the
/// header. Used by `nesting-tax` mode, which runs it flat and then as an inner
/// VM and expects the two to cost the same.
#[polkavm_derive::polkavm_export]
extern "C" fn compute() {
    unsafe {
        let iterations = HEADER.compute_iterations;
        let mut x = 0x243f_6a88_85a3_08d3_u64;
        let mut i = 0;
        while i < iterations {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            x ^= x >> 29;
            i += 1;
        }

        HEADER.compute_result = x;
    }
}
