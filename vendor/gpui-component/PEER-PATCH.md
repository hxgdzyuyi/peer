Vendored from longbridge/gpui-kit v0.7.3 at 67265a36e260ffe03c5aaebdcd8d654dbf892eab.
Apache-2.0. Local changes expose source-coordinate hit testing, retained viewport, tab/whitespace presentation and search highlights needed by Peer. No webview or shell layer is included.

The file-tree follow-up adds `DiffEvent::ViewportFileChanged`. It reads the final
virtual-list offset in a deferred callback after the list's mutable scroll borrow
ends, so Peer can synchronize the active tree/feedback file without scrolling the
Diff back. Source scrolling remains handled by the original native list.
