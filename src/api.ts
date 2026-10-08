import { invoke } from "@tauri-apps/api/core";
import type {
  Bootstrap,
  Comparison,
  FileContent,
  ImportedTheme,
  NewComment,
  Preferences,
  ReviewComment,
  ReviewQueue,
  Snapshot,
} from "./types";

export const api = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),
  savePreferences: (preferences: Preferences) =>
    invoke<void>("save_preferences", { preferences }),
  open: (path: string, comparison: Comparison | null) =>
    invoke<Snapshot>("open_repository", { path, comparison }),
  read: (snapshotId: string, fileId: string, force = false) =>
    invoke<FileContent>("read_file", { snapshotId, fileId, force }),
  branches: (snapshotId: string) =>
    invoke<string[]>("list_branches", { snapshotId }),
  queue: (snapshotId: string) =>
    invoke<ReviewQueue>("get_queue", { snapshotId }),
  comment: (comment: NewComment) =>
    invoke<ReviewComment>("add_comment", { comment }),
  editComment: (queueId: string, id: string, body: string) =>
    invoke<void>("update_comment", { queueId, id, body }),
  deleteComments: (queueId: string, id: string | null = null) =>
    invoke<void>("delete_comments", { queueId, id }),
  reviewed: (
    snapshotId: string,
    fileId: string,
    fingerprint: string,
    reviewed: boolean,
  ) =>
    invoke<void>("set_reviewed", { snapshotId, fileId, fingerprint, reviewed }),
  export: (snapshotId: string) =>
    invoke<string>("export_queue", { snapshotId }),
  saveExport: (snapshotId: string, path: string) =>
    invoke<void>("save_export", { snapshotId, path }),
  importTheme: (path: string) =>
    invoke<ImportedTheme>("import_theme", { path }),
  removeTheme: (id: string) => invoke<void>("remove_theme", { id }),
};
