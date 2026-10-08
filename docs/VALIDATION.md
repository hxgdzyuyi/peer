# Validation record — 2026-10-08 JST

Reference machine: Apple M2, macOS 26.5.2 (25F84 / Darwin 25.5.0). The native app uses WKWebView with a configured 1440 × 940 window. The test-only configuration keeps its window above other windows to avoid occlusion-related rendering pauses; production windows retain normal behavior. Development tools: Rust 1.88 and Node.js 26.7.

## Results

- 15 Rust integration/contract tests passed.
- 10 frontend unit/contract tests passed.
- 12 WebdriverIO UI tests and the separate memory workload passed against the actual Tauri macOS application.
- TypeScript production compilation and Rust formatting checks passed.
- Production dependency audit: zero reported vulnerabilities. This result excludes development/test tooling.
- Universal executable verified to contain both `arm64` and `x86_64` slices.
- The local ad-hoc app signature passes `codesign --verify --deep --strict`.
- The DMG passes `hdiutil verify`.
- Production JavaScript and the native executable exclude the WebdriverIO bridge and embedded server. Seven attribution notices are included in the app's resources.

## Viewer benchmark

The fixture starts with 100 changed TypeScript files and 10,000 changed lines. The refresh test adds two more lines before the benchmark. Whitespace markers are enabled and the Dracula theme is active. Syntax highlighting and diff parsing use local workers.

| Measurement              | Recorded p95 |   Target |
| ------------------------ | -----------: | -------: |
| Scrolling frame interval |        21 ms |  < 33 ms |
| Warm file navigation     |        67 ms | < 100 ms |

Raw measurements are in [benchmark-macos.json](benchmark-macos.json). The test samples 120 animation frames and 100 navigation operations, discarding two warm-up samples from each. Earlier runs used 20 navigation operations, whose retained 18 samples made p95 equal the maximum; the larger sample separates the percentile from isolated stalls. Navigation includes two animation frames after clicking between cached files. This is a local measurement, not a guarantee for every repository or device.

An earlier run during concurrent universal compilation recorded 48 ms and 100 ms respectively. Earlier idle builds recorded 31/68 ms, 29/64 ms, and 27/67 ms. During development of scroll-follow navigation, the shorter navigation samples occasionally reached 102–130 ms. An intermediate layout-fix run recorded 34 ms for scrolling, above the target. The latest run above uses the larger sample and visible test window, keeps the Changes item array stable as the active file changes, includes the current viewer fixes, and uses the default 12 px code font. Measurements vary between runs. The harness records an explicit unmeasured result if the host suspends animation frames rather than substituting timer measurements.

## Functional coverage

Core tests cover staged/unstaged/net/merge-base comparisons; unborn HEAD; renames, deletion, Unicode/control-character paths; ignored files; symlink targets; binary files; submodule updates/deletion; conflicts; large-file deferral; CRLF and missing final newlines; branch/worktree queue isolation; persistence; stale reads/outdated feedback; Reviewed reset; one-based old/new export coordinates; EditorConfig; JSONC validation; duplicate theme names; and imported-theme persistence/removal.

Native tests cover opening the workload, rendering syntax colors, file and line comments, draft preservation through a theme switch, JSON clipboard export, whitespace text preservation, split/unified layouts, jumping beyond the viewport, Reviewed persistence, refresh/outdated feedback, Code/Changes queue isolation, file navigation, and scrolling.

Concise feedback export is checked against the actual native clipboard for both a file comment and a line comment at line 80. The JSON array includes only `file`, optional `line`, and `comment`, with `side: "old"` and `outdated: true` when applicable. Core tests also cover renamed paths, old-side coordinates, removed files, JSON escaping, separate Code/Changes queues, and preservation of rich anchors in storage. Save JSON uses the same core exporter. Clipboard read permission is enabled only in the test configuration. The first native run for this change hit horizontal-scroll and view-switch timeouts; the unchanged build passed all 12 tests on rerun.

Horizontal-overflow coverage resizes the native window to 950 × 740 and checks unified diffs, both split columns, and Code. It verifies that the bottom scrollbar stays inside the viewport, reaches clipped line endings without changing source text, mirrors scrolling over the code, and updates on file selection and collapse/expand. Home/End use standard DOM keyboard events because the embedded driver sends raw WebDriver private-use characters for those keys. The scrollbar reserves a stable height and measures only the active file's rendered columns; macOS scrollbar styling keeps its thumb visible. Pending scrollbar input takes precedence when a layout frame runs before its native scroll event.

Plain-text coverage reproduces the previous `VirtualizedFile.render: rendered a different file than its prepared layout` crash by collapsing a loaded `.txt` file. It then verifies collapse/expand, saved feedback, light/dark theme changes, and refreshed source content with outdated feedback. Unchanged source objects are retained when only the surrounding review item changes, and loading/metadata summaries have separate renderer cache keys. Every desktop test also checks for the fatal error screen and renderer error panels.

