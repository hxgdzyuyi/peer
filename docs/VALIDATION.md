# GPUI native validation — 2026-10-14

See [the subsequent readable-viewport and UI follow-up](NATIVE-READABLE-FOLLOWUP.md)
for the 45-detent boundary, independent editors, file controls and archive checks.

The later independent acceptance found two All-files review-target errors that
these historical results missed. See [the identity follow-up](NATIVE-IDENTITY-FOLLOWUP.md)
for the repair, independent center-pixel assertions, and new execution evidence.

Baseline: `72e854cd1a3c5bcb6d2e6de539cc6caa90c49959`, independently checked against upstream main. Implementation branch: `native/gpui-migration`. Historical WebView results are isolated in [VALIDATION-WEBVIEW-BASELINE.md](VALIDATION-WEBVIEW-BASELINE.md); none count as native acceptance.

## Review safety and workflow follow-up

[The subsequent parity report](NATIVE-PARITY-FOLLOWUP.md) maps the independent
review's eleven source findings and two runtime risks to implemented fixes and
new native-window evidence. Its final result files supersede earlier assumptions
about draft isolation, anchors, navigation history and workflow completeness.

## File-tree follow-up

The user's desktop video review subsequently identified gaps in the original
native tree acceptance. [The follow-up report](NATIVE-TREE-FOLLOWUP.md) documents
the centered-label/space-indent implementation, missing icons/asset registration,
missing ancestor reveal and multi-file scroll tracking, and their native repairs.
Its richer same-fixture screenshots and window tests supersede earlier tree-parity
inferences. The original baseline remains available for comparison.

## Executed checks

| Check | Result and evidence |
|---|---|
| Core suite | 48 passed; two opt-in real-server suites skipped by the standard invocation. [Log](native-validation/core-tests.log) |
| Native adapter and directory tests | 5 passed: absent sides, rename, Unicode filename, CRLF, unterminated lines, deferred/binary/empty content, 100-file actual native parser, environment-variable precedence. [Log](native-validation/native-tests.log) |
| Real language servers | Go (gopls), TypeScript, JavaScript and Rust (rust-analyzer) opt-in integration passed. [Log](native-validation/all-real-lsp.log) |
| Real Python LSP | Pyright 1.1.407: opt-in definition/reference/import/stub/method regression passed independently. [Log](native-validation/python-lsp.log) |
| Native executable | Rust 1.95.0, locked dependencies, Linux debug binary successfully linked. [Log](native-validation/link.log) |
| Formatting | `cargo fmt --all -- --check` passed. [Log](native-validation/format.log) |
| Actual native window regression | Repeated Xvfb/llvmpipe runs, final reproducible smoke: 45 state/lifecycle/pixel assertions passed, with real mouse/keyboard input. [Script](../tests/native_smoke.py), [result](native-validation/smoke/result.json), [screenshots](native-validation/smoke/) |
| Native desktop workflows | 29 assertions: actual GTK folder/export/import dialogs, clipboard equality, theme removal, recent-root relocation/removal, comment jump/context, discard, feedback toggle and confirmed queue clear. [Result](native-validation/desktop/result.json), [script](../tests/native_desktop.py) |
| Navigation regression | 9 assertions with real Pyright: frozen-source search, line jump, Back restores original anchor/draft, definitions and references. [Result](native-validation/navigation/result.json) |
| Dialog error recovery | 5 assertions with deliberately unavailable D-Bus in the app process: folder, export and import errors become visible and the UI remains usable. [Result](native-validation/dialog-errors/result.json) |
| Comparison modes | Code, all, staged, unstaged, branch, commit and rapid switching exercised in the window. [Record](native-validation/ui-modes.json) |
| Comments and Reviewed | Old/new-line anchor 2 and file-scope comments, edit/delete, save, draft switching, restart, outdated retention and fingerprint invalidation. [Record](native-validation/ui-stale.json), [screenshot](native-validation/31-outdated.png) |
| Native clipboard and save dialog | Real clipboard JSON equals file saved through native GTK portal dialog; old side and outdated flags preserved. [Record](native-validation/export-result.json), [dialog](native-validation/35-export-dialog.png) |
| Folder and theme import dialogs | Real native desktop portal windows; imported theme appears in settings. [Folder](native-validation/34-native-folder-dialog.png), [theme](native-validation/41-theme-imported.png) |
| LSP window interaction | Actual Pyright definition result, frozen source, two references, back navigation. [Definition](native-validation/18-definition-fixed.png), [source](native-validation/19-definition-source.png), [references](native-validation/20-references.png), [back](native-validation/21-back.png) |
| Large changes | 100 files, 10,000 old + 10,000 new lines rendered in virtualized multi-file Diff; 22,000-line source requires explicit load. [Screenshot](native-validation/22-large-multifile.png), [record](native-validation/large-workload.json) |
| Scroll and themes | Source-line scroll position restored after process restart; dark theme and draft persisted. [Scroll](native-validation/28-scroll-restored.png), [theme](native-validation/14-restart-draft-theme.png) |
| Recording | [24-second actual X11 interaction video](native-validation/native-interaction.mp4), captured from the running native executable, not a mockup |
| Dependency/startup audit | [Audit](native-validation/dependency-audit.json), [Linux tree](native-validation/dependency-tree-linux.txt), [macOS tree](native-validation/dependency-tree-macos.txt) |

## Reproduction

