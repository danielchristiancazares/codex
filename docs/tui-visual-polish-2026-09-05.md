# TUI visual polish — September 5, 2026

## Intent and review method

Improve the actual terminal interface against the maintainer's four supplied desktop/web
references (Claude, ChatGPT, Gemini, and Cursor), plus the terminal screenshot linked from
[OpenCode's official documentation](https://opencode.ai/docs/).

Each critic is a fresh agent with no inherited conversation, source-change history, or previous
review. The critic receives the artifacts and an evidence-only, harsh comparison brief. Reviewers
are retired after their final report and are never reused. Logos remain visible, so identity
blinding is partial. Review outcomes are recorded as received; universal superiority requires
evidence beyond a favorable opinion about a few still frames.

## Round 1 — baseline

The critic ranked the supplied initial/active frames: Gemini, Cursor, Claude, OpenCode, Codex,
ChatGPT. Codex's directional score was 6.6/10. Its responsiveness was the strongest evidenced
dimension, while input affordance, hierarchy, composition, and distinctive appeal lagged.

The first review used exact TUI snapshots because the pixel-export build was still running.
Color and typography judgments were explicitly left unproven.

Actionable findings:

- Empty input needed a semantic invitation and visible send/newline guidance.
- The open composer rail lacked a complete boundary and a consistent content grid.
- The initial screen emphasized setup metadata over the next useful action.
- Queue previews hid the newest item while advertising edit-latest, repeated labels, and consumed
  too many transcript rows.
- Supporting metadata and essential shortcut hints needed clearer priorities and contrast.
- Loading, narrow, multiline, active, approval, error, light-theme, and low-color states needed
  direct visual evidence.

## Implementation in progress

- A bounded initial workspace delegates input, cursor placement, and editing to the existing
  composer. It is eligible only for a ready, empty session whose transcript contains session
  metadata. Real history, warnings, active work, overlays, and parent-owned input retain their
  existing conversation surfaces.
- The composer has a complete rounded outline, a terminal-derived neutral surface, and keymap-
  derived send/newline cues. Embedded startup fields retain their restricted input semantics.
- Empty ready input and startup drafting have explicit placeholder copy.
- Queue previews show newest first, identify the latest editable item, and use a five-row cap for
  queue-only content (three below 60 columns). This changes presentation only; execution remains FIFO. Pending and rejected
  steers retain priority and the existing larger row budget.
- Startup and ready framing share a bounded column and reserved composer/footer geometry. Protected
  onboarding and session-picker screens retain their input ownership; startup still rejects submission.
- Essential secondary text derives a neutral foreground from the terminal palette. Low-color
  terminals use their default foreground. Working input shows the actual configured queue shortcut
  beside the send shortcut when both fit.

## Round 2 — first rendered revision

The fresh critic ranked Claude (9.0), Gemini (8.6), ChatGPT (8.2), Codex (7.9), OpenCode (7.5),
and Cursor (7.1). Its emotional verdict was **polished**, short of exceptional. Codex received
9/10 for input affordance and discoverability, and strong marks for working-state clarity.

The principal defect was an 80-column starter-action row whose final label clipped silently.
The next revision measures the complete row against the actual inset width and stacks the actions
when necessary. Other findings concerned light-theme contrast, narrow queue density, competing
header conventions, send/queue clarity, and sparse startup/active frames. The review also requested
approval, error, menu, populated-transcript, and low-color evidence; these are added to the gallery.

Every review result remains independent. A favorable impression of selected screenshots does not
establish platform-wide behavior, terminal-host typography, or universal superiority.

## Round 3 — operational gallery

The fresh critic ranked Claude, Gemini, Codex, Cursor, OpenCode, and ChatGPT. It judged Codex
strongest in operational terminal design, active-work clarity, keyboard discovery, state coverage,
and demonstrated responsiveness. Its overall emotional grade remained **polished**.

The next revision addresses:

- Approval commands use a bold terminal-default foreground, preserving exact text and readable
  contrast independently of the selected syntax theme. Selected option explanations retain their
  secondary role instead of inheriting the underlined action-label treatment.
- The hydrated gallery header takes its model, reasoning, and version from the same state used by
  the widget. The former fixture mixed an explicit `high` header with an unspecified-effort footer
  and a placeholder version. Recognized GPT labels now share one formatter.
- The compact startup frame reserves a progress/identity line, and placeholders disclose clipping.
- Command menus use a one-column list below 60 columns and reserve a stable area for the selected
  description. Picker-owned input shows `enter select` and navigation/close hints. Landing actions
  disappear while a popup owns input.
- Pixel-review exports use native modifier names. Portable golden snapshots retain their existing
  deterministic modifier convention; production already used `alt` on Windows and Option on macOS.
- Error prose uses an 88-column maximum measure and preserves its original raw-copy text.
- Composer title styling clears inherited dimming on low-color terminals.

The maintainer explicitly chose to **keep visible Markdown heading markers** after the critic
recommended removing them. The existing heading convention remains in effect.

## Evidence and validation

`chatwidget::tests::visual_review::visual_review_gallery` renders production widget buffers at
120×36, 80×24, and 48×20 in light, dark, and 16-color palettes. Setting `CODEX_TUI_REVIEW_DIR` exports cell
symbols and styles as JSON for pixel review. The review rasterizer uses Cascadia Mono and the
exported colors. These are rasterized production buffers, with an explicitly drawn cursor;
terminal-host rasterization, motion, and platform integration require separate checks.

Baseline: `just test -p codex-tui visual_review_gallery` passed (1 test, 4,444 skipped).

Round 2: both gallery tests passed. The full TUI run executed 4,438 tests: 4,228 passed,
210 failed, and 10 were skipped. Most failures were expected visual snapshot changes; the run
also exposed composer-gutter assertion updates and a right-reserve painting defect. The composer
frame now respects the ambient-pet reservation. Existing baseline failures remain separately
identified in the upstream-backport audit; pending snapshots are being reviewed individually by
change family.

Round 3: both expanded gallery tests passed. The focused follow-up ran 69 tests: 51 passed
(including the startup-to-ready cursor/draft handoff and composer right-reserve check), 18 failed,
and 4,381 were skipped. The failures comprised pending snapshots and two status-style assertions
whose former dim-text expectations were updated. The new supporting-text contrast check measured
6.44:1 in the dark review palette and 5.69:1 in the light palette.

During round 4 compilation, the new error cell passed owned lines to a borrowed-line conversion
helper. This was corrected by iterating by reference; the now-unused composer `Margin` import was
also removed. Compilation and the next gallery review are pending.

Argument-comment validation was attempted through the documented Windows-capable prebuilt
wrapper, `python tools/argument-comment-lint/run-prebuilt-linter.py -p codex-tui`. Its driver built,
but the pinned `nightly-2025-09-18` compiler (Rust 1.92) cannot check current dependencies requiring
Rust 1.94–1.96 (`sqlx` 0.9 and `rama-macros` 0.3). This check remains unavailable until the lint's
toolchain/package is updated. No dependency changes were made to work around that mismatch.

Final validation, snapshot review, and subsequent independent verdicts are pending.

Existing worktree edits and preexisting pending snapshots belong to a separate upstream-backport
task. They are preserved and excluded from this pass's ownership.
