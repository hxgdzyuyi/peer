Vendored from longbridge/gpui-kit v0.7.3 at 67265a36e260ffe03c5aaebdcd8d654dbf892eab.
Apache-2.0. Local changes expose source-coordinate hit testing, retained viewport, tab/whitespace presentation and search highlights needed by Peer. No webview or shell layer is included.

The file-tree follow-up adds `DiffEvent::ViewportFileChanged`. It reads the final
virtual-list offset in a deferred callback after the list's mutable scroll borrow
ends, so Peer can synchronize the active tree/feedback file without scrolling the
Diff back. Source scrolling remains handled by the original native list.


The review-safety follow-up adds `DiffCheckpoint` to retain expansion, file folds,
source-byte/gutter selection and viewport during navigation. Restored byte ranges
use native selection colors and exact source copying; replacement selection gestures
clear them. `select_source_bytes` restores an LSP range after explicit large-source
loading, and `counterpart_position` maps only unchanged lines between versions.
See `docs/NATIVE-PARITY-FOLLOWUP.md` for actual window and clipboard regressions.
