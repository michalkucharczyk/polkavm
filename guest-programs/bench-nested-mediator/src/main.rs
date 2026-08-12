//! The "parachain service" role of the nested host-call overhead harness.
//!
//! Holds an inner PVM and mediates every host call it makes, exactly the way a
//! JAM service has to: the inner VM cannot reach the node, so each of its host
//! calls comes back out of `invoke`, gets its arguments `peek`ed into service
//! memory, is performed by the service against the node, and its result is
//! `poke`d back before the inner VM is resumed.
//!
//! The host-call signatures mirror `jam-pvm-common`'s (`machine`, `peek`,
//! `poke`, `invoke`); the semantics are re-implemented host-side rather than
//! imported, per the hand-off's fidelity rule.
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

/// The largest result the node stub produces.
const MAX_RESULT: usize = 32;

/// `invoke`'s argument block: gas followed by the 13 registers, 112 bytes.
/// Register `i` is `Reg::ALL[i]`, i.e. `[RA, SP, T0, T1, T2, S0, S1, A0, A1,
/// A2, A3, A4, A5]`.
const ARGS_LEN: usize = 14;
const REG_RA: usize = 1 + 0;
const REG_SP: usize = 1 + 1;
const REG_A0: usize = 1 + 7;
const REG_A1: usize = 1 + 8;
const REG_A2: usize = 1 + 9;

/// `invoke` outcome code, as in the Gray Paper / `jam-types`: `HOST`. The
/// others (`HALT` = 0, `PANIC` = 1, `FAULT` = 2, `OOG` = 4) all end the run and
/// are reported back to the host verbatim.
const OUTCOME_HOST_CALL_FAULT: u64 = 3;

/// Peek strategies.
const PEEKS_PACKED: u64 = 0;

/// The control block. `initialize` returns its address; the host fills in the
/// knobs before the first `run()`.
#[repr(C)]
struct Header {
    /// The inner VM's handle (guest writes).
    handle: u64,
    /// Gas handed to the inner VM on every `invoke` (host writes).
    gas: u64,
    /// The inner VM's initial return address, i.e. `RETURN_TO_HOST`
    /// (host writes).
    inner_ra: u64,
    /// The inner VM's initial stack pointer (host writes).
    inner_sp: u64,
    /// Result bytes to poke back per mediated call (host writes).
    result_bytes: u64,
    /// `PEEKS_PACKED` (one peek) or anything else (three peeks) (host writes).
    peek_mode: u64,
    /// Number of mediated host calls in the last `run()` (guest writes).
    mediated: u64,
    /// The outcome that ended the last `run()` (guest writes).
    outcome: u64,
}

static mut HEADER: Header = Header {
    handle: 0,
    gas: 0,
    inner_ra: 0,
    inner_sp: 0,
    result_bytes: 8,
    peek_mode: PEEKS_PACKED,
    mediated: 0,
    outcome: 0,
};

/// Where the inner VM's arguments are peeked to.
static mut BUFFER: [u8; MAX_PAYLOAD] = [0; MAX_PAYLOAD];

/// Where the node stub writes its result, and the source of the poke back.
static mut RESULT: [u8; MAX_RESULT] = [0; MAX_RESULT];

static mut ARGS: [u64; ARGS_LEN] = [0; ARGS_LEN];

#[polkavm_derive::polkavm_import]
extern "C" {
    fn machine(code_ptr: *const u8, code_len: u64, program_counter: u64) -> u64;
    fn peek(vm_handle: u64, outer_dst: *mut u8, inner_src: u64, length: u64) -> u64;
    fn poke(vm_handle: u64, outer_src: *const u8, inner_dst: u64, length: u64) -> u64;
    fn stub(ptr: u64, len: u64, result_ptr: u64);
}

// The tuple return is how `invoke` reports (outcome, detail); for non-PolkaVM
// targets the macro passes the `extern "C"` block through untouched and Rust
// would complain about the tuple, exactly as in `jam-pvm-common`.
#[cfg_attr(not(target_env = "polkavm"), allow(improper_ctypes))]
#[polkavm_derive::polkavm_import]
extern "C" {
    fn invoke(vm_handle: u64, args: *mut u64) -> (u64, u64);
}

/// Creates the inner VM once, outside the measured loop, and publishes the
/// header.
///
/// The code pointer is a formality: the harness's `machine` instantiates the
/// caller blob it was given on the command line. Spawning is not part of what
/// is being measured, so nothing is lost.
#[polkavm_derive::polkavm_export]
extern "C" fn initialize() -> u64 {
    unsafe {
        HEADER.handle = machine(core::ptr::null(), 0, 0);
        core::ptr::addr_of!(HEADER) as usize as u64
    }
}

/// Runs the inner VM to completion, mediating every host call it makes.
#[polkavm_derive::polkavm_export]
extern "C" fn run() {
    unsafe {
        let handle = HEADER.handle;
        let packed = HEADER.peek_mode == PEEKS_PACKED;
        let result_bytes = HEADER.result_bytes;

        // The registers round-trip through the argument block: whatever the
        // inner VM had when it faulted is what it gets back on resume.
        let mut i = 0;
        while i < ARGS_LEN {
            ARGS[i] = 0;
            i += 1;
        }
        ARGS[REG_RA] = HEADER.inner_ra;
        ARGS[REG_SP] = HEADER.inner_sp;

        let mut mediated = 0;
        loop {
            ARGS[0] = HEADER.gas;
            let (outcome, _detail) = invoke(handle, ARGS.as_mut_ptr());
            if outcome != OUTCOME_HOST_CALL_FAULT {
                HEADER.outcome = outcome;
                break;
            }

            // The inner VM's `stub(ptr, len, result_ptr)` arguments.
            let ptr = ARGS[REG_A0];
            let len = ARGS[REG_A1];
            let inner_dst = ARGS[REG_A2];

            if packed {
                peek(handle, BUFFER.as_mut_ptr(), ptr, len);
            } else {
                // What an undisciplined service does: one peek per argument.
                // The split is the ed25519 shape at 128 B (64 + 32 + 32).
                let first = len / 2;
                let second = len / 4;
                let third = len - first - second;
                peek(handle, BUFFER.as_mut_ptr(), ptr, first);
                peek(handle, BUFFER.as_mut_ptr().add(first as usize), ptr + first, second);
                peek(
                    handle,
                    BUFFER.as_mut_ptr().add((first + second) as usize),
                    ptr + first + second,
                    third,
                );
            }

            stub(BUFFER.as_ptr() as usize as u64, len, RESULT.as_ptr() as usize as u64);
            poke(handle, RESULT.as_ptr(), inner_dst, result_bytes);

            mediated += 1;
        }

        HEADER.mediated = mediated;
    }
}
