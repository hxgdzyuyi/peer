export type View = "code" | "changes";
export type Comparison = {
  view: View;
  mode: "all" | "staged" | "unstaged" | "branch";
  baseRef: string | null;
  includeUntracked: boolean;
};
export type Repository = {
  id: string;
  root: string;
  name: string;
  branch: string;
  head: string | null;
};
export type FileEntry = {
  id: string;
  path: string;
  oldPath: string | null;
  status: string;
  kind: string;
  additions: number;
  deletions: number;
  size: number;
  deferred: boolean;
};
export type Snapshot = {
  id: string;
  repository: Repository;
  comparison: Comparison;
  queueId: string;
  baseCommit: string | null;
  files: FileEntry[];
};
export type FileContent = {
  snapshotId: string;
  fileId: string;
  path: string;
  oldPath: string | null;
  status: string;
  kind: string;
  oldContent: string | null;
  newContent: string | null;
  fingerprint: string;
  tabWidth: number;
  deferred: boolean;
  size: number;
  metadata: string;
};
export type Anchor = {
  snapshotId: string;
  fingerprint: string;
  baseCommit?: string;
  headCommit?: string;
  side?: "old" | "new";
  line?: number;
  lineText?: string;
  contextBefore: string[];
  contextAfter: string[];
};
export type ReviewComment = {
  id: string;
  queueId: string;
  path: string;
  oldPath?: string;
  sourceView: View;
  scope: "file" | "line";
  body: string;
  createdAt: string;
  updatedAt: string;
  anchor: Anchor;
  outdated: boolean;
};
export type ReviewQueue = {
  comments: ReviewComment[];
  reviewed: Record<string, string>;
};
export type NewComment = {
  snapshotId: string;
  fileId: string;
  fingerprint: string;
  body: string;
  side: "old" | "new" | null;
  line: number | null;
};
export type Appearance = {
  mode: "system" | "light" | "dark";
  lightTheme: string;
  darkTheme: string;
};
export type Preferences = {
  appearance: Appearance;
  recentRepositories: string[];
  lastRepository: string | null;
  sessions: Record<
    string,
    {
      comparison: Comparison;
      selectedPath: string | null;
      scrollTop: number;
      lastChanges?: Comparison;
    }
  >;
  tabWidth: number;
  showWhitespace: boolean;
  codeFontSize: number;
  diffLayout: "unified" | "split";
  gitExecutable: string | null;
};
export type ImportedTheme = {
  id: string;
  name: string;
  kind: "light" | "dark";
  definition: import("@pierre/diffs").ThemeRegistration;
};
export type Bootstrap = { preferences: Preferences; themes: ImportedTheme[] };
export const defaultComparison: Comparison = {
  view: "changes",
  mode: "all",
  baseRef: null,
  includeUntracked: true,
};
export const defaultPreferences: Preferences = {
  appearance: {
    mode: "system",
    lightTheme: "github-light",
    darkTheme: "github-dark",
  },
  recentRepositories: [],
  lastRepository: null,
  sessions: {},
  tabWidth: 4,
  showWhitespace: false,
  codeFontSize: 12,
  diffLayout: "unified",
  gitExecutable: null,
};
