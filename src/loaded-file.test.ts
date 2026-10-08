import { expect, it } from "vitest";
import { parseDiffFromFile } from "@pierre/diffs";
import { retainFile, sourceLines } from "./loaded-file";
import type { FileContent } from "./types";

it("drops duplicate diff strings while preserving full source and search coordinates", () => {
  const oldContent = "\told  \r\n" + "unchanged\r\n".repeat(100) + "end";
  const newContent = "\tnew  \r\n" + "unchanged\r\n".repeat(100) + "last";
  const content = {
    oldContent,
    newContent,
    fingerprint: "stable",
    path: "file.ts",
  } as FileContent;
  const diff = parseDiffFromFile(
    { name: "file.ts", contents: oldContent },
    { name: "file.ts", contents: newContent },
    { context: 3 },
  );
  const retained = retainFile(content, diff);
  expect(retained.content).not.toHaveProperty("oldContent");
  expect(retained.content).not.toHaveProperty("newContent");
  expect(retained.diff).toBe(diff);
  expect(sourceLines(retained, "old").join("")).toBe(oldContent);
  expect(sourceLines(retained, "new").join("")).toBe(newContent);
  expect(
    sourceLines(retained, "new").findIndex((line) => line === "last") + 1,
  ).toBe(102);
  expect(retained.content.fingerprint).toBe("stable");
  expect(sourceLines(retainFile(content), "new")).toEqual(
    newContent.split("\n"),
  );
});
