import { forwardRef, useMemo } from "react";
import { Viewer, type ViewerHandle, type LoadedFile } from "./Viewer";
import type { NavigationFile, Preferences, Snapshot } from "./types";
import type { Theme } from "./themes";
import type { SymbolPosition } from "./navigation";
import type { SourceMatch } from "./search";

export const NavigationSource = forwardRef<
  ViewerHandle,
  {
    snapshot: Snapshot;
    file: NavigationFile;
    preferences: Preferences;
    theme: Theme;
    onLoad: () => void;
    onNavigate: (
      position: SymbolPosition,
      operation: "auto" | "references",
    ) => void;
    onError: (message: string) => void;
    findQuery?: string;
    findMatch?: SourceMatch;
  }
>(function NavigationSource(
  {
    snapshot,
    file,
    preferences,
    theme,
    onLoad,
    onNavigate,
    onError,
    findQuery,
    findMatch,
  },
  ref,
) {
  const sourceSnapshot = useMemo<Snapshot>(
    () => ({
      ...snapshot,
      comparison: { ...snapshot.comparison, view: "code" },
      files: [
        {
          id: file.documentId,
          path: file.path,
          oldPath: null,
          status: "source",
          kind: file.kind,
          additions: 0,
          deletions: 0,
          size: file.size,
          deferred: file.deferred,
        },
      ],
    }),
    [snapshot, file],
  );
  const loaded = useMemo<Record<string, LoadedFile>>(
    () => ({
      [file.documentId]: {
        content: {
          snapshotId: file.snapshotId,
          fileId: file.documentId,
          path: file.path,
          oldPath: null,
          status: "source",
          kind: file.kind,
          oldContent: null,
          newContent: file.content,
          fingerprint: file.fingerprint,
          tabWidth: file.tabWidth ?? preferences.tabWidth,
          deferred: file.deferred,
          size: file.size,
          metadata: file.label,
        },
      },
    }),
    [file, preferences.tabWidth],
  );
  return (
    <Viewer
      ref={ref}
      snapshot={sourceSnapshot}
      selected={file.documentId}
      loaded={loaded}
      errors={{}}
      queue={{ comments: [], reviewed: {} }}
      preferences={preferences}
      theme={theme}
      onLoad={(_id, force) => {
        if (force) onLoad();
      }}
      onComment={() => {}}
      onReviewed={() => {}}
      onScroll={() => {}}
      onActiveFileChange={() => {}}
      onError={onError}
      navigationOnly
      navigationSide={file.side}
      onNavigate={onNavigate}
      findQuery={findQuery}
      findMatch={findMatch}
    />
  );
});
