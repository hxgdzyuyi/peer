# Review safety and workflow parity follow-up

The later independent acceptance found two All-files review-target errors that
these historical results missed. See [the identity follow-up](NATIVE-IDENTITY-FOLLOWUP.md)
for the repair, independent center-pixel assertions, and new execution evidence.

Original reference: `72e854cd1a3c5bcb6d2e6de539cc6caa90c49959`.
The supplied independent desktop comparison examined native `db2e860`;
this follow-up starts from the already delivered tree repair `1dbfa7a`.
Original source was checked with Git, alongside the supplied desktop findings.
All new execution evidence below comes from the real Linux native application,
not a browser preview. macOS execution is not inferred from these results.

## Findings, implementation and acceptance

| Finding | Implemented behavior | Executed acceptance |
|---|---|---|
| 1: Editing overwrites a new draft | Saved-comment editing suspends a separate complete new draft. Save, Cancel edit and Discard edit restore it. | Review safety window suite: cancel, save, discard, cross-file edit restoration |
| 2: Navigation changes a comment anchor | Navigation and comment positions are separate. The gutter annotation control or explicit Line scope starts a line comment; ordinary clicks only navigate. Multi-file activation restores body, side, line, fingerprint, editing identity and suspended draft. | Old-line comment followed by a different source click; A → B → A in All files; persisted anchors checked after saving |
| 3: Refresh hides stale drafts | Stable `draft-v2:<queue>:<path>` entries retain the original fingerprint. A changed source keeps the body and location visible, blocks submission, and requires explicit File scope or Line scope confirmation. | Legacy-format restart, external modification, Refresh, blocked save and explicit reattachment |
| 4: Renamed old-side definitions use the wrong path | Original-side requests map display paths back to `FileContent.old_path`. | Real Pyright on `old.py → new.py`; old-side definition opens old.py |
| 5: Navigation gestures and version choice | Ctrl/Cmd-click and native context-menu definition/reference actions; a unique definition opens directly. Original A / Modified B map unchanged lines to the counterpart's actual line number. | Linux Ctrl-click and both context actions; shifted common-line A/B navigation; exact target clipboard text |
| 6: Incomplete Back restoration | Search and line jumps push history. Native Diff checkpoints retain expansion, collapsed files, text/gutter selection, viewport and horizontal offset; application history retains source, result panel, target range, navigation anchor and column. | Search Back restores exact target selection; actual mouse text selection survives definition/Back; expanded context markers remain visible; prior navigation suite rerun |
| 7: Deferred navigation loses its target | Retain the complete LSP range; apply line, UTF-16 column and byte-exact selection after either initial or explicit source loading. | Definition at line 22,001, explicit large-source load, exact `deep_target` clipboard selection, search/Back |
| 8: Comparison controls | Untracked toggle, branch picker and Changes restore action; only changes views update `last_changes`. | Branch → Code → Changes, Untracked off, restart preserves comparison and toggle; commit → working restores the original default queue |
| 9 / R1: Commit history closure and paging | Toggle, Escape, outside click and commit selection close the panel. In-flight loading is gated; appended commit IDs are deduplicated. | 66-commit fixture: first 50, rapid repeated pagination reaches exactly 66; outside close and selection close |
| 10: Find and comment keyboard behavior | Query changes recompute highlights; clear removes them; initial Shift+Enter starts at the last hit. Ctrl/Cmd+Enter submits comments. | Live query replacement/clear, forward/reverse navigation, keyboard comment save; smoke and navigation reruns |
| 11 / D1–D5: Tree and feedback | Existing structural tree/QuickOpen repair retained; selection accent and counts added. External activation reveals ancestors and clears an incompatible filter without moving focus; candidate selection follows the active file even while the filter has focus. | 1,009-file fixture, forward/reverse wheel, focused filter, current-file comment and Reviewed targets, deep paths and same-name file/directory transition |
| R2: Sidebar controls | Actual baseline probe confirmed drag worked, but arrows and double-click did not. Native focusable dividers now support arrows, Shift+arrows, Home/End, double-click reset and Escape rollback. Widths remain bounded on a narrower display. | Pointer drag, all keyboard operations, Escape restore and restart persistence |

The divider follow-up uses GPUI elements directly; it does not introduce a browser
or a second UI toolkit. The pinned GPUI version and independent Rust core remain
unchanged. The earlier Settings transaction and Tree-sitter rendering differences
remain documented design choices, not claimed missing features.

## Persistence and races

