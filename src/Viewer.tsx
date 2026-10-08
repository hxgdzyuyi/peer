import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  CodeView,
  useWorkerPool,
  type CodeViewHandle,
  type CodeViewItem,
  type CodeViewReactOptions,
} from "@pierre/diffs/react";
import {
  ChevronDown,
  ChevronRight,
  FileCode2,
  MessageSquarePlus,
  Plus,
  Check,
  LoaderCircle,
} from "lucide-react";
import type {
  FileEntry,
  Preferences,
  ReviewComment,
  ReviewQueue,
  Snapshot,
} from "./types";
import { decorateWhitespace, whitespaceCSS } from "./whitespace";
import { rendererThemeId, type Theme } from "./themes";
import {
  HorizontalScrollbar,
  type HorizontalScrollbarHandle,
} from "./HorizontalScrollbar";

import type { LoadedFile } from "./loaded-file";
export type { LoadedFile } from "./loaded-file";
export type ViewerHandle = {
  jump: (id: string, line?: number, side?: "old" | "new") => void;
  scrollTop: () => number;
  restore: (top: number) => void;
};
type Props = {
  snapshot: Snapshot;
  selected: string | null;
  loaded: Record<string, LoadedFile>;
  errors: Record<string, string>;
  queue: ReviewQueue;
  preferences: Preferences;
  theme: Theme;
  onLoad: (id: string, force?: boolean) => void;
  onComment: (id: string, line?: number, side?: "old" | "new") => void;
  onReviewed: (id: string, checked: boolean) => void;
  onScroll: (top: number) => void;
  onActiveFileChange: (snapshotId: string, id: string) => void;
  onError: (error: string) => void;
};

const viewerCSS = `
  :host { --diffs-font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', monospace;
    --diffs-bg-addition-override: var(--add-bg); --diffs-bg-deletion-override: var(--remove-bg);
    --diffs-bg-addition-emphasis-override: var(--add-word); --diffs-bg-deletion-emphasis-override: var(--remove-word);
    --diffs-fg-number-override: var(--muted); --diffs-bg-selection-override: var(--selection); }
  [data-diffs-header] { background: var(--surface); border-block: 1px solid var(--border); }
`;

