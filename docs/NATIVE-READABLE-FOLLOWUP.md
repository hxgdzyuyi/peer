# Readable viewport, native review controls and application archive

This follow-up starts from `938036e406efa86683c8b04edec42627f9a4b083`,
which descends from the authorized `c9253aa96818b4d245b9f32950bf8db3884720a1`
baseline. The previous 44-detent tree result is **not acceptance** of the
reported 45-detent boundary. The new harness retains 45 and nearby 44/46 inputs.

## Changes under validation

The viewport identity uses the first complete nonempty painted source font band or
file/notice/fold label. Empty clipped padding cannot choose the review target.
Source geometry uses TextLayout's pixel-snapped line height and complete
x-height-to-baseline letter body band, excluding empty leading; it does not compare the fractional style
height against a rounded layout box. All scroll inputs converge after layout; generation, projection
and current-offset checks and explicit selection priority remain in force.
Sticky file headers use that rendered file, with path-bound review actions;
the sticky header itself is excluded from the identity geometry and pixel oracle.

Changes initially opens continuous All files. Explicit Single file is persisted.
Each file header has Reviewed and Comment on file controls bound to its own
FileContent. The new-comment composer always displays its full target path.
Existing comments have author/date and an explicit Outdated tag, and editing
uses a separate card-local input. Completion reconciles only the submitted
buffer, preserving a newer draft and preventing submitted drafts from returning.
Commit history has author/date/current check and closes on outside tree clicks.
The comparison toolbar shows mode, branch, totals and selected-commit metadata.

Original source comparison: `lantern24/peer` / local Git object
`72e854cd1a3c5bcb6d2e6de539cc6caa90c49959` (unmodified Tauri 0.1.2).
`src/CommitHistory.tsx` uses container containment for outside clicks and renders
author, date and a current-commit check; `src/Feedback.tsx` has independent editing,
You/date and Outdated. This is source evidence, not a claim that the parent's
original video has been viewed in this environment.

macOS packaging now creates `Peer-native-macos.zip` with `ditto --keepParent`.
The validator checks the Peer.app root, executable Unix mode, signature resources
and ZIP CRC. The build extracts the ZIP, checks executable mode and identical
binary bytes, and runs strict deep codesign verification on the extracted app.
GitHub's artifact contains this inner ZIP; extract it to obtain Peer.app.
No Release publication is requested or performed.

## Acceptance record

Linux release executable SHA-256:
`0a915ce174692d11b333bd4778bff3ab08836247f1826ba413e5563819b5e4ac`.
The final build includes history Escape capture and correctly positioned trigger/panel bounds. A second click on Commit history closes it; a prior release run exposed the missing absolute origin and was not accepted. `cargo test --locked
--workspace`: **63 passed, 2 ignored**; workspace/vendor formatting passed.
The archive validator accepted the synthetic valid ZIP and rejected missing root,
executable mode and signature resources. Actual macOS extraction/signature
verification still requires the new CI run.

Real release-window results (counts include setup and synchronization assertions):

| Script | Result | Evidence |
|---|---|---|
| `identity` | 79 passed | [result](native-validation/readable-release/release-identity/result.json), [normal-speed video](native-validation/readable-release/release-identity/interaction.mp4) |
| `original` | 28 passed | [result](native-validation/readable-release/release-original/result.json), [normal-speed video](native-validation/readable-release/release-original/interaction.mp4) |
| `tree` | 39 passed | [result](native-validation/readable-release/release-tree/result.json), [normal-speed video](native-validation/readable-release/release-tree/interaction.mp4) |
| `review` | 23 passed | [result](native-validation/readable-release/release-review/result.json), [normal-speed video](native-validation/readable-release/release-review/interaction.mp4) |
| `parity` | 17 passed | [result](native-validation/readable-release/release-parity/result.json), [normal-speed video](native-validation/readable-release/release-parity/interaction.mp4) |
| `gestures` | 10 passed | [result](native-validation/readable-release/release-gestures/result.json), [normal-speed video](native-validation/readable-release/release-gestures/interaction.mp4) |

The independent center-source oracle observed PageDown from module05 to module10;
only module10 received the new comment and review status. Find → QuickOpen config
→ Back restored main, its latest draft, mode and source; save/Back did not revive
submitted text. PageUp, Home/End, arrows, explicit bottom-file selection, sticky
Reviewed and scrollbar drag also passed. The final scrollbar case reads module00
from source pixels and matches its selected tree row and persisted session.
Numbered tree filenames normalize the observed OCR `0`/`o` ambiguity only.

The UI test exercised separate saved-comment and new-draft inputs, saving either
without losing the other, per-file controls with exact paths, Outdated metadata,
history outside/Escape dismissal, commit selection, Single-file restart and the
full long composer path. The intentional explicit bottom-file selection can
remain different from the first visible source file; viewport tracking must not
overwrite that explicit choice.

The 1009-file tree run kept the reported 45 wheel detents and adjacent 44/46
checks: center-source OCR, tree/session identity and exact persisted review/comment
paths agreed; previous files remained unchanged.
Review safety, renamed-source/deferred navigation, comparison restart and mouse/keyboard navigation gestures were rerun against this same release executable.
Exact commands and counts: [commands.json](native-validation/readable-release/commands.json).
Video durations, dimensions, hashes and full-decode results: [videos.json](native-validation/readable-release/videos.json).
Development failures and transient-frame/OCR harness failures are not counted as
passes. Final recordings are 15 fps wall-clock X11 captures without time scaling.
The source/binary provenance is recorded in
[build.json](native-validation/readable-release/build.json) and
[source-inputs.sha256](native-validation/readable-release/source-inputs.sha256).

All files still performs bounded batch reads (32 MiB and deferred-file exclusion),
not the original's lazy loading. A macOS build/signature check is not real-Mac
interaction acceptance. The original Library video reread returned 403 `scope_violation`, as confirmed
by the user; no alternate download URL is attempted. Original-video frame-by-frame
verification and real macOS interaction have not been redone. Source comparison,
prior records and current Linux recordings do not establish full visual acceptance.
