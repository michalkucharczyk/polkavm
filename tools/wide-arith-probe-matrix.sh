#!/usr/bin/env bash
#
# Wide-arithmetic source-probe experiment: end-to-end A/B runner.
#
# Self-contained on a fresh machine: builds the redc guest blobs in place,
# builds opcount, sanity-checks the gas, and runs the interleaved timing
# matrix. Assumes only this polkavm checkout (the script lives in it), the
# curve25519-dalek fork checked out as its sibling (the guest programs patch
# curve25519-dalek to ../../curve25519-dalek), `rustup`/`cargo` and `taskset`.
#
# The primary question this answers is MODE B: what the wide-arithmetic
# *source* read-probes actually cost, with nothing paid in return. That number
# is the entire basis for deciding whether the remaining probe-removal design
# (no register saves at all; a faulting wide op zeroes its whole destroyed set
# including operands, so a source fault stops being resumable) is worth a spec
# amendment. The vmctx-save design was already measured and rejected
# in-container - see the negative-result section of wide-arith-results.md.
#
#   ./tools/wide-arith-probe-matrix.sh                 # all modes, Linux sandbox
#   ./tools/wide-arith-probe-matrix.sh --generic       # all modes, generic sandbox
#   ./tools/wide-arith-probe-matrix.sh --reuse-blobs   # skip the guest rebuild
#   ./tools/wide-arith-probe-matrix.sh --quick         # only A and B (see below)
#
# It measures every mode by default. The modes share one binary and one blob
# set, so the full matrix costs only wall time - and leaving a mode out is how
# a verdict ends up resting on one machine. --quick exists for re-checking the
# probe cost alone, not for deciding anything about the saves.
#
# Env: ROUNDS (default 8), CPU (default 2), NATIVE_TOOLCHAIN (see below).
#
# MEASUREMENT ONLY. Mode B knowingly breaks the fault contract: a source load
# can fault mid-body where the compiled code has already clobbered guest
# registers and the host has no fixup for them. Never run the test suite, the
# tracing crosscheck, or anything with dynamic paging against a mode-B build.

set -euo pipefail

REPO="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
GUEST="$REPO/guest-programs"
# build-benchmarks.sh links the blobs here, which is also where benchtool
# reads them from, so this doubles as the setup for a run-crypto-benches run.
BLOBS="$GUEST/target/riscv64emac-unknown-none-polkavm/release"
WORKDIR="$REPO/target/wide-arith-probe-matrix"
ROUNDS="${ROUNDS:-8}"
CPU="${CPU:-2}"

# The guest-side dalek fork must sit next to this checkout: guest-programs
# patches curve25519-dalek to ../../curve25519-dalek/curve25519-dalek. Assumed
# already checked out on the right branch - this script never touches it.
DALEK_DIR="$(dirname -- "$REPO")/curve25519-dalek"

# Gas per verify of the redc zebra blob (contract configuration). Checked
# before any timing: the classic stale-blob tell is numbers coming back at a
# previous configuration's value.
EXPECTED_GAS=109685

SANDBOX_FLAG="--linux"
SANDBOX_NAME="linux"
CARGO_FEATURES=()
FULL=1
REUSE_BLOBS=0

while [ $# -gt 0 ]; do
    case "$1" in
        --quick)   FULL=0;;
        --full)    FULL=1;;  # kept for compatibility; this is the default now
        --generic) SANDBOX_FLAG=""; SANDBOX_NAME="generic"; CARGO_FEATURES=(--features polkavm/generic-sandbox);;
        --reuse-blobs) REUSE_BLOBS=1;;
        -h|--help) sed -n '2,30p' -- "${BASH_SOURCE[0]}" | sed 's/^# \?//'; exit 0;;
        *) echo "unknown argument: $1" >&2; exit 1;;
    esac
    shift
done