The native draft reader accepts the first migration's fingerprint-keyed records,
including an older nonempty draft when the current fingerprint has an empty view.
It checks the fingerprint suffix so colon-containing paths cannot accidentally
match another file. Existing legacy entries remain intact; a v2 empty draft is an
explicit current state and does not resurrect them. Core review queues, preferences,
`state.json`, directory semantics and environment-variable precedence are retained.

Save completion compares the complete submitted draft identity, including location,
fingerprint and suspended editor state. It does not clear text or an anchor changed
while saving. Conditional cleanup drains queued session writes before removing a
submitted draft, preventing a rapid file switch from restoring it after a successful
save. An unrelated background failure no longer unlocks an in-flight comment save.
Frozen navigation sources cannot submit or retarget review comments.

These operations continue through Tokio and the existing storage gate. No Git write
operation, AI feature, credential or security-setting change was added to the app.

A full smoke rerun also exposed a comparison identity regression: commit comparisons
normalize Untracked to false in the core. Returning to working changes now restores
the original app's default instead of silently selecting another review queue.
The 46-assertion smoke checks this transition and subsequent outdated-comment refresh.
Pending comparison state also preserves the toggle across rapid requests.

## Evidence

Final locked release acceptance: **61 workspace tests passed** (two opt-in suites
ignored), and **192 named window assertions passed across 11 scenarios**.
[Exact counts and binary hash](native-validation/parity-release/summary.json) distinguish
these assertions from independent feature counts. The two actual-process recordings
are 70.67 seconds (review) and 65.60 seconds (navigation), with full decode verified.

Final results, build/test logs, actual-process executable hashes, screenshots and
recordings are retained in [parity-release](native-validation/parity-release/).
The [review recording](native-validation/parity-release/review/interaction.mp4) and
[navigation recording](native-validation/parity-release/navigation/interaction.mp4)
show actual X11 input against the optimized executable. These are unedited native
captures of synthetic repositories. `process.json` records the running executable
and its SHA-256; the external delivery manifest maps the final commit to its binary.

Useful visual checkpoints:

- [Cancelled edit restores the new draft](native-validation/parity-release/review/01-cancel-restores-draft.png)
- [Stale draft remains visible](native-validation/parity-release/review/02-stale-draft-visible.png)
- [Multi-file draft restoration](native-validation/parity-release/review/03-multi-file-draft.png)
- [Renamed old-side definition](native-validation/parity-release/navigation/01-renamed-old-definition.png)
- [Deferred navigation target loaded and selected](native-validation/parity-release/navigation/03-loaded-target-range.png)
- [Native context menu](native-validation/parity-release/gestures/02-context-menu.png)
- [Expanded context restored](native-validation/parity-release/gestures/05-expanded-context-restored.png)
- [Tree follows the visible Diff](native-validation/parity-release/tree/10-multifile-scroll-sync.png)

Harness fixes are explicit: Tesseract TSV is parsed with quoting disabled because
source-code quote characters otherwise swallow subsequent rows; merged OCR words
are matched without treating a missing space as a missing button; the tree-only OCR
crop is enlarged 2× to distinguish icon-adjacent filenames. GTK folder selection
leaves Recent before entering its path. Tests wait for rendered controls and use
current button labels rather than stale coordinates after layout changes. Assertions
still use actual input, pixels, clipboard and resulting persistent state. An early
suite whose runner was edited while running produced a shell EOF error after its
Python assertions passed; that run is not counted as final acceptance.

## Reproduction

With the Linux dependencies and external language servers described in
[VALIDATION.md](VALIDATION.md), build and run on disposable displays:

```sh
cargo test --locked --workspace
cargo build --locked --release -p peer
export PEER_NATIVE_BINARY=target/release/peer
for scenario in review parity gestures resize history smoke desktop navigation errors tree transition; do
  scripts/test-native-linux.sh "$scenario" "test-results/final-$scenario"
done
```

Set `PEER_RECORD=1` to capture the actual X11 session. The tests generate their own
Git fixtures and isolated application data; they must not target a person's desktop.

## Limits

Assertion counts include setup/lifecycle checks and are not independent feature
counts or exhaustive scheduling proofs. The rapid-save test exercises real input;
unit checks also cover preservation of a changed body, anchor and fingerprint.
The final recordings and logs establish Linux native behavior. macOS 15+ menus,
Cmd gestures, IME, signing and packaging still require macOS execution. The parent's
independent desktop comparison remains an additional acceptance pass.