On a Linux graphical development host with the GPUI platform development libraries, Git, Rust 1.95, Node 22 (core fake-LSP tests only), Xvfb, xfwm4, wmctrl, xdotool, xclip, ImageMagick, Tesseract, D-Bus, GTK desktop portals and Mesa Vulkan (navigation additionally needs Pyright on PATH):

```sh
cargo test --locked -p peer-core
cargo test --locked -p peer
cargo build --locked -p peer
cargo fmt --all -- --check
scripts/test-native-linux.sh smoke test-results/native-smoke
scripts/test-native-linux.sh desktop test-results/native-desktop
scripts/test-native-linux.sh navigation test-results/native-navigation
scripts/test-native-linux.sh errors test-results/native-errors
```

The harness creates disposable repositories and a dedicated PEER_DATA_DIR. It sends actual input, verifies the on-disk state and records screenshots. It must run in a disposable display, never a colleague's active desktop. UI screenshots require visual inspection; persisted-state assertions alone do not prove pixels or hit targets.

This cloud host is Debian 13 with no permitted system package writes. Official Debian packages were downloaded/extracted to `/workspace/peer-native-sdk`; its activate script configures headers, libraries, Clang and pkg-config. Rust lives under `/workspace/.toolchains`. Xvfb used Mesa llvmpipe software Vulkan. Folder/export/import tests used an isolated D-Bus session and locally extracted xdg-desktop-portal/GTK. No credentials or security settings were added or changed. The initial apt install failed with OS permission denied; local extraction resolved the build dependency need.

The numeric screenshots retain intermediate investigation, including the initial layout, unsupported LSP request value, missing portal, and focus/input-script mistakes. They are not all passing snapshots. The linked evidence above and final smoke are the acceptance set. The initial LSP request was corrected to the core's `auto` protocol; later actual definition/reference windows passed. The latest code also preserves review anchors when entering/leaving frozen navigation sources and searches/jumps within the visible navigation source.

## Limits and remaining differences

- Linux evidence does **not** validate macOS 15+ menus, IME, native dialogs, packaging, ad-hoc signing, notarization or installation. A macOS bundle script and CI job exist but were not executed here. No hosted CI, release or merge is claimed; native work is delivered on the migration branch.
- Real LSP window coverage is Python; the real-server core suite additionally exercised Go, TypeScript, JavaScript and Rust. Server installations are external tools, not bundled app dependencies.
- No controlled 100 ms startup/input or 33 ms frame-time gate was measured. Software-rendered large-diff interaction is functional evidence, not a latency benchmark. Generation guards reject stale results; already-running CPU diff work is bounded rather than instantly terminated.
- Native settings now include normal form controls, theme search/previews, import/removal, per-server enable/executable and Python interpreter. Native Save/Cancel is an explicit transaction; the original appearance form applied some choices immediately. Advanced custom-server JSON is optional.
- All language grammars offered by the pinned GPUI component are enabled. The former Shiki/TextMate engine and the new Tree-sitter engine do not have identical language coverage or selectors. Unsupported languages display plain text. Theme mapping details are in [the ledger](NATIVE-MIGRATION.md#theme-rendering-differences).
- Navigation history remains backward-only and in-memory, matching the original hook; review/session state and vertical/horizontal viewport positions persist. The final window suites verify quick-open candidate visibility, horizontal restoration after layout and second shutdown, frozen-source search/back anchors, special Git file kinds and explicit large-source loading.
- Linux/macOS active dependency graphs have no Tauri app runtime, Wry, WebKit or WebView. `Cargo.lock` still contains `tauri-winrt-notification`, a Windows-only notification helper through upstream `notify-rust`; it is not the Tauri app framework. [Exact dependency chain](native-validation/windows-notification-dependency.txt). Windows desktop is not accepted.

The old application is removed and the Linux workflow checks pass. This is **not a claim of identical syntax pixels or macOS release readiness**. The acceptance ledger distinguishes executed Linux checks, automated core invariants and platform gates.

## Follow-up regression evidence

The final forms and workflows were exercised again after fixing rapid-switch last-request loss, horizontal restoration after layout, quick-open repaint/scroll and hidden dialog errors. Source display preparation and frozen-navigation materialization run on background workers.

- [Appearance and theme search](native-validation/smoke/13-theme-search.png), [viewer](native-validation/smoke/10-settings-viewer.png), [navigation settings](native-validation/smoke/11-settings-navigation.png), [Git settings](native-validation/smoke/12-settings-git.png).
- [Keyboard candidate beyond the viewport](native-validation/smoke/14-keyboard-candidate-visible.png), [horizontal position restored](native-validation/smoke/16-horizontal-restored.png), [explicit 22,000-line source](native-validation/smoke/18-explicit-large-source.png).
- [Feedback hidden](native-validation/desktop/07-feedback-hidden.png), [confirmed queue clear](native-validation/desktop/08-clear-confirmed.png).

Each reported number counts named harness assertions, including lifecycle setup; it is not a count of independent product features. Initial numeric screenshots/video remain real earlier native runs; current subdirectory results are the repeatable acceptance set. No failed-run screenshot is counted as a pass.

Repeated runs also exposed OCR harness ambiguity between theme filter text and the actual card, plus footer segmentation under concurrent compilation. The harness now uses verified theme-button hit targets, a footer crop and one OCR worker. The failed screenshots visibly showed successful native LSP results; they were not counted as passing runs. All four final suites then passed.
