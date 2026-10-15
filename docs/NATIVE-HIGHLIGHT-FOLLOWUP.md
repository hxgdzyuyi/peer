# Native capture quality and bounded background work

This change follows the readable-viewport/UI/archive commit. It keeps Tree-sitter,
Longbridge gpui-kit and native GPUI rendering. It adds no JavaScript runtime,
semantic LSP, syntect dependency or general gpui-kit upgrade. The existing Cargo.lock remains unchanged (Tree-sitter
0.26.13 and gpui-kit revision 67265a36e260ffe03c5aaebdcd8d654dbf892eab).

Swift, C#, GraphQL, Proto and CMake now have actual highlight queries; native
unit tests require real string captures, not just successful parser creation.
The immutable diff capture pass parses complete old/new documents afresh. It
supports HTML JavaScript/CSS and Markdown fenced code with at most two injection
levels, 32 injected regions and 512 KiB per region. Host documents over 2 MiB
fall back to plain text. Parse/query execution shares a cooperative deadline,
a query match limit of 4096 and a capture limit of 100,000. Per-file preparation
shares a 100 ms execution/projection budget across both sides. Cancellation or exhausted work drops partial
capture output; source-span projection also checks cancellation/deadline. Generation checks still reject obsolete asynchronous results.
Fixed bundled query compilation is cached within each preparation pass and is not interruptible; its cold initialization time is excluded from that execution budget, including injected languages. It remains on the background worker. This avoids silently discarding all Swift captures on a cold language initialization; actual response evidence includes this initialization cost. Existing inline-diff computation is separate from the syntax budget. These are cooperative
work limits, not a hard wall-clock guarantee; max_line_length is only a display
filter. This diff path does not claim incremental parsing.

All six bundled themes define detailed native capture mappings. Imported VSCode
themes explicitly support a subset: editor colors, basic token scopes and font
styles, excluding contextual selectors and semantic token rules. Each application
starts from the selected light/dark defaults, so partial imports cannot retain
colors from the previous theme. Unsupported contextual and semantic import rules
are tested explicitly. A → B → A is checked in unit and real-window tests. Saving
or cancelling settings returns focus to the workspace, so Ctrl+P works after
the hidden theme filter is removed.

The repo-core full-context adapter is unchanged. Multiline comment, triple-quoted
string and heredoc fixtures place their opening delimiter beyond ordinary diff
context and check old/new captures. Work remains on the background per-file path;
reset/drop cancels obsolete capture work as well as rejecting its generation.

## Evidence

`cargo test --locked --workspace`: **76 passed, 2 ignored**. The extra regression
covers cold Swift query initialization inside Markdown without consuming the
parse/query execution budget. `cargo build --release --locked -p peer` completed
in 10 min 56 s. Linux executable SHA-256:
`9f42c609537d8ff96ecd8a577443e80711f0b45d7fb5c4577e8c5c1451f7e52e`.

The actual release-window highlighting run passed **50 checks**, including
startup/synchronization, independent center-source recognition, seven color
fixtures, partial-import A → Dracula B → A exact source-pixel restoration and a
large HTML selection followed by Swift with no later stale overwrite. Cancellation
of parse/query work itself is additionally tested at unit level; the window test
does not instrument whether a parser was in flight at the moment of the switch.
The baseline is the **native GPUI** executable from
`6981b97406f440ab843b904605fb927691d2553b`, not the original Tauri application.
Its 25 setup/source checks completed, with zero expected string-color pixels in
all seven fixtures and zero injected number/function pixels. The new binary has
115–177 matching string pixels and 37/30 injected HTML/Markdown pixels in the
same source crop. Raw results retain all observations.

Return-to-first-independent-source-OCR completion was **3.782–8.219 seconds**;
Return-to-observed-color completion was **4.640–9.020 seconds**. These are measured
software-rendered upper bounds including screenshots, OCR and (for colors) the
preceding identity/session checks, not isolated rendering latency. The baseline
mode's color-time field records its single color observation (zero pixels), not
time until colors appeared. No parsing or UI speedup is inferred from this run.
Large HTML → Swift took 6.645 seconds including palette/input/synchronization/OCR.
Final recordings run serially after compilation, at wall-clock speed without
time scaling. Development runs overlapping compilation are excluded.

The original Library video reread was denied with 403 scope_violation; no alternate
retrieval was attempted. Original-video frame-by-frame verification and real macOS
interaction have not been redone. This evidence does not establish full visual
acceptance. The separate [14.5 deployment-target correction](NATIVE-MACOS-14.5.md)
provides a rebuilt Apple Silicon candidate for the user's real-machine test.

The new release also passed the complete **79-check identity/write-target** run.
Its first attempt failed because normal OCR read the first italic `module10` as
`modulel10`, and the old regex silently skipped it for a later module12. The
retained screenshot and OCR show the actual source/session both were module10.
The final oracle reads only the first source marker, uses contrast and normal
renderings, and fails rather than skipping an unreadable first marker. The same
rule is applied to the tree source oracle. The failed attempt is preserved as a
development failure and excluded from pass counts.

The **1009-file tree run passed 39 checks**, retaining 45 detents and adjacent
44/46 cases, with independent source identity, tree/session agreement, exact
persisted review/comment paths and unchanged previous-file writes.

| Release scenario | Checks | Video |
|---|---:|---|
| Native 6981b97 baseline | 25 setup/source | [baseline recording](native-validation/highlight-release/baseline-release/interaction.mp4) |
| New highlighting | 50 | [highlighting recording](native-validation/highlight-release/final-release/interaction.mp4) |
| Complete identity/write-target rerun | 79 | [identity recording](native-validation/highlight-release/final-identity-rerun/interaction.mp4) |
| 1009-file tree | 39 | [tree recording](native-validation/highlight-release/final-tree/interaction.mp4) |

Exact commands and counts: [commands.json](native-validation/highlight-release/commands.json).
Binary and source provenance: [build.json](native-validation/highlight-release/build.json),
[source-inputs.sha256](native-validation/highlight-release/source-inputs.sha256).
Video metadata, hashes and complete decode results:
[videos.json](native-validation/highlight-release/videos.json).
Failed OCR attempt: [development record](native-validation/highlight-release/development-identity-ocr/README.md).
All new release recordings use the same immutable `peer-highlight-final` binary;
the baseline uses the separately retained `peer-readable-final` binary.
