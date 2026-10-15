# File tree follow-up: db2e860 versus native repair

The later independent acceptance found two All-files review-target errors that
these historical results missed. See [the identity follow-up](NATIVE-IDENTITY-FOLLOWUP.md)
for the repair, independent center-pixel assertions, and new execution evidence.

The user's video review identified real migration defects. They are implementation
omissions, not GPUI limitations. Baseline source: `72e854c:src/FileTree.tsx` and
`Viewer.tsx`; first delivered native source: `db2e860:crates/peer-native/src/workspace.rs`.

## Root causes and repair

| Before | Repair |
|---|---|
| Full-width centered Button label concatenated spaces, full status word and basename | Native fixed-height rows with separate left-aligned disclosure, folder/file SVG, flexible ellipsized name and right-aligned status |
| Space indentation changed with text width | Structural directory projection and explicit 14 px indentation; directories stay contiguous even if input files are interleaved |
| Missing folder/file/comment icons and no original sidebar caption/count | Bundled Lucide assets registered on the application, folder/file/comment/Reviewed icons, caption/count/read-only footer |
| SVG asset source was never registered on the native application | `application().with_assets(AllAssets)`; real screenshots confirm visible SVGs, including Diff disclosure arrows |
| Only search-box Up/Down handled navigation | Focused tree supports Up/Down, Left parent/collapse, Right child/expand, Enter, Home/End; native selection previews files and scrolls the tree |
| Only quick-open candidate scroll was wired | File selection, Back and external multi-file activation expand ancestors and reveal the selected row without moving focus |
| Ctrl+P only focused the sidebar filter | Independent native Go to file overlay, 100-result window, full paths/highlights, keyboard candidate scrolling, Enter selection and Escape cancellation |
| Filter still showed hierarchy and basename only, without match highlighting | Flat full-path results as in the original, UTF-8-safe case-insensitive highlight, full-path tooltip; ordinary directory collapse state survives filtering |
| Multi-file scroll did not track the active file | Native virtual-list scroll event reports the visible file boundary; tree and feedback switch without jumping the Diff back |

Longbridge v0.7.3 already supplies native Tree, ListItem and virtual-list primitives.
The existing TreeState's Left/Right handlers only expand/collapse the current
folder and its events only expose expansion, so directly swapping it in would
not supply the requested parent/child navigation and file-preview integration.
This repair uses its native ListItem and GPUI UniformList, with an explicit
application tree projection/focus controller. No WebView or browser layer is added.

Very deep trees retain exact paths and parent/child relationships. Once indentation
would consume the label area, visual indentation is bounded and `…/` marks the
omitted depth; the tooltip always shows the complete path. This avoids making a
deep file unreachable in a narrow sidebar. Width resizing adjusts that bound.

## Evidence and regression method

`tests/native_tree.py` builds a synthetic 1,009-entry repository with nested
folders, a Unicode/deep path, 1,000 sibling files and several Git statuses. The
optional `PEER_TREE_BASELINE` binary captures the *same fixture* with db2e860;
`PEER_TREE_RECORD=1` records actual new native input through X11/ffmpeg.

Artifacts: [before](native-validation/tree-release/00-db2e860-before.png),
[structural tree](native-validation/tree-release/01-rich-tree.png),
[comment/Reviewed icons](native-validation/tree-release/01b-comment-reviewed-icons.png),
[collapse](native-validation/tree-release/02-collapsed-app.png),
[full path tooltip](native-validation/tree-release/04b-full-path-tooltip.png),
[Back reveal](native-validation/tree-release/06-back-expanded.png),
[1,000th file reveal](native-validation/tree-release/07-last-file-revealed.png).

The harness uses actual mouse/keyboard input, persisted selections, sidebar-only
OCR and screenshots. Whole-window filename OCR is insufficient because the Diff
header may contain a filename even if its tree row is invisible. OCR normalizes
only the observed zero/letter-O ambiguity for numbered fixture filenames.

Native unit coverage adds structural flattening, collapse/filter/reveal preservation,
10,000-file uniqueness and Unicode highlight boundaries. Prior review, settings,
portal and real-LSP window suites are rerun with the sidebar filter's new location.
The rendering was inspected directly; compilation and state assertions alone do
not establish the visual acceptance.

## What the earlier acceptance missed

The earlier broad smoke suite did not establish rich hierarchical tree parity.
Its quick-open checks exercised a filtered list, not a collapsed-ancestor global
file jump or multi-file scroll tracking. Missing SVG asset registration was also
invisible in text/state-only assertions. These were migration omissions and
insufficient acceptance coverage; GPUI was not the limiting factor.

During this follow-up, screenshots caught layout composition mistakes before
acceptance. The repeated suite also caught a stale test coordinate after adding
the sidebar caption: clicking the same file could persist a draft without really
switching files. The corrected regression now asserts the other file's persisted
selection before returning. Native settings tests wait for the rendered tab's
controls before typing. Multi-file wheel tests wait for the actual “files loaded”
state, capture the pre-scroll selected file and require a different selected file
after scrolling; a fixed delay during an optimized build is not equivalent.

Native Go to file intentionally owns a separate query. Selecting its result clears
an old sidebar filter so the selected tree row can be revealed; Escape leaves the
review selection unchanged. Re-selecting the active single file preserves its
viewport and draft instead of unnecessarily reloading the source.

## Final follow-up validation

The optimized release binary passed all 24 native interaction assertions on the
1,009-file fixture. [Actual 68.67-second recording](native-validation/tree-release/tree-interaction.mp4),
[results](native-validation/tree-release/result.json) and
[binary provenance](native-validation/tree-release/provenance.json) are retained.
The video was fully decoded without errors. The independent Go to file overlay,
ancestor expansion and multi-file scroll selection were visually inspected.
The native scrollbar was also dragged from the thousandth file back to the root.

A separate [six-assertion native regression](native-validation/tree-transition/result.json)
covers replacing a tracked file with a same-named directory. Folder and deleted
file rows now have distinct typed identities, preserving independent selection,
collapse and palette reveal. Unit coverage includes this transition too.

The locked workspace test suite passed 57 tests (including 9 native tests); two
opt-in server suites were skipped by this invocation and retain their earlier
real-server validation evidence. Formatting checks also passed.

The broad native suites passed 46 smoke, 29 desktop/settings/real-LSP,
9 navigation and 5 error-recovery assertions. These are assertion counts, not
claims of exhaustive behavioral coverage. Linux native execution does not verify
macOS menus, input methods or packaging; those platform checks remain outstanding.
