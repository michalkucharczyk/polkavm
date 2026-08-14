#!/bin/sh

# Turns `run-ec-benches` output into markdown summary tables.
#
# Usage:
#   ./run-ec-benches > ec.txt
#   ./process-ec-results.sh ec.txt > ec-table.md
#
# Neither sibling script fits: process-crypto-results.sh keys rows by benchmark
# name alone, so the sized MSM rows would collapse into one, and
# process-hash-results.sh divides by the hash benchmarks' amortization loop,
# which these benchmarks do not have (one run() is one operation).
#
# Two row shapes are handled:
#   runtime/<bench>/<backend>: ... : 1.23ms          - pairing and mul
#   runtime/<bench>/<backend>/<count>: ... : 1.23ms  - MSM, <count> = bases
#
# Roles per benchmark (same naming as the report / process-crypto-results.sh):
#   host portable = <bench>        with the native backend (plain build)
#   host native   = <bench>-native with the native backend
#                   (-C target-cpu=native, built by build-ec-native.sh;
#                   column shows "-" when those libraries are absent)
#   pvm           = <bench>        with the polkavm64_compiler_sync_gas backend
#
# All three are single-threaded. The production RFC-163 host functions are not:
# sp-crypto-ec-utils's `std` feature enables `ark-ec/parallel`, so a validator
# can parallelize MSM across cores on top of the per-core numbers below.

set -eu

awk '
function to_ns(time) {
    ns = time
    if (time ~ /ns$/)      { sub(/ns$/, "", ns); return ns + 0 }
    else if (time ~ /us$/) { sub(/us$/, "", ns); return ns * 1000 }
    else if (time ~ /ms$/) { sub(/ms$/, "", ns); return ns * 1000000 }
    else if (time ~ /s$/)  { sub(/s$/,  "", ns); return ns * 1000000000 }
    return ""
}

function fmt(ns) {
    if (ns == "") return "-"
    if (ns < 1000) return sprintf("%.0f ns", ns)
    if (ns < 1000000) return sprintf("%.2f µs", ns / 1000)
    return sprintf("%.2f ms", ns / 1000000)
}

function ratio(a, b) {
    if (a == "" || b == "" || b == 0) return "-"
    return sprintf("%.2fx", a / b)
}

function per_point(ns, count) {
    return (ns == "") ? "-" : fmt(ns / count)
}

$1 ~ /^runtime\// {
    key = $1; sub(/:$/, "", key)
    segments = split(key, path, "/")
    if (segments < 3) next

    bench = path[2]
    backend = path[3]
    count = (segments >= 4) ? path[4] : "-"

    ns = to_ns($NF)
    if (ns == "") next

    tuned_artifact = sub(/-native$/, "", bench)

    if (backend == "native") {
        if (tuned_artifact) hostnative[bench, count] = ns
        else                portable[bench, count] = ns
    } else if (!tuned_artifact && backend == "polkavm64_compiler_sync_gas") {
        pvm[bench, count] = ns
    } else {
        next
    }

    if (!((bench, count) in seen)) {
        seen[bench, count] = 1
        if (count == "-") {
            unsized[++unsized_rows] = bench
        } else {
            sized_bench[++sized_rows] = bench
            sized_count[sized_rows] = count
        }
    }
}

END {
    print "*pvm* = PVM guest blob (recompiler, sync gas) · *host native* = host"
    print "build with `-C target-cpu=native` (build machine only) · *host portable*"
    print "= host build that runs on any x86-64. All single-threaded."
    print ""

    if (unsized_rows) {
        print "## Per-operation"
        print ""
        print "| benchmark | host portable | host native | PVM (64-bit, sync gas) | pvm/portable | pvm/native |"
        print "|---|---:|---:|---:|---:|---:|"
        for (i = 1; i <= unsized_rows; i++) {
            b = unsized[i]
            printf "| %s | %s | %s | %s | %s | %s |\n", b, \
                fmt(portable[b, "-"]), fmt(hostnative[b, "-"]), fmt(pvm[b, "-"]), \
                ratio(pvm[b, "-"], portable[b, "-"]), ratio(pvm[b, "-"], hostnative[b, "-"])
        }
        printf "\n"
    }

    if (sized_rows) {
        print "## Multi-scalar multiplication"
        print ""
        print "Per-point columns are the number that matters for Pippenger scaling:"
        print "they must fall as the number of bases rises."
        print ""
        print "| benchmark | bases | host portable | host native | PVM (64-bit, sync gas) | pvm/portable | pvm/native | PVM per point | portable per point |"
        print "|---|---:|---:|---:|---:|---:|---:|---:|---:|"
        for (i = 1; i <= sized_rows; i++) {
            b = sized_bench[i]
            n = sized_count[i]
            printf "| %s | %s | %s | %s | %s | %s | %s | %s | %s |\n", b, n, \
                fmt(portable[b, n]), fmt(hostnative[b, n]), fmt(pvm[b, n]), \
                ratio(pvm[b, n], portable[b, n]), ratio(pvm[b, n], hostnative[b, n]), \
                per_point(pvm[b, n], n), per_point(portable[b, n], n)
        }
        printf "\n"
    }
}
' "$@"
