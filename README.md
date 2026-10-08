# Peer

A local, read-only repository viewer and review workspace. Built with Tauri 2, React, TypeScript, and an independent Rust core. Git is required on the user's machine; Node.js and Rust are development dependencies only.

Licensed under [Apache-2.0](LICENSE). Bundled dependencies and themes retain their [third-party notices](THIRD_PARTY_NOTICES.md).

## Run and build

Requirements: Node.js 22+, Rust 1.88+, Git, and [Tauri's platform prerequisites](https://v2.tauri.app/start/prerequisites/). On macOS, install Xcode Command Line Tools.

```sh
npm ci
npm run desktop
```

Production packages (without the test driver):

```sh
npm run bundle
# Both Apple Silicon and Intel in one .app and .dmg:
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run bundle -- --target universal-apple-darwin
```

Artifacts are written to `target/[target-triple/]release/bundle`, including `macos/Peer.app` and `dmg/Peer_0.1.0_universal.dmg` for the universal Mac build. The executable is named `peer`. Development builds need no signing credentials. For distribution, configure Tauri's `APPLE_SIGNING_IDENTITY`, certificate/password, and Apple notarization credentials. `.github/workflows/macos-release.yml` packages a universal application and reads these from GitHub Actions secrets; unsigned builds are possible without them. macOS minimum version is 13.

## Review workflow

Open a repository with the folder button or ⌘/Ctrl O. The first opening shows Changes if any exist, otherwise Code. The app restores the last repository, comparison, selected file, and scroll position. Refresh with the toolbar or ⌘/Ctrl R after your agent edits files.

- **Code:** existing tracked files plus non-ignored untracked files in the working directory.
- **All local changes:** HEAD against working files, combining staged and unstaged edits.
- **Staged:** HEAD against the index. **Unstaged:** index against working files.
- **Compare to branch:** the merge base of that branch and HEAD against working files. This includes committed branch changes and local edits. No automatic fetch.

Filter filenames in the sidebar, use ⌘/Ctrl P for quick-open, ⌘/Ctrl F for in-file search, and ⌘/Ctrl G to jump to a line. Switch unified/split layouts or toggle whitespace markers in the viewer toolbar. Source tabs, spaces, CRLF, and trailing whitespace are retained. EditorConfig supplies tab width, falling back to the width in Settings.

As you scroll Changes, the navigator highlights the file at the top of the diff viewport and keeps its row visible, expanding parent folders as needed. Keyboard focus and your filename filter are preserved.

Settings → Viewer → Code font size adjusts Code and Changes from 10–32 px and is saved across restarts. Drag the top title bar, including the app name, to move the window.

Use the gutter **+** to comment on a line, or a file header's comment button for a file comment. Comments can be edited, deleted individually, or cleared from the active queue. Changes files can be marked Reviewed. Feedback is stored per worktree and branch, with distinct Code and comparison queues.

Refresh marks comments outdated when their file fingerprint changes, and resets Reviewed for changed files. Original anchors and context remain intact, including feedback on files that disappear. Drafts survive theme and view switching during the session; only saved comments survive application restarts.

**Copy comments** copies a concise JSON array from the active queue:

```json
[
  {
    "file": "src/example.ts",
    "line": 42,
    "comment": "Handle an empty result here."
  },
  { "file": "README.md", "comment": "Document the setup steps." }
]
```

File comments omit `line`. Line numbers are one-based; diff comments on the old side additionally include `"side": "old"`. Outdated comments are included with `"outdated": true`, retaining their original location. Copying does not clear feedback. Save JSON and the selectable clipboard fallback use the same concise format.

## Themes

Settings → Appearance includes System/Light/Dark modes, independent light/dark selections, searchable previews, and import/removal. GitHub Light/Dark, One Dark Pro, Dracula, and Catppuccin Latte/Mocha are bundled locally.

Import a self-contained VS Code `.json` or `.jsonc` theme. Validated definitions are copied into the application-data store, with IDs independent of display names. Grammar token colors and supported editor/diff/sidebar/panel/list/input/button/selection/focus colors apply across the app. Missing UI colors use light/dark defaults. Theme changes do not read Git or alter source fingerprints.

External `include` and token-color file references are rejected. Semantic tokens, icon themes, VSIX installation, and online theme installation are outside v1. Native menus and file dialogs follow the OS appearance. Theme attribution and license texts are retained in `licenses/` and bundled as resources.

## Architecture and storage

`crates/repo-core` implements Git discovery, comparisons, snapshot validation, comment/review services, export, preferences, and theme validation. It has no dependency on Tauri. `src-tauri` exposes typed commands and native menus/dialogs/clipboard. `src/Viewer.tsx` adapts `@pierre/diffs` with virtualized file/line rendering, a diff worker, and a Shiki worker pool. Syntax grammars and themes are packaged locally.

The viewer uses one Shiki worker with the Oniguruma/WASM engine. File and diff highlighting share a 24 MiB estimated retained-data budget and a 12-result limit; refresh and view/repository switches clear cached results. Code source uses an 8 MiB / 8-file LRU, retaining the active file even if it exceeds the budget. Diff models retain full source lines for context expansion and stable scroll layout, without a second copy of the original source strings. The parser worker terminates after 15 seconds idle. These are cache budgets, not a cap on total WebView memory: active renderers, diff models, syntax grammars, and the JavaScript heap add overhead.

Peer retains the original `dev.repoinspector.app` application identifier and OS-specific application-data directory so existing RepoInspector settings, reviews, and imported themes remain available without migration. A versioned `state.json` contains preferences, queues, and normalized imported themes. Atomic file replacement, a process mutex, and an OS file lock serialize writes. Invalid saved data produces an error and is preserved. `PEER_DATA_DIR` may override the store location for testing; the legacy `REPO_INSPECTOR_DATA_DIR` override is also accepted, with `PEER_DATA_DIR` taking precedence.

Git runs asynchronously with argument arrays, NUL-delimited path metadata, disabled external diff/textconv, and optional locks disabled. The app never edits, stages, commits, fetches, or applies repository changes. It tries PATH and common installed Git locations; Settings accepts an explicit executable path.

Snapshot IDs and source fingerprints prevent feedback from attaching to a different version. Saved comments retain their original anchors, Git revisions, timestamps, and code context internally. Copying or saving feedback exports only the relative filename, line number when applicable, and comment text, plus old-side/outdated markers when needed. Renamed files use their current comparison path.

A future MCP adapter can instantiate the Rust `Inspector` against the same store. MCP transport, browser distribution, filesystem watching, history browsing, repository-wide search, symbol navigation, image comparison, and editing are not included.

## Validation

See [the validation record](docs/VALIDATION.md) for native test results, benchmark measurements, and release boundaries.

```sh
npm test
cargo test -p peer-core --locked
npm run build:test-app
npm run test:desktop
# macOS memory workload (isolated app; includes compressed WebKit memory)
PEER_MEMORY_TEST=1 npm run test:desktop
```

Desktop tests launch the actual app through WebdriverIO's embedded Tauri driver. They create a temporary Git repository and isolated data store. Test plugins and the frontend bridge are feature/environment gated and excluded from normal packages. Logs, screenshots, and performance measurements go into `test-results/`.

The benchmark uses 100 changed TypeScript files and 10,000 changed lines. Targets: warm navigation below 100 ms and p95 scrolling frame intervals below 33 ms. Results include the reference CPU/OS. Files above 1 MiB or 20,000 lines require explicit loading; highlighting is bounded on long lines. A 64 MiB inspection limit protects the viewer from very large payloads.

CI runs Rust core tests on macOS, Windows, and Linux, frontend tests/builds, and the macOS desktop suite. Windows/Linux desktop releases remain future work; a CI configuration is not evidence of a completed remote run.
