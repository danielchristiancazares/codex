# Performance migration

## Current baseline — upstream/main migration, 2026-09-07

The retained working state starts at upstream
`b4373e53ab79df7baadc6805dea54a060b820307`. Fork behavior comes from
`a925b5b3c3843a08c85e38a47de8dd7f21d8fbdb`. Measurements below compare the
upstream implementation with the migrated implementation, using the same fixtures
and Rust **1.95.0** on arm64 macOS in release mode with LTO disabled on both sides.

Each process discards five warmup samples and reports the median of 31 samples.
Five comparison rounds alternate which implementation runs first. The runs were
made without competing compilation or test processes.

| Production operation | Upstream median, ns/event | Migrated median, ns/event | Reduction |
| --- | ---: | ---: | ---: |
| Agent-message delta to app-server notification | 79.150 | 40.884 | 48.3% |
| WebSocket telemetry with metrics disabled | 299.842 | 1.487 | 99.5% |
| Response recording with a disabled tracing span | 17.641 | 1.317 | 92.5% |

Process medians, in round order:

| Operation | Upstream | Migrated |
| --- | --- | --- |
| Delta mapping | 78.680, 79.150, 79.210, 79.603, 78.864 | 40.756, 41.009, 40.884, 40.703, 40.893 |
| Disabled WebSocket telemetry | 299.842, 299.620, 299.937, 301.519, 299.691 | 2.165, 1.438, 1.487, 1.487, 1.487 |
| Disabled response span | 17.733, 17.566, 17.593, 19.702, 17.641 | 1.927, 1.317, 1.317, 1.317, 1.317 |

These are CPU costs at specific production boundaries. They do not measure model
latency, network latency, widget computation, ANSI serialization, or a terminal's
end-to-end rendering cost. No new TUI throughput claim follows from these results.
The migrated rendering changes retain their prior fork rationale and are checked
for correctness with snapshots and terminal integration tests.

### Fixtures and reproduction

The checked-in examples call production code:

- `codex-rs/app-server-protocol/examples/migration_latency.rs` maps batches of
  50,000 prebuilt agent-message deltas with matching thread/turn identifiers.
  It checks the serialized notification before measuring. Event construction and
  notification destruction are outside the timing interval.
- `codex-rs/otel/examples/migration_telemetry.rs` makes 100,000 calls per sample
  using a representative JSON text-delta WebSocket message, no global telemetry
  provider, and `tracing::Span::none()`.

Copy those identical example files into a clean worktree at the upstream base
before building the baseline. Run from each worktree's `codex-rs` directory:

```sh
CARGO_PROFILE_RELEASE_LTO=false cargo +1.95.0 build --release \
  -p codex-app-server-protocol --example migration_latency \
  -p codex-otel --example migration_telemetry
```

Save each implementation's binaries before switching source trees. The measured
executables for this migration were `/tmp/upstream-migration_latency`,
`/tmp/final-migration_latency`, `/tmp/upstream-migration_telemetry`, and
`/tmp/final-migration_telemetry`. Each executable performs its own warmup and
sampling; run each once per comparison round, alternating baseline/final order.
Raw output was saved to `/tmp/migration-perf-comparison-final.txt`; all reported
values are also recorded above.

Use separate artifact directories for future baseline/final builds. A shared
artifact directory reused a stale telemetry executable during this migration.
That comparison was discarded. The final telemetry crate was explicitly cleaned
and rebuilt before the reported measurements:

```sh
cargo +1.95.0 clean -p codex-otel --release
CARGO_PROFILE_RELEASE_LTO=false cargo +1.95.0 build --release \
  -p codex-otel --example migration_telemetry
```

### Retained changes and earlier evidence

The source fork's historical measurements remain available in
`a925b5b3c3843a08c85e38a47de8dd7f21d8fbdb:PERF_LOG.md`. They are historical
evidence, not measurements of this new base.

| Retained behavior | Source commits | Current evidence |
| --- | --- | --- |
| Skip inactive WebSocket telemetry and disabled response spans; borrow static event names | `b28d181e8f`, `146570235b`, `bceef00ef8` | Disabled telemetry measurements above |
| Reuse matching delta identifiers | `e688e994a4` | Delta mapping measurement and protocol tests |
| Parse ordinary WebSocket events once and reuse decoded metadata | `cf046d49f8`, `1c39bfc048` | API/telemetry correctness tests; historical fork measurements |
| Reuse request metadata and bypass repeated first-token timing locks | `724bec936f`, `34cc98b7b3` | Focused client/timing tests; historical fork measurements |
| Borrow notification method names and handler dependencies; move listener events | `3439fdfb9e`, `f624b14584`, `3aba0e58e9` | Protocol and app-server correctness coverage; historical fork measurements |
| Reuse filtered append batches, subscriber snapshots, and listener state | `9b13f5bc27`, `0a58077465`, `fc92d4927b` | Thread-store and subscriber/listener coverage; historical fork measurements |
| Disable release LTO | `9f0fe3beae` | Preserves fork build policy; both benchmark sides use the same setting |

### Rejected and omitted experiments

- Measurements taken while other builds were busy were discarded because of
  contention, especially in WebSocket telemetry.
- An accidental build with the root's Rust 1.97.1 default was excluded. All
  reported comparisons use the workspace's pinned 1.95.0 toolchain.
- The stale-artifact telemetry comparison was discarded and repeated after a
  crate-specific clean rebuild.
- `ff5b6f6ea5` is unnecessary: upstream already avoids retaining raw WebSocket
  payloads. Archive filename filtering is also already upstream.
- `7a0f0d2016` and `e0d928650c` depend on omitted fork-only realtime-history
  effects; `5066a20eaf` depends on omitted context-token projection.
- `90b443d162` belongs to the separate context/token-policy work, and
  `0aed56f3da` belongs to the separate reasoning-mode API. Neither was migrated.
- Broad token audits, dependency/default-feature changes, and platform packaging
  changes are outside this migration.

No new optimization candidate was retained on the strength of a noisy comparison.
Unmeasured migrated paths retain historical evidence only; future optimization
work should establish a fresh baseline for its particular workload.
