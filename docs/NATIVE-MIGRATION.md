# Native migration acceptance ledger

Baseline: upstream `main` verified with `git ls-remote` as
`72e854cd1a3c5bcb6d2e6de539cc6caa90c49959` on 2026-10-14.
Work branch: `native/gpui-migration`. No release, merge, deployment or remote push.
Reference: `longbridge/gpui-kit` v0.7.3,
`67265a36e260ffe03c5aaebdcd8d654dbf892eab` (immutable dependency pin).

The baseline is retained by Git, including its frontend tests. The following is
an acceptance checklist. Executed outcomes and remaining differences are in
[the native validation report](VALIDATION.md); rows below define the full target,
not blanket passing assertions. Evidence is retained in `native-validation/`.

| Area | Baseline contract to compare | Verification required |
|---|---|---|
| Open | Native folder selection, path entry, 12 recent roots, invalid path recovery | Window + core |
| Repository management | Switch, relocate while preserving identity, remove without deleting source/reviews | Window + core |
| Comparison | Code, all, staged, unstaged, merge-base branch + working tree, first-parent commit | Window + frozen fixture |
| History | Paginated commit browser, independent queues, initial and merge commits | Window + core |
| Tree | Hierarchical paths, status, filtering, quick open, keyboard selection, resizable panels | Window |
| Diff | Actual old/new comparison (previously diff.worker.ts), unified/split, full context expansion, original coordinates | Adapter + window |
| Sources | Added/deleted/renamed, binary, conflict, symlink, submodule, empty, no final newline, CRLF | Adapter + core + window |
| Feedback | File/old/new line comments, gutter editor, edit/delete, stale anchors, drafts across switching | Window + core |
| Reviewed | Fingerprint-bound toggle, invalidation after source edits, restart | Window + core |
| Export | Copy/save only filename/line/body and old/outdated markers | Window + core |
| Persistence | dev.repoinspector.app, PEER_DATA_DIR > REPO_INSPECTOR_DATA_DIR, existing state.json | Automated + restart |
| Search | Literal case-insensitive, original Unicode offsets, old/new matches, highlight, next/previous, jump | Automated + window |
| Navigation | Quick open, go-to-line, back history, source/viewport restoration | Window |
| Preferences | System/light/dark, bundled/imported themes, tab width, whitespace, font size, Git executable | Window + core |
| LSP | Definition/references, side-specific frozen sources, UTF-16 positions, result navigation, cancellation | Automated + real server + window |
| Responsiveness | Tokio kept alive; Git, diff CPU, store writes and LSP off UI; generation checks | Review + rapid switching |
| Bounds | Explicit load >1 MiB/20k lines, hard 64 MiB limit, multi-file 10k-line workload | Automated + measured window |
| Errors | Missing Git/root/LSP, stale snapshot, corrupt store, failed save, recover without data loss | Automated + window |
| Native integration | Menus, keyboard, selection/copy, dialogs, close/shutdown, IME | Platform window |
| Distribution | Native-only dependencies/start scripts/CI, macOS 15+ bundle | Dependency audit + macOS |

Linux execution cannot validate macOS menus, input methods, signing or packaging.
A successful `cargo check` never counts as a window test. No AI chat or Git-writing
functionality belongs in this migration.

## Executed comparison against the original

The source references below refer to the retained baseline commit, not files in
this native checkout. “Window” means actual input on the native Linux executable;
“core” and “adapter” mean automated Rust assertions, not a claim of GUI coverage.
Detailed counts, artifacts and reproduction commands are in [VALIDATION.md](VALIDATION.md).