step() { printf '\n=== %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

step "Preflight"
for tool in cargo rustup git taskset; do
    command -v "$tool" >/dev/null || die "$tool not found in PATH"
done

# The knobs live in the recompiler; without them every mode would silently
# measure the same thing.
grep -q "POLKAVM_WIDE_ARITH_EXPERIMENT_NO_SRC_PROBES" "$REPO/crates/polkavm/src/compiler/amd64.rs" \
    || die "this checkout has no experiment knobs - it must include the measurement commit (branch mku-wide-arith-probe-experiment)"

[ -d "$DALEK_DIR/curve25519-dalek" ] \
    || die "the curve25519-dalek fork must be checked out at $DALEK_DIR (guest-programs patches curve25519-dalek to that path)"

echo "polkavm: $REPO @ $(git -C "$REPO" rev-parse --short HEAD) ($(git -C "$REPO" rev-parse --abbrev-ref HEAD))"
echo "dalek:   $DALEK_DIR @ $(git -C "$DALEK_DIR" rev-parse --short HEAD) ($(git -C "$DALEK_DIR" rev-parse --abbrev-ref HEAD))"
echo "sandbox: $SANDBOX_NAME, $ROUNDS rounds, pinned to CPU $CPU"

step "Guest blobs (WIDE_ARITH=redc)"
if [ "$REUSE_BLOBS" = 1 ] && [ -f "$BLOBS/bench-ed25519-zebra.polkavm" ]; then
    echo "reusing $BLOBS"
else
    # On x86_64 Linux build-benchmarks.sh always builds the native host
    # libraries too, by default with a pinned nightly. Those are irrelevant
    # here (nothing below measures host baselines) and a missing nightly would
    # abort the whole build on a fresh machine, so pin them to the repo's
    # stable toolchain unless the caller says otherwise. The guest builds use
    # the nightly pinned by guest-programs/rust-toolchain.toml either way.
    (cd -- "$GUEST" && WIDE_ARITH=redc NATIVE_TOOLCHAIN="${NATIVE_TOOLCHAIN:-+1.86.0}" ./build-benchmarks.sh)
fi

step "Building opcount ($SANDBOX_NAME sandbox)"
(cd -- "$REPO" && cargo build --release -p opcount "${CARGO_FEATURES[@]}")
OPCOUNT="$REPO/target/release/opcount"

step "Gas sanity (stale-blob check)"
gas=$("$OPCOUNT" "$BLOBS/bench-ed25519-zebra.polkavm" | sed -n 's/^=== run: [0-9]* instructions, \([0-9]*\) gas ===$/\1/p')
[ "$gas" = "$EXPECTED_GAS" ] || die "zebra blob reports $gas gas, expected $EXPECTED_GAS - not the redc configuration, or the wrong commit"
echo "zebra redc: $gas gas ✓"

step "Timing matrix"
P=POLKAVM_WIDE_ARITH_EXPERIMENT_NO_SRC_PROBES
V=POLKAVM_WIDE_ARITH_EXPERIMENT_VMCTX_SAVES

# Mode list: name plus the env assignments that define it. A is the untouched
# baseline, B the prize (probes gone, nothing paid), C the price of making a
# mid-body fault recoverable via vmctx, D = B + C the shippable design's hot
# path. C and D are what tell you whether the design pays *on this machine*,
# so they are measured unless --quick says otherwise.
MODES=("A:WIDE_ARITH_EXPERIMENT_UNSET=1" "B_all:$P=all")
if [ "$FULL" = 1 ]; then
    MODES+=("C_all:$V=all" "D_all:$P=all $V=all")
    for family in fused redc addsub; do
        MODES+=("D_$family:$P=$family $V=$family")
    done
fi

# zebra/ed25519/sr25519 carry the wide-arithmetic work; ecdsa-k256 contains
# none, so every mode executes identical machine code there and its spread is
# the measurement noise floor.
BENCHES="bench-ed25519-zebra bench-ed25519 bench-sr25519 bench-ecdsa-k256"

mkdir -p "$WORKDIR"
RAW="$WORKDIR/raw-$SANDBOX_NAME.txt"
: > "$RAW"
for round in $(seq "$ROUNDS"); do
    for bench in $BENCHES; do
        for entry in "${MODES[@]}"; do
            name="${entry%%:*}"
            # shellcheck disable=SC2086 # both must word-split
            ns=$(env ${entry#*:} taskset -c "$CPU" "$OPCOUNT" --time $SANDBOX_FLAG \
                    "$BLOBS/$bench.polkavm" | sed -n 's/.*best \([0-9]*\) ns.*/\1/p')
            [ -n "$ns" ] || die "no timing from opcount for $bench/$name"
            echo "$bench $name $ns" >> "$RAW"
        done
    done
    echo "round $round/$ROUNDS done" >&2
done

step "Results ($SANDBOX_NAME sandbox, best-of-batch / mean, us per verify)"
# The control bench (no wide-arithmetic instructions) runs byte-identical
# machine code in every mode, so the spread of its per-mode results IS this
# machine's error bar. Deltas smaller than it are not signals, and the run is
# worthless if it is larger than the effects being measured - so it is computed
# first and every delta is checked against it.
awk -v control="bench-ecdsa-k256" '
    { key = $1 " " $2
      if (!(key in best) || $3 < best[key]) best[key] = $3
      sum[key] += $3; n[key]++
      if (!($1 in seen_bench)) { benches[++nb] = $1; seen_bench[$1] = 1 }
      if (!($2 in seen_mode))  { modes[++nm] = $2;   seen_mode[$2] = 1 }
    }
    END {
        floor_us = 0
        if (control " A" in best) {
            lo = hi = best[control " A"]
            for (m = 1; m <= nm; m++) {
                key = control " " modes[m]
                if (!(key in best)) continue
                if (best[key] < lo) lo = best[key]
                if (best[key] > hi) hi = best[key]
            }
            floor_us = (hi - lo) / 1000
            printf "\nnoise floor from %s (identical code in every mode): +-%.2f us\n", control, floor_us
        } else {
            printf "\nWARNING: control bench %s missing - no error bar for this run\n", control
        }

        for (b = 1; b <= nb; b++) {
            bench = benches[b]
            printf "\n%s\n", bench
            base = best[bench " A"]
            for (m = 1; m <= nm; m++) {
                mode = modes[m]; key = bench " " mode
                if (!(key in best)) continue
                printf "  %-9s best %8.2f  mean %8.2f", mode, best[key]/1000, sum[key]/n[key]/1000
                if (mode != "A" && base > 0) {
                    d = (best[key] - base) / 1000
                    printf "  delta %+6.2f", d
                    if (floor_us > 0 && (d < 0 ? -d : d) < floor_us) printf "  (within noise)"
                }
                printf "\n"
            }
        }

        if (floor_us > 1.0)
            printf "\nWARNING: the error bar (%.2f us) is larger than the effects being measured\n         (expect 2-4 us). This run cannot decide anything - quiesce the\n         machine (performance governor, idle SMT sibling, ideally isolcpus),\n         raise ROUNDS, and try --generic to remove the sandbox round-trip.\n", floor_us
    }
' "$RAW"

cat <<EOF

Raw samples: $RAW
How to read it:
  * bench-ecdsa-k256 has no wide-arithmetic instructions, so every mode runs
    identical machine code there - its spread IS the error bar. In-container on
    Zen 3 that was +-0.7 us on a 57-60 us bench; anything smaller than the k256
    spread is not a signal.
  * B_all is the prize: the source read-probes' true cost. Zen 3 container
    -2.53 / -3.83 / -3.28 us; machine B (generic sandbox) -3.06 / -3.40 /
    -3.33 us (zebra / ed25519 / sr25519).
  * C_all is what making a mid-body fault recoverable costs: +3.58 / +3.64 /
    +2.84 us in-container. D_all is the net, +1.49 / +2.38 / +0.22 there - a
    regression in the container. Whether that holds on other machines is
    exactly what the C and D rows are for: the container is known to
    over-weight save/restore memory traffic, which is all mode C adds.
  * The blobs are now in the directory benchtool reads, so the same numbers can
    be reproduced in the format of record with:
      POLKAVM_WIDE_ARITH_EXPERIMENT_NO_SRC_PROBES=all ./run-crypto-benches
    after rebuilding benchtool at this commit.
EOF
