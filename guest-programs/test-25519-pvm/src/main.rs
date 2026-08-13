//! Correctness guest for the 256-bit wide-arithmetic backend.
//!
//! Every export reads its operands from `INPUT` and writes its result to
//! `OUTPUT`; the host discovers both addresses once via `input_ptr()` /
//! `output_ptr()`. Nothing here panics on a *failure* result — the signature
//! entry points return 0/1 so the host can assert that bad signatures are
//! **rejected**, which is the coverage a `.unwrap()`-based benchmark cannot
//! provide.

#![no_std]
#![no_main]

extern crate alloc;

use curve25519_dalek::edwards::CompressedEdwardsY;
use curve25519_dalek::edwards::EdwardsPoint;
use curve25519_dalek::scalar::Scalar;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::arch::asm!("unimp", options(noreturn)) }
}

// Verification allocates; 64 KiB is generous (the benches run both ed25519 and
// sr25519 verification in 16 KiB) and this program is never timed.
const HEAP_SIZE: usize = 64 * 1024;

#[global_allocator]
static mut GLOBAL_ALLOC: picoalloc::Mutex<picoalloc::Allocator<picoalloc::ArrayPointer<HEAP_SIZE>>> = {
    static mut ARRAY: picoalloc::Array<HEAP_SIZE> = picoalloc::Array([0; HEAP_SIZE]);
    picoalloc::Mutex::new(picoalloc::Allocator::new(unsafe {
        picoalloc::ArrayPointer::new(&raw mut ARRAY)
    }))
};

const INPUT_LEN: usize = 1024;
const OUTPUT_LEN: usize = 64;

static mut INPUT: [u8; INPUT_LEN] = [0; INPUT_LEN];
static mut OUTPUT: [u8; OUTPUT_LEN] = [0; OUTPUT_LEN];

#[polkavm_derive::polkavm_export]
extern "C" fn input_ptr() -> u64 {
    core::ptr::addr_of!(INPUT) as u64
}

#[polkavm_derive::polkavm_export]
extern "C" fn output_ptr() -> u64 {
    core::ptr::addr_of!(OUTPUT) as u64
}

fn input() -> &'static [u8; INPUT_LEN] {
    unsafe { &*core::ptr::addr_of!(INPUT) }
}

fn take32(offset: usize) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&input()[offset..offset + 32]);
    out
}

fn take64(offset: usize) -> [u8; 64] {
    let mut out = [0u8; 64];
    out.copy_from_slice(&input()[offset..offset + 64]);
    out
}

fn write_output(bytes: &[u8]) {
    unsafe { (&mut (*core::ptr::addr_of_mut!(OUTPUT)))[..bytes.len()].copy_from_slice(bytes) }
}

/// Edwards point arithmetic.
///
/// Input layout: `A` at 0 (compressed), `B` at 32, scalar `s` at 64, scalar `t`
/// at 96. The result is written compressed to `OUTPUT[0..32]`.
///
/// Returns 0 on success, 1 if `A` failed to decompress, 2 if `B` did — so the
/// host can tell "this input is legitimately not a curve point" apart from
/// "the arithmetic is wrong". Not-on-curve inputs are a real case worth
/// exercising, so the host feeds them deliberately.
#[polkavm_derive::polkavm_export]
extern "C" fn point_op(op: u64) -> u64 {
    let Some(a) = CompressedEdwardsY(take32(0)).decompress() else {
        return 1;
    };
    let s = Scalar::from_bytes_mod_order(take32(64));

    let result: EdwardsPoint = match op {
        // Doubling and scalar multiplication only need `A`.
        1 => a + a,
        2 => a * s,
        3 => EdwardsPoint::mul_base(&s),
        4 => EdwardsPoint::mul_base_clamped(take32(64)),
        // Everything else needs `B` too.
        _ => {
            let Some(b) = CompressedEdwardsY(take32(32)).decompress() else {
                return 2;
            };
            let t = Scalar::from_bytes_mod_order(take32(96));
            match op {
                0 => a + b,
                5 => a - b,
                6 => a * s + b * t,
                // A long dependent chain: repeated doubling, which is where a
                // subtly wrong carry fold shows up as a diverging result
                // rather than an obviously broken one.
                7 => {
                    let mut acc = a;
                    for _ in 0..64 {
                        acc = acc + acc;
                    }
                    acc + b
                }
                _ => return 3,
            }
        }
    };

    write_output(result.compress().as_bytes());
    0
}

/// Scalar arithmetic. `a` at 0, `b` at 32; canonical bytes to `OUTPUT[0..32]`.
#[polkavm_derive::polkavm_export]
extern "C" fn scalar_op(op: u64) -> u64 {
    let a = Scalar::from_bytes_mod_order(take32(0));
    let b = Scalar::from_bytes_mod_order(take32(32));

    let result = match op {
        0 => a + b,
        1 => a - b,
        2 => a * b,
        3 => a * b.invert(),
        _ => return 1,
    };

    write_output(result.as_bytes());
    0
}

/// `ed25519-zebra` verification (ZIP-215 rules — what `sp_core::ed25519` uses).
///
/// Input: key at 0, signature at 32, message at 96 for `msg_len` bytes.
/// Returns 1 if the signature verifies, 0 otherwise. **Never panics on a
/// rejection** — the host asserts on this value.
#[polkavm_derive::polkavm_export]
extern "C" fn verify_ed25519_zebra(msg_len: u64) -> u32 {
    let Ok(key) = ed25519_zebra::VerificationKey::try_from(take32(0)) else {
        return 0;
    };
    let signature = ed25519_zebra::Signature::from(take64(32));
    let message = &input()[96..96 + msg_len as usize];
    u32::from(key.verify(&signature, message).is_ok())
}

/// `ed25519-dalek` verification (strict RFC 8032). Same layout as above; the
/// two implementations differ on edge cases, so the host holds each to its own
/// expectation.
#[polkavm_derive::polkavm_export]
extern "C" fn verify_ed25519_dalek(msg_len: u64) -> u32 {
    let Ok(key) = ed25519_dalek::VerifyingKey::from_bytes(&take32(0)) else {
        return 0;
    };
    let signature = ed25519_dalek::Signature::from_bytes(&take64(32));
    let message = &input()[96..96 + msg_len as usize];
    u32::from(ed25519_dalek::Verifier::verify(&key, message, &signature).is_ok())
}

/// schnorrkel (sr25519) verification. Same layout; the signing context is
/// passed in at 160 for `ctx_len` bytes.
#[polkavm_derive::polkavm_export]
extern "C" fn verify_sr25519(msg_len: u64, ctx_len: u64) -> u32 {
    let Ok(key) = schnorrkel::PublicKey::from_bytes(&take32(0)) else {
        return 0;
    };
    let Ok(signature) = schnorrkel::Signature::from_bytes(&take64(32)) else {
        return 0;
    };
    let message = &input()[96..96 + msg_len as usize];
    let context = &input()[160..160 + ctx_len as usize];
    u32::from(key.verify_simple(context, message, &signature).is_ok())
}
