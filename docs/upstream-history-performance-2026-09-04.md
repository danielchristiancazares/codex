# Upstream history lookup performance — 2026-09-04

This durable audit mirrors the current task's section in the maintainer's ignored root
`PERFORMANCE_LOG.md`; its earlier local performance records remain in place.

## Current Baseline — 2026-09-04 upstream history lookups

- Baseline code: `0043a0777a`, before upstream #41413. The original Unicode mapper and its existing test were moved without algorithm changes into a focused private module for production-path benchmark reuse.
- Candidate under review: `170da98842877c730a1d8ec9ee7421e54c06bb6d`; lazy per-turn item indexing and linear Unicode match-range mapping. Candidate measurements and disposition are pending.
- Host: AMD Ryzen 9 9900X, 12 cores / 24 logical processors, Windows x86-64, High performance power scheme. Rust 1.98.1 (`48a229cea`).
- Profile: Cargo `bench`, opt-level 3, LTO off, 16 codegen units, line-table debuginfo. This run overrides incremental compilation off to bound cache growth.
- Exact command from repository root: `just bench --config build.incremental=false --jobs 4 -- history_lookups`.
- Sampling: one excluded warmed invocation, then five recorded invocations; each case uses 50 samples × 20 iterations. A dispatcher interruption lost one final invocation's output; that unrecorded invocation was replaced with the fifth complete recorded run.
- Fixtures: `ThreadHistoryBuilder::handle_event` appends 8/32/128/1,024 uniquely identified nonempty plan items, or updates those items in reverse order after turn completion. Both include `finish()`; input setup is outside the timed region and verifies the complete item list. Unicode fixtures repeat `İ a ` 16/256/4,096 times, querying `i` or nonmatching `z` through the production mapper.
- These isolate history-reducer and composer computation. Buffer diff, ANSI serialization, and terminal/VT100 end-to-end costs are outside this candidate's measured scope.
- Read-only source reviews ran concurrently. Per-run variation is visible below; small differences within these ranges are noise. Retention requires repeatable gains beyond that variation.
- Initial release build exited with compiler-process `STATUS_HEAP_CORRUPTION` in `codex-memories-write`. The four-job retry and all recorded benchmark invocations completed successfully; production code was unchanged during baseline collection.

| Case / size | Five warmed medians (µs) | Baseline median (µs) |
| --- | --- | ---: |
| `thread_history_append/8` | 0.6648, 1.164, 0.6948, 0.6498, 0.6748 | 0.6748 |
| `thread_history_append/32` | 3.712, 6.242, 5.817, 3.697, 3.877 | 3.877 |
| `thread_history_append/128` | 41.85, 59.11, 52.46, 35.38, 38.88 | 41.85 |
| `thread_history_append/1024` | 1874, 1105, 1813, 1105, 1105 | 1105 |
| `thread_history_late_updates/8` | 1.002, 0.6798, 0.6848, 0.6898, 0.6898 | 0.6898 |
| `thread_history_late_updates/32` | 5.799, 3.589, 3.544, 3.594, 3.614 | 3.594 |
| `thread_history_late_updates/128` | 48.97, 24.96, 26.28, 24.76, 26.78 | 26.28 |
| `thread_history_late_updates/1024` | 1839, 1187, 1147, 1107, 1174 | 1174 |
| `unicode_highlight_no_match/16` | 1.602, 1.179, 1.179, 1.224, 1.534 | 1.224 |
| `unicode_highlight_no_match/256` | 9.157, 9.579, 9.869, 9.924, 15.14 | 9.869 |
| `unicode_highlight_no_match/4096` | 319.8, 278.3, 247, 234.8, 313.5 | 278.3 |
| `unicode_highlight_ranges/16` | 2.417, 1.359, 1.379, 1.404, 2.209 | 1.404 |
| `unicode_highlight_ranges/256` | 161.3, 103.5, 99.07, 84.14, 165.1 | 103.5 |
| `unicode_highlight_ranges/4096` | 26360, 24100, 26900, 25920, 28220 | 26360 |
