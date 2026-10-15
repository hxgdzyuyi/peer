# Third-party notices

Peer uses GPUI (`gpui-pre` 0.3.9, Apache-2.0) and Longbridge GPUI Kit 0.7.3
(Apache-2.0). The local component copy retains its upstream license and provenance
in `vendor/gpui-component/`. Its limited Peer changes are documented there.

Bundled JSON theme definitions are the same six themes used by the original
application, extracted from Shiki's theme collection at migration time. Their
native Tree-sitter capture mappings are explicit Peer additions; their
original notices remain under `licenses/` and are copied into macOS bundles.

| Component | Upstream | Notice |
|---|---|---|
| Bundled Lucide icons (GPUI Kit assets) | https://github.com/lucide-icons/lucide | licenses/lucide.txt |
| Shiki themes | https://github.com/shikijs/shiki | licenses/shiki.txt |
| GitHub Light / Dark | https://github.com/primer/github-vscode-theme | licenses/github-theme.txt |
| One Dark Pro | https://github.com/Binaryify/OneDark-Pro | licenses/one-dark-pro.txt |
| Dracula | https://github.com/dracula/visual-studio-code | licenses/dracula.txt |
| Catppuccin Latte / Mocha | https://github.com/catppuccin/vscode | licenses/catppuccin.txt |

Other Rust dependency licenses are recorded in their packages and Cargo manifests.
There is no React or Pierre Diffs runtime. Theme data does not require JavaScript.
