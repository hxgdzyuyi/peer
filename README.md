# Peer — native migration branch

A read-only repository viewer and review workspace. The application is Rust and
GPUI, using Longbridge's `gpui-component` Diff surface through `gpui-kit`.
`crates/repo-core` remains independent of the GUI. There is no JavaScript frontend,
Tauri command layer, embedded browser, or WebView in the startup chain.

**Migration status:** native implementation runs and has repeated Linux window regression evidence;
the Linux workflow checklist is exercised, with theme-rendering differences and macOS acceptance remaining. See [the acceptance ledger](docs/NATIVE-MIGRATION.md)
and [validation results and differences](docs/VALIDATION.md). Do not infer feature parity or
macOS acceptance from compilation. The original application is available at Git
commit `72e854cd1a3c5bcb6d2e6de539cc6caa90c49959`.

## Run

Install the Rust toolchain specified in `rust-toolchain.toml` and Git. On macOS,
install Xcode Command Line Tools; deployment requires macOS 15+. On Linux, GPUI
requires a graphical session and platform build libraries (Clang, Fontconfig,
Wayland/X11, XKB, ALSA, Vulkan, OpenSSL and pkg-config development packages).

```sh
cargo run --locked -p peer -- /path/to/repository
cargo test --locked -p peer-core
cargo test --locked -p peer
# macOS only, local ad-hoc signed app bundle:
scripts/bundle-macos.sh
```

Node.js is only needed to run the core's existing fake-LSP tests; it is not needed
to launch or package the app. The immutable GPUI Kit revision is pinned in the
native crate's manifest and all transitive dependencies are in `Cargo.lock`.

Use Open/Browse to select a repository; select a comparison, then a file. The
viewer supports unified/split differences and context expansion. The gutter plus
opens line feedback; File scope attaches a file comment. The feedback panel
supports editing/deleting, Reviewed, copying and saving export. Repository source
is never changed. Settings provides native appearance/theme search and previews, viewer controls, Git path,
and language-server executable/enable/interpreter controls. Save applies the settings;
Cancel discards the form. Advanced custom language-server JSON remains available. LSP servers must be installed separately.

Storage retains `dev.repoinspector.app`, existing `state.json`, atomic replacement
and file locking. `PEER_DATA_DIR` overrides the data directory; legacy
`REPO_INSPECTOR_DATA_DIR` is supported with lower precedence. Corrupt data is
reported and preserved. Repository snapshots, feedback fingerprints, stale-source
checks and isolated LSP workspaces remain owned by `peer-core`.

Licensed under [Apache-2.0](LICENSE). See [third-party notices](THIRD_PARTY_NOTICES.md).
