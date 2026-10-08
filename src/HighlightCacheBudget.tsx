import { useEffect } from "react";
import { useWorkerPool } from "@pierre/diffs/react";
import { createHighlightBudget } from "./memory";

/** Budget cached ASTs without discarding visible renderers or their layout. */
export function HighlightCacheBudget({ scope }: { scope?: string }) {
  const pool = useWorkerPool();
  useEffect(() => {
    if (!pool) return;
    const trim = createHighlightBudget();
    const evict = (kind: "file" | "diff", key: string) => {
      if (kind === "file") pool.evictFileFromCache(key);
      else pool.evictDiffFromCache(key);
    };
    const clear = () => {
      const { fileCache, diffCache } = pool.inspectCaches();
      for (const [key] of Array.from(fileCache)) evict("file", key);
      for (const [key] of Array.from(diffCache)) evict("diff", key);
    };
    clear();
    const unsubscribe = pool.subscribeToStatChanges(() => {
      const usage = trim(pool.inspectCaches(), evict);
      if (import.meta.env.VITE_DESKTOP_TEST === "1") {
        document.documentElement.dataset.highlightCacheBytes = String(
          usage.bytes,
        );
        document.documentElement.dataset.highlightCacheEntries = String(
          usage.entries,
        );
      }
    });
    return () => {
      unsubscribe();
      clear();
    };
  }, [pool, scope]);
  return null;
}