Scroll-follow coverage verifies downward and upward scrolling across file boundaries, the navigator's current-file highlight and automatic row visibility, expansion of collapsed parent folders, retained keyboard focus, persisted current-file selection, and continued click-to-file navigation, including reselecting the current file without leaving a delayed jump behind. The current file is found from the virtualizer's logical offsets without scanning source lines or rebuilding the Changes item array on each selection update.

Font-size coverage verifies immediate text/line-height updates in Code and Changes, retained drafts and feedback, persistence after webview reload, and restoration to the default. Core tests cover loading older settings without a font size, persistence across service restarts, and rejection of invalid sizes. Title-bar tests exercise Tauri's real drag handler on the bar and app name, exclude the Settings button, and verify the native drag command is permitted. Synthetic WebDriver events do not verify physical movement by the macOS window manager.

## Memory measurements

The separate memory fixture contains 100 TypeScript files with 1,000 lines each and 10,000 changed lines in total. It visits every Changes file twice, refreshes, visits 100 Code files, and returns to Changes. Both measurements use the same Mac and a Tauri debug build with the embedded driver. No compilation runs concurrently. Samples allow 1.5 seconds for pending work; they do not force garbage collection. Navigation retries a jump while adjacent asynchronous file layouts settle, so this workload does not measure navigation latency.

`launchctl print pid/<test-app-pid>` identifies that app's active WebContent service; `footprint` records physical footprint, including compressed memory. RSS would undercount compressed pages. These numbers cover the WebContent process, not GPU/network helpers or total system memory. The Rust process remained around 27–39 MiB in the optimized run.

| WebContent measurement            |    Before |   After |
| --------------------------------- | --------: | ------: |
| Startup                           |   220 MiB | 257 MiB |
| After first 100-file Changes pass |   873 MiB | 416 MiB |
| After second Changes pass         |   934 MiB | 607 MiB |
| After refresh                     | 1,063 MiB | 695 MiB |
| Peak through refresh              | 1,299 MiB | 979 MiB |

The second-pass sample fell by 35%, and the peak through refresh fell by 25%. Startup increased in this run. Raw records: [baseline](memory-baseline.json), [optimized](memory-macos.json). The baseline captured all Changes samples, then failed the new Code-retention assertion because it retained all 100 source files; no baseline Code memory sample was captured. The optimized workload completed, retaining eight Code sources and at most two highlighted diffs or four highlighted Code files for these 1,000-line files. Its final WebContent sample was 666 MiB after returning to Changes.

The implementation combines the file/diff AST caches under a 24 MiB estimated-data / 12-result budget, uses one WASM/Oniguruma highlighting worker, clears result caches when the snapshot changes, removes duplicate raw strings from full diff models, bounds Code sources to 8 MiB / 8 entries (with the active file pinned), and retires the parser worker after 15 seconds idle. Unit tests cover shared/cyclic size estimation, oversized entries, combined cache budgets, LRU/pinning, exact whitespace and full-context search coordinates after source compaction, and parser lifetime with pending requests.

The native eviction regression keeps a comment draft open while visiting 20 other Code files, verifies the eight-file cache bound, saves the original comment, and reopens its evicted file to check source text and the inline comment. The full native suite still records 21 ms p95 scroll intervals and 67 ms warm navigation after these changes.

This is a first memory reduction, not a fixed process-memory ceiling or proof of a plateau. Changes still retains full diff source/layout models for the active snapshot to preserve context expansion and scroll geometry. WebKit also retains allocator pages and highlighting grammar state; the measured process remains much larger than the explicit cache budget. Larger repositories and longer sessions need further profiling.

## Release boundaries

The project and application are now named **Peer**. The npm package and native executable are `peer`, the Rust library package is `peer-core`, and Mac artifacts are `Peer.app` and `Peer_0.1.0_universal.dmg`. Frontend/core tests and the 12-test native UI suite passed after the rename. The app retains the original `dev.repoinspector.app` identifier and storage location for existing reviews, preferences, and themes; `PEER_DATA_DIR` is the new override, with the previous override retained as a fallback.

The delivered `.app` and `.dmg` are development artifacts with a local ad-hoc signature. They are not Apple Developer ID signed or notarized. The release workflow accepts certificate/signing/notarization secrets; development builds require none.

Windows/Linux core-test jobs are configured in CI but were not executed from this macOS workspace. The Intel executable was cross-compiled and packaged, not tested on an Intel Mac. Older macOS versions were not exercised here.

To reproduce the local package without interactive Finder customization:

```sh
CI=true APPLE_SIGNING_IDENTITY=- npm run bundle -- --target universal-apple-darwin
```
