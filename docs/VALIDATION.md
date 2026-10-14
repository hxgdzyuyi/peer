# GPUI native validation — 2026-10-14

Baseline: `72e854cd1a3c5bcb6d2e6de539cc6caa90c49959`, independently checked against upstream main. Implementation branch: `native/gpui-migration`. Historical WebView results are isolated in [VALIDATION-WEBVIEW-BASELINE.md](VALIDATION-WEBVIEW-BASELINE.md); none count as native acceptance.

## Executed checks

| Check | Result and evidence |
|---|---|
| Core suite | 48 passed; two opt-in real-server suites skipped by the standard invocation. [Log](native-validation/core-tests.log) |
| Native adapter and directory tests | 5 passed: absent sides, rename, Unicode filename, CRLF, unterminated lines, deferred/binary/empty content, 100-file actual native parser, environment-variable precedence. [Log](native-validation/native-tests.log) |
| Real Python LSP | Pyright 1.1.407: opt-in definition/reference/import/stub/method regression passed independently. [Log](native-validation/python-lsp.log) |
| Native executable | Rust 1.95.0, locked dependencies, Linux debug binary successfully linked. [Log](native-validation/link.log) |
| Formatting | `cargo fmt --all -- --check` passed. [Log](native-validation/format.log) |
| Actual native window regression | Repeated Xvfb/llvmpipe runs, final reproducible smoke: 23 state/lifecycle assertions passed, with real mouse/keyboard input. [Script](../tests/native_smoke.py), [result](native-validation/smoke/result.json), [screenshots](native-validation/smoke/) |
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

On a Linux graphical development host with the GPUI platform development libraries, Git, Rust 1.95, Node 22 (core fake-LSP tests only), Xvfb, xdotool, ImageMagick and Mesa Vulkan:

```sh
cargo test --locked -p peer-core
cargo test --locked -p peer
cargo build --locked -p peer
cargo fmt --all -- --check
xvfb-run -a -s '-screen 0 1440x900x24' \
  python3 tests/native_smoke.py target/debug/peer test-results/native-smoke
```

The harness creates disposable repositories and a dedicated PEER_DATA_DIR. It sends actual input, verifies the on-disk state and records screenshots. It must run in a disposable display, never a colleague's active desktop. UI screenshots require visual inspection; persisted-state assertions alone do not prove pixels or hit targets.

This cloud host is Debian 13 with no permitted system package writes. Official Debian packages were downloaded/extracted to `/workspace/peer-native-sdk`; its activate script configures headers, libraries, Clang and pkg-config. Rust lives under `/workspace/.toolchains`. Xvfb used Mesa llvmpipe software Vulkan. Folder/export/import tests used an isolated D-Bus session and locally extracted xdg-desktop-portal/GTK. No credentials or security settings were added or changed. The initial apt install failed with OS permission denied; local extraction resolved the build dependency need.

The numeric screenshots retain intermediate investigation, including the initial layout, unsupported LSP request value, missing portal, and focus/input-script mistakes. They are not all passing snapshots. The linked evidence above and final smoke are the acceptance set. The initial LSP request was corrected to the core's `auto` protocol; later actual definition/reference windows passed. The latest code also preserves review anchors when entering/leaving frozen navigation sources and searches/jumps within the visible navigation source.

## Limits and remaining differences

- Linux evidence does **not** validate macOS 15+ menus, IME, native dialogs, packaging, ad-hoc signing, notarization or installation. A macOS bundle script and CI job exist but were not executed here. No hosted CI, release, merge or remote push is claimed.
- Real LSP window coverage is Python. The unchanged core's fake-server regression covers the other presets; the combined opt-in Go/TypeScript/JavaScript/Rust suite was not run for this migration. Earlier macOS/WebView real-server evidence is historical only.
- No controlled 100 ms startup/input or 33 ms frame-time gate was measured. Software-rendered large-diff interaction is functional evidence, not a latency benchmark. Cancellation/generation guards prevent stale application of results, but do not instantly terminate already-running CPU diff calculations.
- Settings currently uses a native JSON editor backed by the existing validated preference model. The original individual form controls, theme search and previews have not been recreated. Bundled themes/import/removal are implemented; TextMate theme colors are approximated into Tree-sitter categories rather than guaranteeing identical token appearance.
- File tree filtering and keyboard quick-open are implemented; keyboard selection scrolls the candidate into view. Navigation history is backward-only and in-memory, as distinct from persisted review/session state.
- Recent-root relocation/removal and special Git file kinds are implemented and have core/adapter coverage, but lack dedicated native-window assertions. They remain explicit UI acceptance gaps, not inferred passes.
- The final standard smoke rechecks primary review/comparison/error recovery paths. Horizontal-scroll restore, quick-open auto-scroll, macOS Edit menu, and the final frozen-source search/back-anchor refinements compile and are code-reviewed, but do not have dedicated automated native-window assertions.
- Linux/macOS active dependency graphs have no Tauri app runtime, Wry, WebKit or WebView. `Cargo.lock` still contains `tauri-winrt-notification`, a Windows-only notification helper through upstream `notify-rust`; it is not the Tauri app framework. [Exact dependency chain](native-validation/windows-notification-dependency.txt). Windows desktop is not accepted.

This is a substantive, running native migration with the old application removed, **not a claim of 100% original UI parity or macOS release readiness**. Remaining acceptance gaps are recorded so follow-up work cannot mistake compilation for platform validation.
