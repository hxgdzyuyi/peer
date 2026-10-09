# Validation record — 2026-10-08 JST

Reference machine: Apple M2, macOS 26.5.2 (25F84 / Darwin 25.5.0). The native app uses WKWebView with a configured 1440 × 940 window. The test-only configuration keeps its window above other windows to avoid occlusion-related rendering pauses; production windows retain normal behavior. Development tools: Rust 1.88 and Node.js 26.7.

## Repository location repair and removal — 2026-10-09 JST

The repository dropdown now offers explicit Update location and Remove from list actions, including for the active repository and missing paths. Relocation validates the new Git root, migrates the saved session, and retains stable review-queue identity across renames and restarts. Reusing a moved repository's former directory receives a distinct identity. Removal forgets the list entry without deleting files or review data.

`cargo test -p peer-core --locked` passed 36 tests, with two optional real-language-server tests ignored. `npm test` passed all 24 frontend/harness tests. Added core regressions for session/comment/Reviewed retention after relocation and restart, reused-path isolation, and invalid/conflicting destinations leaving saved data untouched.

The native test app built successfully using `CARGO_TARGET_DIR=/tmp/peer-repository-lifecycle-target`; the previous target directory contained generated permission paths referring to the checkout's old name. All four existing repository-switcher desktop tests passed. A temporary native WKWebView smoke scenario renamed a real Git directory, rejected a missing replacement, cancelled the dialog, repaired the path, verified its name and comment after reload, removed a deleted inactive entry, removed the active entry without deleting its files, and verified the empty list after reload. Screenshots were inspected for the updated dropdown and location dialog. The temporary scenario was removed after verification; the full desktop suite was not rerun.

The final smoke run also verified Right-arrow focus on the location action. Activation used the focused button's DOM click: the embedded driver's Enter implementation dispatches synthetic key events without a button's native default activation.

## Dead-code cleanup — 2026-10-09 JST

Removed the unused `Store::directory` getter, an unread navigation-ref field, an always-hidden Reviewed icon, unused imports, redundant SVG styles, and unused benchmark state/control flow. Removed one duplicate parser whitespace test from `themes.test.ts`; `loaded-file.test.ts` retains exact source round-trip coverage for tabs, trailing spaces, CRLF, and missing final newlines. All desktop scenarios, Rust tests, and optional integration workloads remain.

The strict TypeScript unused-local/parameter check and all 25 frontend/harness tests passed. `cargo test -p peer-core --locked` passed all 35 regular tests; the optional real-language-server test remained ignored. `npm run build:test-app` passed. The full native suite passed all 34 tests across five spec files without skips in performance report mode. Inspected screenshots of the Reviewed control, repository switcher, commit history, and collapsed-file Back restoration. The benchmark measured 18 ms scrolling p95 and 66 ms warm-navigation p95, within the unchanged targets. The separate memory workload was not rerun.

## Desktop collapse and navigation readiness — 2026-10-09 JST

