import { describe, expect, it } from "vitest";
import { createHighlightBudget, estimateBytes, SourceCache } from "./memory";

describe("retained data budgets", () => {
  it("counts shared/cyclic objects once and stops at an oversized entry", () => {
    const leaf = { text: "x".repeat(1000) };
    expect(estimateBytes([leaf, leaf])).toBeLessThan(
      estimateBytes([leaf, { ...leaf }]),
    );
    const cyclic: { self?: unknown } = {};
    cyclic.self = cyclic;
    expect(estimateBytes(cyclic)).toBeLessThan(200);
    expect(estimateBytes([leaf, leaf], 100)).toBeGreaterThan(100);
  });

  it("shares one budget across file and diff ASTs and evicts oversized results", () => {
    const fileCache = new Map([["old-file", { text: "x".repeat(2000) }]]);
    const diffCache = new Map([
      ["old-diff", { text: "x".repeat(2000) }],
      ["new-diff", { text: "ok" }],
    ]);
    const trim = createHighlightBudget(1000, 2);
    const usage = trim({ fileCache, diffCache }, (kind, key) =>
      (kind === "file" ? fileCache : diffCache).delete(key),
    );
    expect(usage.bytes).toBeLessThanOrEqual(1000);
    expect(usage.entries).toBe(1);
    expect(Array.from(diffCache.keys())).toEqual(["new-diff"]);
    expect(fileCache.size).toBe(0);
  });

  it("bounds many small entries, not just their byte size", () => {
    const fileCache = new Map(
      Array.from({ length: 40 }, (_, i) => [String(i), {}]),
    );
    const diffCache = new Map<string, object>();
    const usage = createHighlightBudget(10000, 4)(
      { fileCache, diffCache },
      (_, key) => fileCache.delete(key),
    );
    expect(usage.entries).toBe(4);
    expect(Array.from(fileCache.keys())).toEqual(["36", "37", "38", "39"]);
  });

  it("keeps recent Code files, pins the active file and releases replaced snapshots", () => {
    const cache = new SourceCache<{ text: string }>(1000, 2);
    cache.set("a", { text: "a" }, new Set());
    cache.set("b", { text: "b" }, new Set());
    cache.get("a");
    expect(Object.keys(cache.set("c", { text: "c" }, new Set(["c"])))).toEqual([
      "a",
      "c",
    ]);
    const huge = { text: "x".repeat(1000) };
    expect(cache.set("huge", huge, new Set(["huge"]))).toEqual({ huge });
    expect(cache.set("next", { text: "next" }, new Set(["next"]))).toEqual({
      next: { text: "next" },
    });
    cache.clear();
    expect(cache.get("next")).toBeUndefined();
  });
});
