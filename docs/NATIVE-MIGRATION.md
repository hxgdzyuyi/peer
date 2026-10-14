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