Investigated the two desktop failures from [run 37903620759](https://github.com/lantern24/peer/actions/runs/37903620759). The narrow-window check now uses the existing in-place click helper for collapse/expand and verifies that the intended file remains active. The embedded driver's normal click centers the element and focuses it, which can change the active file in the virtualized viewer before checking its horizontal scrollbar. The CI screenshot for this case is captured after the test's `finally` block restores the window and selects the first file, so it does not establish the active file at the original timeout.

The Code-history scenario now waits for the new snapshot to finish opening, verifies the collapsed state before navigating away, and waits for quick-open selection and Back restoration to finish. Local validation also exposed intermittent scroll-setup and search-transition failures. Runtime tracing confirmed that source expansion can precede scrollbar measurement and that the search scenario interacted with Code while Changes was still opening. The tests now wait for the expanded viewport and usable horizontal range, and reuse the snapshot-readiness check for the Changes transition. No application behavior, assertions, timeouts, or performance thresholds were weakened.

`npm run build:test-app` passed. After removing the temporary diagnostic configurations, the final `PEER_PERFORMANCE_MODE=report npm run test:desktop` run passed all 34 tests across all five spec files in the native WKWebView, with no skips. Screenshots verify the selected collapsed file with its scrollbar hidden and collapsed `call.ts` restored by Back. Earlier diagnostic runs are not substituted for this final uninstrumented result; these test corrections have not yet been rerun on GitHub-hosted CI.

## Language-server shutdown cleanup — 2026-10-09 JST

Language-server shutdown now waits for process exit before releasing the temporary workspace. The child process uses an async mutex so waiting does not block an executor thread. This addresses the Windows cleanup race where the server's current directory could still be in use when `TempDir` attempted deletion. The protocol test now checks the actual child exit status immediately after shutdown and explicitly closes the temporary directory, without a polling grace period. Working-file mode selection uses immutable, platform-specific bindings to avoid the Windows-only `unused_mut` warning while preserving Unix permission handling.

`cargo test -p peer-core --locked` passed all 35 regular tests on macOS, including `sessions_documents_and_idle_retirement_are_bounded`; the optional real-language-server test remained ignored. Rust formatting checks passed. A throwaway runtime probe using the actual LSP client and existing Node fixture verified 20 shutdowns with immediate workspace removal, repeated shutdown, and concurrent shutdown. Five end-to-end navigation queries resolved the expected definition and removed every logged workspace on shutdown. The probe was removed afterward. GitHub Actions run 37903620759 subsequently passed all three core jobs on Windows, macOS, and Linux, verifying the shutdown fix on the originally failing platform.

## Desktop assertion and driver fixes — 2026-10-09 JST

Investigated [run 37892311931](https://github.com/lantern24/peer/actions/runs/37892311931) using its failure screenshots/state and the actual WKWebView app. Selection assertions retained the old current row while asynchronous navigation selected another row; they now wait for `aria-current` on the intended file. History checks read only the first split-diff column, missing the displayed additions; the history fixture now explicitly selects split layout and reads both columns.

The embedded driver's element click calls `scrollIntoView({block: "center"})` before clicking. That setup scroll changes the top visible file and can evict a virtualized header. Scroll-sensitive comment, collapse, and review setup now invokes the real DOM handlers without that forced scroll; sidebar drags still use WebDriver pointer actions. Review-state waits query the current checkbox after renderer updates. Refresh anchoring uses the visible horizontal scrollbar and finds the collapsed fixture by passive scrolling rather than assuming earlier scenarios left it first in the file list. No application behavior, timeouts, or performance targets were changed.

Sidebar persistence checks use WebDriver refresh and require a new document `performance.timeOrigin` before checking restored widths, replacing a timer-triggered reload and custom window marker.

The test-only window sets `backgroundThrottling: "disabled"` (supported on macOS 14+) after active-console runs also recorded suspended WKWebView animation frames. Normal packages retain the default scheduling policy. Performance measurements still require an unlocked desktop, and their targets are unchanged.

Local debugging used `act`'s native macOS executor with the existing `mac-desktop` job, Node.js 22.23.3, and Rust 1.99.0. The workflow installs/selects host toolchains and runs the real Tauri build, not a Linux browser substitute. A locked-session attempt explicitly recorded suspended WKWebView animation frames; functional timeouts from that attempt are not evidence of an active-desktop regression.

The final uninstrumented `act` job succeeded: TypeScript/Vite and the native test build passed, followed by all 34 desktop tests across five spec files with no skips. Refresh anchoring, restored sidebar widths, and split commit-diff screenshots were inspected. The refresh artifact preserved the same visible source text, vertical offset, and horizontal scroll across refresh. This completes the previously pending full desktop checks recorded below; it is a local native run, not a fresh GitHub-hosted or cross-platform core-test result.

| Measurement              | Recorded p95 |   Target |
| ------------------------ | -----------: | -------: |
| Scrolling frame interval |        25 ms |  < 33 ms |
| Warm file navigation     |        68 ms | < 100 ms |

## Apple Silicon releases — 2026-10-09 JST

Mac releases now target only `aarch64-apple-darwin`. The default release command and packaging workflow produce an Apple Silicon `.app`; DMG packaging is optional via `npm run bundle:dmg`. Earlier universal-build measurements below are historical and do not describe the current supported architectures.

## Branch commit history — 2026-10-09 JST

All 35 regular core tests, 26 frontend tests, and seven focused native tests passed. Core coverage includes paginated history pinned to the captured branch head, unborn and detached HEAD, initial and empty commits, merge diffs against the first parent, shallow history with an unavailable parent, historical renames/deletions/binary files, immutable contents despite local edits and staging, correct review anchors, and Git-backed navigation inventories on both sides. The optional real-language-server smoke test remains ignored.

Native tests cover opening and dismissing history, keyboard navigation, loading older commits, rendering the selected commit's actual before/after content, isolated review comments, restoring the selection after reload, returning to local changes, and repository-switcher regressions. History and diff screenshots were inspected. As with existing select tests, the comparison selector uses a DOM change event because the embedded driver's native option clicks do not emit one. These focused runs do not replace the full desktop performance suite.

## GitHub Actions desktop follow-up — 2026-10-09 JST

Inspected the artifacts from [run 37732905157](https://github.com/lantern24/peer/actions/runs/37732905157). Its Apple M1 virtual runner measured 41 ms scrolling p95 and 177 ms warm-navigation p95, exceeding the local reference targets of 33/100 ms. Hosted CI now explicitly uses performance report mode: measurements, targets, and overruns are retained in the uploaded JSON and job summary, while functional assertions remain mandatory. Local runs still enforce both unchanged targets by default; an unmeasured run cannot pass enforcement mode.

The eviction failure screenshot shows a snapshot-expired banner and the comment still in its draft editor, while source text has rendered correctly. The old test's whole-panel text assertion matched the unsaved draft. Review tests now wait for Code/Changes snapshot transitions to finish before interacting with the new view, avoid reopening an already active view, and require the composer to close and a saved feedback card to appear. Source and inline-comment checks have separate failure messages. Failure artifacts retain per-test state, errors, drafts, and comments instead of overwriting the previous failure's diagnostics.

All 26 frontend/harness unit tests and the production TypeScript/Vite build passed. The policy tests verify that 41 ms still fails enforcement, report mode retains overruns, and invalid measurements fail both modes.

## Resizable sidebars follow-up — 2026-10-09 JST

All 22 frontend tests and 29 regular Rust tests passed, along with the TypeScript/Vite/native test build. Layout tests cover bounds, smaller windows, preserving preferred widths, and hiding Feedback. Core checks cover older preference stores, partial defaults, persistence, and rejecting invalid widths. The optional real-language-server smoke test was not rerun for this change.

Three focused native scenarios passed: dragging both dividers, saved widths after reload and hiding/reopening Feedback, draft preservation, keyboard resizing, Escape cancellation, reset, minimum source space in a smaller window, and a shared width between References and Feedback through symbol navigation and Back. Screenshots were inspected at normal and narrow window sizes. The embedded driver emits mouse events rather than pointer events; double-click reset was exercised with a directly dispatched DOM double-click because its double-click action does not emit that event.

Two additional passive-scrolling and refresh-anchoring regression scenarios timed out while rendering was suspended. Native UI automation confirmed that the Mac was locked; these checks remain pending on an unlocked Mac. The three passing focused scenarios do not replace the pending full desktop suite or performance benchmark below.

Rebuilt the universal macOS app and DMG. Both arm64 and x86_64 slices are present; the ad-hoc app signature and DMG checksum verification passed.

## Refresh anchoring follow-up — 2026-10-09 JST

All 19 frontend tests and the TypeScript/Vite/native test build passed. Five focused native scenarios cover passive file highlighting, one-click Code/Changes refresh (including an interrupted snapshot), and viewport restoration. Refresh checks verify both diff layouts after snapshot expiry, inserted lines, changed preceding-file heights, horizontal scroll, collapsed files, unchanged expanded context, and fallback when the viewed diff disappears. The existing symbol-jump/Back check also passes with expanded context, Unicode selection, and both scroll axes. Unit checks cover renamed paths, removed-file fallback, and excluding external source pages from review anchors. These focused checks do not replace the pending full code-navigation suite below.

Rebuilt the universal macOS app and DMG. Both arm64 and x86_64 slices are present; the ad-hoc app signature and DMG checksum verification passed.

## Settings tabs follow-up — 2026-10-09 JST

All 16 frontend tests and the TypeScript/Vite/native test build passed. Three focused native scenarios verified appearance changes with feedback drafts, persisted viewer preferences, and the new settings tabs. Tab checks cover Left/Right and Home/End navigation, a single keyboard tab stop, one visible panel, fixed navigation while panel content scrolls, preserved theme filters and unsaved server JSON across tabs, and Escape dismissal. Home/End events were dispatched directly because the embedded driver's key map omits them. The dark-mode layout was inspected from a native screenshot. These focused checks do not replace the pending full code-navigation suite below.

## Search highlighting follow-up — 2026-10-09 JST

All 16 frontend tests and the TypeScript/Vite/native test build passed. Three focused native scenarios passed, covering Quick Open keyboard selection and matching-text highlights, full-source Find and Back, and exact source highlights in unified/split diffs. The checks include literal punctuation, case-insensitive matching, syntax and whitespace spans, tabs/emoji, expanded context, multiple occurrences on one line, active-match styling, Enter navigation, clearing/closing Find, and sidebar filename highlights. These focused checks do not replace the pending full code-navigation suite below.

## Quick-open keyboard follow-up

All 13 frontend tests and the TypeScript/Vite/native test build passed. The focused native quick-open scenario passed using WebDriver keyboard input: initial search focus, ↑/↓ and Ctrl+P/Ctrl+N selection, wraparound, query preservation, filter reset, empty results, Enter opening the selected file, and Back restoring the previous file. This focused run does not replace the pending full code-navigation suite below.

## Code navigation validation

- 28 regular Rust tests passed (3 protocol/lifecycle unit tests, 7 navigation integration tests, and 18 existing review/contract tests).
- The optional real-server smoke test passed with gopls, typescript-language-server with TypeScript 5.9.3, and rust-analyzer. It exercises Go, TypeScript, JavaScript, and Rust definitions, cross-file resolution, methods, nested local shadowing, and references.
- 13 frontend unit/contract tests passed. Production TypeScript/Vite compilation and Rust formatting checks passed.
- A focused native run passed all seven navigation scenarios during development. The last full-suite attempt recorded 15 passed, 4 failed interaction checks, and an unmeasured/skipped benchmark while WKWebView animation frames were suspended; native UI automation confirmed the Mac was locked. The refresh follow-up above now verifies scrollbar and Back restoration in focused checks. A full desktop rerun and fresh performance benchmark remain pending; the historical performance results below do not validate this change.

The new fixtures distinguish HEAD, index, working files, and merge base, including unchanged, deleted, renamed, and hidden untracked destinations. They verify immutable source reads, races during copying, dependency previews, snapshot-aware package links, rejection of live repository escapes, unchanged repository/index contents, and stale-context rejection. Protocol fixtures exercise Location/LocationLink forms, UTF-16 positions, configuration, unavailable capabilities, timeout/cancellation with late replies, crashes, session/document caps, idle retirement, and cleanup.

Native coverage includes F12, modifier-click, context menus, unified shared-line side selection, split columns, references and Back, full-source search, quick-open, collapsed and expanded context, selection after tabs/emoji/whitespace decoration, both scroll axes, feedback drafts, input/dialog exclusions, and refresh clearing history. No language server starts during ordinary browsing.

## Previous baseline results

- 18 Rust integration/contract tests passed on macOS.
- 10 frontend unit/contract tests passed.
- 13 WebdriverIO UI tests passed against the actual Tauri macOS application. The separate memory workload passed during the earlier memory optimization; it was not rerun for the refresh fix.
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

Refresh coverage verifies that one toolbar click loads updated Code content and added/deleted files, clears a stale-source banner, and recovers from a Git index update injected during the reload. A stale Reviewed action still shows the refresh prompt. Three Unix core regressions cover changes during snapshot discovery, changes during feedback reconciliation, and the three-attempt retry limit with the previous queue retained. Both transient-change regressions failed with the original extra-click error before the fix. The full native suite passed all 13 tests after tightening navigation preconditions in the new test; the first run also encountered the existing horizontal-scroll timeout. This refresh validation recorded 22 ms scroll p95 and 65 ms warm-navigation p95; the retained benchmark dataset above predates this change.

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

The project and application are now named **Peer**. The npm package and native executable are `peer`, the Rust library package is `peer-core`, and the supported Mac artifact is the Apple Silicon `Peer.app`, with an optional arm64 DMG. Frontend/core tests and the 12-test native UI suite passed after the rename. The app retains the original `dev.repoinspector.app` identifier and storage location for existing reviews, preferences, and themes; `PEER_DATA_DIR` is the new override, with the previous override retained as a fallback.

The delivered `.app` and `.dmg` are development artifacts with a local ad-hoc signature. They are not Apple Developer ID signed or notarized. The release workflow accepts certificate/signing/notarization secrets; development builds require none.

Windows/Linux core-test jobs are configured in CI but were not executed from this macOS workspace. Intel Mac releases are no longer supported. Older macOS versions were not exercised here.

To reproduce the local package without interactive Finder customization:

```sh
CI=true APPLE_SIGNING_IDENTITY=- npm run bundle
```
