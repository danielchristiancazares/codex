# Upstream history lookup performance — 2026-09-04

This durable audit mirrors the current task's section in the maintainer's ignored root
`PERFORMANCE_LOG.md`; its earlier local performance records remain in place.

## Current Baseline — 2026-09-04 upstream history lookups

- Baseline code: `0043a0777a`, before upstream #41413. The original Unicode mapper and its existing test were moved without algorithm changes into a focused private module for production-path benchmark reuse.
- Retained source: `170da98842877c730a1d8ec9ee7421e54c06bb6d`; lazy per-turn item indexing and linear Unicode match-range mapping. Current retained-state timings and the accepted small-turn tradeoffs are recorded below.
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

### Retained result

- Retained the complete source intent, with a focused `Linear | Indexed` turn-index enum and private sibling implementation/tests. Duplicate IDs retain their first position; indexes remain attached through completion and rollback. Unicode matching preserves the original UTF-8 boundaries.
- The operator authorized retaining measured positive coding gains with their tradeoffs. At eight items, append cost increased by about 0.42 µs and late-update cost by about 0.10 µs; at 1,024 items, median append/update time decreased by 79.83%/84.56%. The 4,096-repeat Unicode highlight case decreased by 98.90%.
- Small nonmatching/control differences overlap the observed variance and carry no independent improvement claim. No additional algorithm variants were retained or rejected.
- Correctness: `just test -p codex-app-server-protocol -p codex-tui --config build.incremental=false -E 'package(codex-app-server-protocol) | test(history_search) | test(history_match_ranges)' --retries 0` passed 322/322 (303 protocol, 19 TUI), including Unicode, duplicate-ID, rollback, protocol/schema and composer behavior.
- Five candidate invocations used the identical baseline command/fixtures/sampling after the excluded build-and-run warmup. One additional final-state invocation passed and reconfirmed the retained gains; its medians are shown separately below.

| Case / size | Five retained medians (µs) | Current median (µs) | Time change | Final check (µs) |
| --- | --- | ---: | ---: | ---: |
| `thread_history_append/8` | 1.534, 1.037, 1.084, 1.097, 1.189 | 1.097 | +62.57% | 1.097 |
| `thread_history_append/32` | 4.137, 4.149, 4.194, 4.099, 4.237 | 4.149 | +7.02% | 4.234 |
| `thread_history_append/128` | 37.32, 31.83, 31.55, 32.03, 31.35 | 31.83 | -23.94% | 33.48 |
| `thread_history_append/1024` | 219.2, 222.9, 221.7, 226.3, 231.2 | 222.9 | -79.83% | 245.4 |
| `thread_history_late_updates/8` | 0.7898, 0.7898, 0.7823, 0.7748, 0.8323 | 0.7898 | +14.50% | 0.7748 |
| `thread_history_late_updates/32` | 4.144, 4.092, 4.054, 4.037, 4.314 | 4.092 | +13.86% | 4.067 |
| `thread_history_late_updates/128` | 14.28, 13.43, 13.97, 15.28, 14.59 | 14.28 | -45.66% | 14.32 |
| `thread_history_late_updates/1024` | 164.1, 144.5, 181.3, 215.1, 183.9 | 181.3 | -84.56% | 186.3 |
| `unicode_highlight_no_match/16` | 1.259, 1.009, 1.144, 1.357, 1.014 | 1.144 | -6.54% | 1.159 |
| `unicode_highlight_no_match/256` | 9.289, 9.237, 9.254, 9.534, 9.417 | 9.289 | -5.88% | 10.36 |
| `unicode_highlight_no_match/4096` | 253.1, 241.7, 244, 245.4, 265.5 | 245.4 | -11.82% | 295.2 |
| `unicode_highlight_ranges/16` | 1.174, 1.184, 1.169, 1.169, 1.174 | 1.174 | -16.38% | 1.202 |
| `unicode_highlight_ranges/256` | 11.5, 11.68, 11.57, 11.51, 11.64 | 11.57 | -88.82% | 12.09 |
| `unicode_highlight_ranges/4096` | 289.8, 275.8, 290.3, 291, 300.9 | 290.3 | -98.90% | 357.2 |
