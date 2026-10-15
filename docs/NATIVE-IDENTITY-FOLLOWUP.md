# All-files review identity follow-up

Repair baseline: `c9253aa96818b4d245b9f32950bf8db3884720a1` on
`native/gpui-migration`. The independent desktop acceptance found two P1 errors:
PageDown moved the source without changing the review target, and Find → QuickOpen
→ Back restored an earlier Diff while retaining the later review file. Previous
wheel-only results do not establish correctness for these paths.

## Implementation

Diff observes the virtual list after layout/clamping, regardless of the scrolling
source. Wheel, keyboard and the toolkit scrollbar mark scrolling intent; they no
longer publish a file from pre-layout offsets. Deferred events check the source
revision, row projection and final offset. Explicit file/line selections and Back
retain their identity across repaint/reflow, including bottom clamping of short
files, until another scroll input requests viewport following.

Navigation uses one bounded checkpoint stack for file jumps and source navigation.
Each checkpoint retains the review FileContent identity, loaded-file map, All files
mode, native Diff checkpoint, navigation source/results, target range and column.
Back saves the departing draft, invalidates pending source work, restores the
complete identity, and reads the latest stable per-file draft. Checkpoints never
hold old draft copies. Save completion checks the stable draft key, so returning
to the same file during a save cannot revive the submitted body. Session writes
are chained in UI order instead of relying on Tokio mutex acquisition order.

The Windows idle-session test now advances the observation time by 301 seconds
instead of subtracting 301 seconds from an Instant. The production expiration
threshold and ownership test are unchanged. A non-expired observation is also
asserted before retirement.

## Regression method

`tests/native_identity.py` puts a distinct file token in every fixture source
line. Its oracle reads only the center source pixels, then checks the selected
tree-row pixels and persisted session against that independently observed file.
Actual comment and Reviewed writes must land at that path; earlier files must
remain untouched. It exercises Find → QuickOpen → Back with separate latest
per-file drafts, Back after saving, leaving/restoring All files, interrupted
source loading, PageUp/Down, Home/End, arrows, explicit bottom selection and the
vertical scrollbar. `tests/native_tree.py` now independently reads the center's
file header/source token instead of deriving its expected target from selected().

The exact baseline was rebuilt from a clean detached checkout. The new independent
window oracle failed on it in both reported ways (expected negative controls):

- `baseline-scroll-5`: PageDown center `src/module11.rs`, persisted selection and
  tree `src/module05.rs`.
- `baseline-back`: Find → QuickOpen config → Back center `app/main.rs`, persisted
  selection/tree `config/app.json`, with the config draft still displayed.

The negative controls stop at the identity failure; they do not claim to exercise
baseline incorrect writes. Positive checks perform actual comment/Reviewed writes.
Baseline executable SHA-256:
`6b4e9fc0ea7b4ab75e3edd06eeac89d5ca27453f3b47c630361648f9a514de19`.
The retained videos are uninterrupted wall-clock captures, not sped-up demonstrations;
`video.json` reports actual frame rate/duration and successful full decoding.

Commands (with Rust 1.95, Node 22, native Linux SDK, X11 utilities and Pyright on PATH):

```sh
cargo test --workspace --locked
cargo fmt --all -- --check
cargo build --locked --release -p peer
PEER_NATIVE_BINARY=/path/to/baseline/peer PEER_RECORD=1 PEER_IDENTITY_SCROLL_ONLY=1 scripts/test-native-linux.sh identity test-results/p1/baseline-scroll-5
PEER_NATIVE_BINARY=/path/to/baseline/peer PEER_RECORD=1 scripts/test-native-linux.sh identity test-results/p1/baseline-back
PEER_NATIVE_BINARY=target/release/peer PEER_RECORD=1 scripts/test-native-linux.sh identity test-results/p1/release-identity-verified
```

The synthetic identity harness disables X11 key auto-repeat in its disposable
server, verifies both initial drafts before leaving, waits for the palette's
rendered title, and includes the Diff's first pixel row in its center crop. Earlier
harness-development attempts with cropped text, transient frames or duplicated
synthetic keystrokes are not counted as passes. The 1,009-file tree scenario
allows up to 180 seconds for batch loading on this cloud runner (the initial
40-second wait timed out while the window still showed Loading); this is not
a load-time performance acceptance. The tree wheel fixture uses 44 detents to
leave the first source text readable. At 45, the preceding file had only a few
blank bottom pixels visible and OCR read the next complete file. Viewport tracking
uses the first partially visible row, including such a clipped tail; it does not
implement a first-fully-readable-line threshold. That boundary is not accepted by
the text-only OCR oracle and is not counted as a passing assertion.

Release build succeeded with the pinned Longbridge gpui-kit revision and native
renderer. Executable SHA-256:
`dca15ae47f89368a8aee5f67106d827bfcd120fef2a3dd68270d886112ba6c55`.
`cargo test --workspace --locked`: **61 passed, 2 ignored**, no failures.
Workspace and changed vendor Rust formatting checks passed.
Real release-window results (assertion counts include setup/synchronization,
not independent feature counts):

| Scenario | Passed | Evidence |
| --- | ---: | --- |
| All-files identity | 77 | [Exact assertions](native-validation/identity-release/release-identity/result.json), [video](native-validation/identity-release/release-identity/interaction.mp4), [persisted state](native-validation/identity-release/release-identity/state.json) |
| Review/draft safety | 22 | [Exact assertions](native-validation/identity-release/release-review/result.json) |
| Navigation parity | 17 | [Exact assertions](native-validation/identity-release/release-parity/result.json) |
| Navigation gestures | 10 | [Exact assertions](native-validation/identity-release/release-gestures/result.json) |
| File tree (1,009 files) | 29 | [Exact assertions](native-validation/identity-release/release-tree/result.json) |
| Window smoke | 46 | [Exact assertions](native-validation/identity-release/release-smoke/result.json) |

The identity run persisted comments only at `app/main.rs` and `src/module11.rs`;
Reviewed contains those paths plus the explicitly selected `src/module39.rs`.
Neither old `src/module05.rs` nor `config/app.json` was written. Its native process
hash matches the release executable. The 275.2-second 1440×900 recording decoded
without errors; actual average frame rate is recorded in `video.json`.

The additional release scenarios use the same invocation with `PEER_NATIVE_BINARY=target/release/peer`,
scenario `review`, `parity`, `gestures`, `smoke` or `tree`, and output directory
`test-results/p1/release-<scenario>` (the final tree rerun is
`test-results/p1/release-tree-complete`). [Build logs and evidence](native-validation/identity-release/)
retain exact outputs, screenshots and binary hashes. Screenshots for the two P1
transitions, saved empty draft, bottom explicit selection, scrollbar and navigation
target were visually inspected.

Windows execution must be checked in the CI run attached to the pushed commit;
Linux success and a macOS bundle build are not Windows or real-Mac interaction
verification.

## External acceptance boundary

The unmodified original reference is `lantern24/peer` at
`72e854cd1a3c5bcb6d2e6de539cc6caa90c49959` (Tauri 0.1.2). The parent's original and
independent GPUI videos are Library files `libfile_00f9008162fc8191a555bad4f048b93d`
and `libfile_38788f35955881919902f7f56ac1184a`. This environment exposes no Library
read/download action, so those videos were not viewed here. The parent will repeat
acceptance on its 74-file/69-change desktop fixture after delivery. Local synthetic
Linux window checks do not replace that comparison or verify real macOS gestures,
IME or dialogs. No main merge or Release is part of this task.
