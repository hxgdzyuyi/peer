import type { FileDiffMetadata } from "@pierre/diffs";
import type { FileContent } from "./types";

export type LoadedFile = {
  content: Omit<FileContent, "oldContent" | "newContent"> &
    Partial<Pick<FileContent, "oldContent" | "newContent">>;
  diff?: FileDiffMetadata;
};

export function retainFile(
  content: FileContent,
  diff?: FileDiffMetadata,
): LoadedFile {
  if (!diff) return { content };
  // Full diff models already contain every source line (including collapsed
  // context). Don't also keep both original multi-megabyte strings in React.
  const { oldContent: _old, newContent: _new, ...metadata } = content;
  return { content: metadata, diff };
}

export function sourceLines(
  file: LoadedFile,
  side: "old" | "new",
): readonly string[] {
  if (file.diff)
    return side === "old" ? file.diff.deletionLines : file.diff.additionLines;
  return (
    (side === "old" ? file.content.oldContent : file.content.newContent)?.split(
      "\n",
    ) ?? []
  );
}