export const Viewer = forwardRef<ViewerHandle, Props>(
  function Viewer(props, outerRef) {
    const {
      snapshot,
      selected,
      loaded,
      errors,
      queue,
      preferences,
      theme,
      onLoad,
      onComment,
      onReviewed,
      onScroll,
      onError,
    } = props;
    const viewer = useRef<CodeViewHandle<ReviewComment, undefined>>(null);
    const horizontalScrollbar = useRef<HorizontalScrollbarHandle>(null);
    const scroll = useRef(0);
    const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
    const [highlighterVersion, setHighlighterVersion] = useState(0);
    const [selectedLine, setSelectedLine] = useState<{
      id: string;
      line: number;
      side?: "old" | "new";
    } | null>(null);
    const pool = useWorkerPool();
    const refs = useRef(props);
    refs.current = props;
    const entries = useMemo(
      () => new Map(snapshot.files.map((file) => [file.id, file])),
      [snapshot],
    );
    const itemCache = useRef(
      new Map<string, { key: string; item: CodeViewItem<ReviewComment> }>(),
    );
    const pendingJump = useRef<{
      id: string;
      line?: number;
      side?: "old" | "new";
    } | null>(null);

    useEffect(() => {
      let active = true;
      pool
        ?.setRenderOptions({
          theme: rendererThemeId(theme),
          tokenizeMaxLineLength: 1500,
          maxLineDiffLength: 1500,
          lineDiffType: "word-alt",
        })
        .then(() => {
          if (active) setHighlighterVersion((v) => v + 1);
        })
        .catch((error) => {
          if (active) onError(String(error));
        });
      return () => {
        active = false;
      };
    }, [pool, theme, onError]);

    const navigate = useCallback(
      (id: string, line?: number, side?: "old" | "new") => {
        const handle = viewer.current;
        if (!handle) return;
        handle.scrollTo({
          type: "item",
          id,
          align: "start",
          behavior: "instant",
        });
        const instance = handle.getInstance();
        // Commit programmatic navigation even when the OS suspends animation frames.
        instance?.render(true);
        if (line !== undefined) {
          const rendered = instance
            ?.getRenderedItems()
            .find((entry) => entry.item.id === id);
          if (rendered?.type === "diff") {
            // Search and comment jumps must also reach lines inside collapsed context.
            for (let i = 0; i < rendered.item.fileDiff.hunks.length; i++)
              rendered.instance.expandHunk(i, "both", Number.MAX_SAFE_INTEGER);
          }
          handle.scrollTo({
            type: "line",
            id,
            lineNumber: line,
            side: side === "old" ? "deletions" : "additions",
            align: "center",
            behavior: "instant",
          });
          handle.setSelectedLines({
            id,
            range: {
              start: line,
              end: line,
              ...(refs.current.snapshot.comparison.view === "changes"
                ? {
                    side:
                      side === "old"
                        ? ("deletions" as const)
                        : ("additions" as const),
                  }
                : {}),
            },
          });
          setSelectedLine({ id, line, side });
          instance?.render(true);
        }
      },
      [],
    );
    const jump = useCallback(
      (id: string, line?: number, side?: "old" | "new") => {
        setCollapsed((current) => {
          if (!current.has(id)) return current;
          const next = new Set(current);
          next.delete(id);
          return next;
        });
        const content = refs.current.loaded[id]?.content;
        const needsRetry =
          !content ||
          content.deferred ||
          collapsed.has(id) ||
          viewer.current?.getInstance()?.getTopForItem(id) === undefined;
        pendingJump.current = needsRetry ? { id, line, side } : null;
        navigate(id, line, side);
        refs.current.onLoad(id);
      },
      [navigate, collapsed],
    );
    useImperativeHandle(
      outerRef,
      () => ({
        jump,
        scrollTop: () => scroll.current,
        restore: (top) =>
          viewer.current?.scrollTo({
            type: "position",
            position: top,
            behavior: "instant",
          }),
      }),
      [jump],
    );

    // Changes always contains the same file list. Moving the active highlight
    // must not rebuild its item array or ask the virtualizer to reconcile it.
    const codeSelection = snapshot.comparison.view === "code" ? selected : null;
    const items = useMemo(() => {
      const files =
        snapshot.comparison.view === "code"
          ? snapshot.files.filter((f) => f.id === codeSelection)
          : snapshot.files;
      const validIds = new Set(files.map((f) => f.id));
      for (const id of itemCache.current.keys())
        if (!validIds.has(id)) itemCache.current.delete(id);
      return files.map((entry): CodeViewItem<ReviewComment> => {
        const value = loaded[entry.id];
        const comments = queue.comments.filter(
          (c) =>
            c.path === entry.path &&
            !c.outdated &&
            c.anchor.fingerprint === value?.content.fingerprint,
        );
        const isCollapsed = collapsed.has(entry.id);
        const key = JSON.stringify([
          snapshot.id,
          highlighterVersion,
          value?.content.fingerprint,
          value?.content.deferred,
          errors[entry.id],
          comments.map((c) => [c.id, c.updatedAt]),
          isCollapsed,
        ]);
        const cached = itemCache.current.get(entry.id);
        if (cached?.key === key) return cached.item;
        const version = (cached?.item.version ?? 0) + 1;
        let item: CodeViewItem<ReviewComment>;
        if (value?.diff && snapshot.comparison.view === "changes") {
          item = {
            id: entry.id,
            type: "diff",
            fileDiff: value.diff,
            version,
            collapsed: isCollapsed,
            annotations: comments.map((c) => ({
              lineNumber: c.anchor.line ?? 0,
              side: c.anchor.side === "old" ? "deletions" : "additions",
              metadata: c,
            })),
          };
        } else {
          const content = value?.content;
          let contents = content?.newContent ?? content?.oldContent ?? "";
          const summary =
            !content ||
            content.deferred ||
            ["binary", "conflict", "submodule"].includes(content.kind);
          if (summary)
            contents =
              errors[entry.id] ??
              (entry.deferred || content?.deferred
                ? "Large file — load its contents to inspect."
                : content?.kind === "binary"
                  ? "Binary file — source preview is unavailable."
                  : content?.kind === "conflict"
                    ? "Unresolved merge conflict — resolve it in your editor, then refresh."
                    : content?.kind === "submodule"
                      ? "Submodule change — inspect this repository separately."
                      : "Loading file…");
          if (summary && content && !content.deferred)
            contents += `\n${content.size.toLocaleString()} bytes · ${content.status}\n${content.metadata}`;
          // The renderer prepares layout and renders against the same file
          // object. Feedback, collapse and theme changes must only replace the
          // surrounding item, not unchanged source. Summary messages also need
          // distinct keys, even before a content fingerprint is available.
          const cacheKey = JSON.stringify([
            "file",
            entry.path,
            content?.fingerprint || snapshot.id,
            summary ? contents : null,
          ]);
          const previousFile =
            cached?.item.type === "file" ? cached.item.file : undefined;
          item = {
            id: entry.id,
            type: "file",
            file:
              previousFile?.cacheKey === cacheKey
                ? previousFile
                : {
                    name: entry.path,
                    contents,
                    ...(summary ? { lang: "text" as const } : {}),
                    cacheKey,
                  },
            version,
            collapsed: isCollapsed,
            annotations: comments.map((c) => ({
              lineNumber: c.anchor.line ?? 0,
              metadata: c,
            })),
          };
        }
        itemCache.current.set(entry.id, { key, item });
        return item;
      });
    }, [
      snapshot,
      codeSelection,
      loaded,
      errors,
      queue.comments,
      collapsed,
      highlighterVersion,
    ]);

    useEffect(() => {
      const target = pendingJump.current;
      if (target && loaded[target.id] && !loaded[target.id].content.deferred) {
        navigate(target.id, target.line, target.side);
        pendingJump.current = null;
      }
    }, [items, loaded, navigate, selected]);

    const options = useMemo<CodeViewReactOptions<ReviewComment, undefined>>(
      () => ({
        diffStyle: preferences.diffLayout,
        theme: rendererThemeId(theme),
        themeType: theme.kind,
        preferredHighlighter: "shiki-wasm",
        diffIndicators: "classic",
        overflow: "scroll",
        enableGutterUtility: true,
        enableLineSelection: true,
        stickyHeaders: true,
        lineHoverHighlight: "both",
        expansionLineCount: 30,
        hunkSeparators: "line-info",
        itemMetrics: {
          lineHeight: Math.round((preferences.codeFontSize * 22) / 12),
        },
        layout: { paddingTop: 0, paddingBottom: 40, gap: 18 },
        unsafeCSS:
          viewerCSS +
          `:host { --diffs-font-size: ${preferences.codeFontSize}px; --diffs-line-height: ${Math.round((preferences.codeFontSize * 22) / 12)}px; }` +
          (preferences.showWhitespace ? whitespaceCSS : ""),
        onPostRender(node, _instance, phase, context) {
          horizontalScrollbar.current?.refresh();
          if (phase === "unmount") return;
          const props = refs.current;
          const id = context.item.id;
          const content = props.loaded[id]?.content;
          node.style.setProperty(
            "--diffs-tab-size",
            String(content?.tabWidth ?? props.preferences.tabWidth),
          );
          if (props.preferences.showWhitespace)
            decorateWhitespace(node.shadowRoot ?? node);
          for (const button of (
            node.shadowRoot ?? node
          ).querySelectorAll<HTMLElement>("[data-expand-button]")) {
            button.tabIndex = 0;
            button.setAttribute(
              "aria-label",
              button.hasAttribute("data-expand-all-button")
                ? "Expand all context"
                : button.hasAttribute("data-expand-up")
                  ? "Show preceding lines"
                  : "Show following lines",
            );
            if (!button.dataset.inspectorKeyboard) {
              button.dataset.inspectorKeyboard = "true";
              button.addEventListener("keydown", (event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  event.stopPropagation();
                  button.click();
                }
              });
            }
          }
          if (!content && !props.errors[id])
            queueMicrotask(() => props.onLoad(id));
        },
      }),
      [
        preferences.diffLayout,
        preferences.showWhitespace,
        preferences.codeFontSize,
        theme,
      ],
    );

    const renderHeader = useCallback(
      (item: CodeViewItem<ReviewComment>) => {
        const entry = entries.get(item.id)!;
        const data = loaded[item.id]?.content;
        const large = data?.deferred || (!data && entry.deferred);
        const reviewed =
          !!data?.fingerprint &&
          queue.reviewed[entry.path] === data.fingerprint;
        return (
          <div className="file-heading" data-file-path={entry.path}>
            <button
              className="icon-button"
              aria-label={`${collapsed.has(item.id) ? "Expand" : "Collapse"} ${entry.path}`}
              onClick={() =>
                setCollapsed((current) => {
                  const next = new Set(current);
                  if (next.has(item.id)) next.delete(item.id);
                  else next.add(item.id);
                  return next;
                })
              }
            >
              {collapsed.has(item.id) ? (
                <ChevronRight size={15} />
              ) : (
                <ChevronDown size={15} />
              )}
            </button>
            <FileCode2 size={15} className="muted" />
            <span className="file-heading-name" title={entry.path}>
              {entry.oldPath && (
                <span className="muted">{entry.oldPath} → </span>
              )}
              {entry.path}
            </span>
            {entry.additions > 0 && (
              <span className="add">+{entry.additions}</span>
            )}
            {entry.deletions > 0 && (
              <span className="remove">−{entry.deletions}</span>
            )}
            {!data && !large && !errors[item.id] && (
              <LoaderCircle size={13} className="spin muted" />
            )}
            {large && (
              <button
                className="small-button"
                onClick={() => onLoad(item.id, true)}
              >
                Load {snapshot.comparison.view === "code" ? "file" : "diff"}
              </button>
            )}
            {errors[item.id] && (
              <button
                className="small-button"
                onClick={() => onLoad(item.id, true)}
              >
                Retry
              </button>
            )}
            {selectedLine?.id === item.id && (
              <button
                className="small-button"
                aria-label="Comment on selected line"
                onClick={() =>
                  onComment(
                    item.id,
                    selectedLine.line,
                    snapshot.comparison.view === "changes"
                      ? (selectedLine.side ?? "new")
                      : undefined,
                  )
                }
              >
                Comment on line {selectedLine.line}
              </button>
            )}
            <button
              className="icon-button"
              aria-label={`Comment on file ${entry.path}`}
              title="Comment on file"
              onClick={() => onComment(item.id)}
            >
              <MessageSquarePlus size={15} />
            </button>
            {snapshot.comparison.view === "changes" && (
              <label
                className={`review-check ${reviewed ? "is-reviewed" : ""}`}
              >
                <input
                  type="checkbox"
                  checked={reviewed}
                  onChange={(e) => onReviewed(item.id, e.target.checked)}
                />
                <Check size={13} /> Reviewed
              </label>
            )}
          </div>
        );
      },
      [
        entries,
        loaded,
        queue.reviewed,
        collapsed,
        errors,
        onComment,
        onReviewed,
        onLoad,
        snapshot.comparison.view,
        selectedLine,
      ],
    );

    return (
      <>
        <CodeView<ReviewComment>
          ref={viewer}
          className="code-view"
          items={items}
          options={options}
          onSelectedLinesChange={(selection) =>
            setSelectedLine(
              selection
                ? {
                    id: selection.id,
                    line: selection.range.start,
                    side: selection.range.side === "deletions" ? "old" : "new",
                  }
                : null,
            )
          }
          onScroll={(top, instance) => {
            scroll.current = top;
            const current = refs.current;
            if (current.snapshot.comparison.view === "changes") {
              // Use the virtualizer's logical offsets, not overscanned DOM nodes.
              // Only a file boundary changes React state; scrolling within a file
              // stays independent of the navigator and never initiates a jump.
              const files = current.snapshot.files;
              let low = 0;
              let high = files.length - 1;
              let active: string | undefined;
              while (low <= high) {
                const middle = (low + high) >>> 1;
                const fileTop = instance.getTopForItem(files[middle].id);
                if (fileTop === undefined) break;
                if (fileTop <= top + 1) {
                  active = files[middle].id;
                  low = middle + 1;
                } else high = middle - 1;
              }
              if (active && active !== current.selected) {
                // Moving on cancels an unfinished jump to a still-loading file.
                if (pendingJump.current?.id !== active)
                  pendingJump.current = null;
                current.onActiveFileChange(current.snapshot.id, active);
              }
            }
            onScroll(top);
            horizontalScrollbar.current?.refresh();
          }}
          renderCustomHeader={renderHeader}
          renderGutterUtility={(getHoveredLine, item) => (
            <button
              className="gutter-comment"
              aria-label="Comment on line"
              onClick={() => {
                const hovered = getHoveredLine();
                if (
                  !hovered ||
                  !loaded[item.id] ||
                  loaded[item.id].content.deferred
                )
                  return;
                onComment(
                  item.id,
                  hovered.lineNumber,
                  snapshot.comparison.view === "changes"
                    ? "side" in hovered && hovered.side === "deletions"
                      ? "old"
                      : "new"
                    : undefined,
                );
              }}
            >
              <Plus size={13} />
            </button>
          )}
          renderAnnotation={(annotation) => (
            <div className="inline-comment">
              <div className="inline-comment-label">
                <span className="avatar">Y</span>
                <strong>You</strong>
                <span className="muted">
                  {annotation.metadata.scope === "file"
                    ? "File comment"
                    : `Line ${annotation.metadata.anchor.line}`}
                </span>
              </div>
              <p>{annotation.metadata.body}</p>
            </div>
          )}
          renderCodeViewFooter={() => (
            <div className="viewer-footer">
              {snapshot.comparison.view === "changes"
                ? `${snapshot.files.length} changed files · `
                : ""}
              Read-only source · Your feedback is saved locally
            </div>
          )}
        />
        <HorizontalScrollbar
          ref={horizontalScrollbar}
          path={selected ? entries.get(selected)?.path : undefined}
          getHost={() =>
            viewer.current
              ?.getInstance()
              ?.getRenderedItems()
              .find((item) => item.id === refs.current.selected)?.element
          }
        />
      </>
    );
  },
);
