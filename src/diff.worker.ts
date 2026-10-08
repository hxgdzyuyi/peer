import { parseDiffFromFile } from "@pierre/diffs";
import type { FileContent } from "./types";

self.onmessage = ({
  data,
}: MessageEvent<{ requestId: number; content: FileContent }>) => {
  const { content, requestId } = data;
  try {
    const oldFile =
      content.oldContent === null
        ? null
        : {
            name: content.oldPath ?? content.path,
            contents: content.oldContent,
            cacheKey: `${content.fingerprint}-old`,
          };
    const newFile =
      content.newContent === null
        ? null
        : {
            name: content.path,
            contents: content.newContent,
            cacheKey: `${content.fingerprint}-new`,
          };
    const diff = parseDiffFromFile(oldFile, newFile, { context: 3 });
    diff.cacheKey = content.fingerprint;
    self.postMessage({ requestId, diff });
  } catch (error) {
    self.postMessage({ requestId, error: String(error) });
  }
};