| Original source / behavior | Native implementation | Evidence / outcome |
|---|---|---|
| `RepositoryLocation.tsx`, `RepositorySwitcher.tsx`: browse, recents, relocate/remove | `workspace.rs`: native portal/path entry, recent identity preservation | Desktop 29 assertions; missing-root recovery in smoke; Linux passed |
| `App.tsx`, `CommitHistory.tsx`: six modes, branch/commit, history pages | Core snapshots/history plus async native toolbar | Mode screenshots/record + smoke rapid switching; core history tests passed |
| `FileTree.tsx`, `QuickOpen.tsx`, `ResizableWorkspace.tsx`: tree, filter, keyboard, panels | Virtual tree, expanded filtering, keyboard candidate scroll, native resizable panels | Smoke candidate beyond viewport, desktop feedback hide/show; passed |
| `diff.worker.ts`, `parse-diff.ts`, `Viewer.tsx`: old/new text, coordinates, split/unified, context | Core `presentation.rs`, native `adapter.rs`, GPUI Diff | Five adapter/storage tests; actual multi-file 10k-line rendering and mode screenshots; passed |
| `loaded-file.ts`: special files and deferred sources | Core semantics retained; native placeholders + explicit load | Smoke binary/symlink/rename/submodule/conflict/deferred and 22k-line load; passed |
| `Feedback.tsx`: old/new/file anchor, edit/delete, stale context, comment jump | Native comments and gutter extension, saved source text, pending line jump | Smoke old/new/file/edit/delete/stale/restart; desktop jump/context; passed |
| `Feedback.tsx`: draft, discard, clear with confirmation, panel visibility | Per-file/fingerprint draft and explicit clear confirmation | Smoke switch/restart; desktop discard/cancel/confirm/hide/show; passed |
| `App.tsx`, `Feedback.tsx`: Reviewed and export | Existing fingerprint model, clipboard + native save dialog | Smoke invalidation; desktop export byte equality and clear retains Reviewed; passed |
| `api.ts`, old Tauri storage commands: schema1 and data directory | Independent core store + additive native state | Legacy state round trip, precedence test, process restarts; passed |
| `search.tsx`, `whitespace.ts`, `Viewer.tsx`: search/jump/selection/copy | Literal Unicode search, exact source mappings, native Diff selection | Core Unicode/UTF-16/CRLF tests, smoke search, real frozen-source search/jump; passed |
| `useNavigation.ts`, `HorizontalScrollbar.tsx`: Back and viewport | In-memory Back plus persisted review source/vertical/horizontal anchors | Navigation 9 assertions and horizontal second-shutdown equality; passed |
| `Settings.tsx`, `themes.ts`: appearance, theme search/preview/import/remove, font/tab/whitespace/Git | `settings.rs` native form tabs with Save/Cancel; `themes.rs` conversion | Smoke controls/restart and desktop actual import/remove; passed with rendering differences below |
| `NavigationSettings.tsx`: servers, executable, enable, Python interpreter, custom configuration | Native fields/toggles/preset restore; optional advanced JSON | Smoke persisted settings; core validation; passed |
| `navigation.ts`, `NavigationResults.tsx`, `NavigationSource.tsx`: definitions/references, frozen sides, cancel | Core Tokio LSP + generation guards, native result/source view | Real Python window 9 assertions, real Go/TS/JS/Rust core integration, standard fake-server races; passed |
| `refresh.ts`, `memory.ts`: stale completion and large workload | Generation checks, pending-root continuity, background CPU work, virtual lists and budgets | Rapid switching smoke, core stale-save tests, 100-file/10k-line actual render; functional passed; latency gate unmeasured |
| Native dialog failure and unavailable services | Both platform errors and canceled channel errors shown in UI | Dialog-errors 5 assertions; passed |
| Native menu/IME, macOS bundle | GPUI menus and macOS 15+ bundle script | Linux keyboard/clipboard/close passed; macOS integration remains platform-blocked |
| Build/start chain | Rust executable, pinned GPUI Kit, independent core; old frontend deleted | Linux/macOS dependency graph audit and locked build; no Tauri app/WebView; Windows helper caveat in report |

## Theme rendering differences

All Tree-sitter languages included by GPUI component v0.7.3 are enabled. This
replaces the old Shiki grammar engine; languages without a native grammar fall
back to plain text. Exact tokenization and colors cannot be identical across
these different engines.

The six original bundled JSON themes and their licenses remain. Imported VS Code
JSON is retained in the original core model. Editor background/foreground,
selection, line numbers, panel borders, button colors, muted text and Git
addition/deletion colors map to native theme fields. TextMate comment, function,
keyword/storage, string, numeric/constant, type and variable categories map to
Tree-sitter styles, including foreground, italic and bold (normal style resets
are respected). Hierarchical TextMate selectors, semantic-token rules and
underline/strikethrough do not have equivalent mappings in the pinned native
ThemeStyle. Unmapped UI fields retain the selected native light/dark defaults.
This is an explicit rendering difference, not a hidden claim of pixel parity.

## Remaining platform gates

A macOS 15+ host must execute menu, IME, dialog, bundle/install and signing checks.
This Linux cloud environment cannot prove those results. Controlled hardware
performance thresholds and Windows desktop acceptance also remain unmeasured.
No remote release, push or CI run was performed. The Linux workflows above have
executable regression coverage rather than being deferred as implementation work.
