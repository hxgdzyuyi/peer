// These are estimates of retained JS data, not limits on the WebView process.
// Strings, object headers and property/array slots all contribute; shared
// objects are counted once. Stop early for entries larger than the budget.
export function estimateBytes(value: unknown, ceiling = Infinity): number {
  const seen = new Set<object>();
  const pending = [value];
  let bytes = 0;
  while (pending.length && bytes <= ceiling) {
    const item = pending.pop();
    if (typeof item === "string") bytes += 24 + item.length * 2;
    else if (item !== null && typeof item === "object" && !seen.has(item)) {
      seen.add(item);
      bytes += 64;
      for (const key in item) {
        if (!Object.hasOwn(item, key)) continue;
        bytes += 16;
        pending.push((item as Record<string, unknown>)[key]);
      }
    } else bytes += 8;
  }
  return bytes;
}

/** A byte- and entry-bounded source cache. Active files survive eviction. */
export class SourceCache<T extends object> {
  private entries = new Map<string, { value: T; bytes: number }>();
  private bytes = 0;
  constructor(
    private maxBytes: number,
    private maxEntries: number,
  ) {}
  get(id: string): T | undefined {
    const entry = this.entries.get(id);
    if (!entry) return;
    this.entries.delete(id);
    this.entries.set(id, entry);
    return entry.value;
  }
  set(id: string, value: T, pinned: ReadonlySet<string>): Record<string, T> {
    this.bytes -= this.entries.get(id)?.bytes ?? 0;
    this.entries.delete(id);
    const bytes = estimateBytes(value);
    this.entries.set(id, { value, bytes });
    this.bytes += bytes;
    for (const [key, entry] of this.entries) {
      if (this.bytes <= this.maxBytes && this.entries.size <= this.maxEntries)
        break;
      if (pinned.has(key)) continue;
      this.entries.delete(key);
      this.bytes -= entry.bytes;
    }
    return Object.fromEntries(
      Array.from(this.entries, ([key, entry]) => [key, entry.value]),
    );
  }
  clear() {
    this.entries.clear();
    this.bytes = 0;
  }
}

type Cache = Iterable<[string, object]>;
export type HighlightCaches = {
  fileCache: Cache;
  diffCache: Cache;
};
export const HIGHLIGHT_BUDGET = 24 * 1024 * 1024;
export const HIGHLIGHT_ENTRIES = 12;

export function createHighlightBudget(
  maxBytes = HIGHLIGHT_BUDGET,
  maxEntries = HIGHLIGHT_ENTRIES,
) {
  const weights = new WeakMap<object, number>();
  return (
    caches: HighlightCaches,
    evict: (kind: "file" | "diff", key: string) => void,
  ) => {
    const list = (cache: Cache) =>
      Array.from(cache, ([key, value]) => {
        let bytes = weights.get(value);
        if (bytes === undefined) {
          bytes = estimateBytes(value, maxBytes);
          weights.set(value, bytes);
        }
        return { key, bytes };
      });
    // Each library cache iterates least-recently-used first. Trim the larger
    // side first so switching views cannot retain two independent budgets.
    const file = list(caches.fileCache),
      diff = list(caches.diffCache);
    let fileBytes = file.reduce((sum, item) => sum + item.bytes, 0);
    let diffBytes = diff.reduce((sum, item) => sum + item.bytes, 0);
    while (
      fileBytes + diffBytes > maxBytes ||
      file.length + diff.length > maxEntries
    ) {
      const kind = fileBytes >= diffBytes && file.length ? "file" : "diff";
      const item = (kind === "file" ? file : diff).shift();
      if (!item) break;
      evict(kind, item.key);
      if (kind === "file") fileBytes -= item.bytes;
      else diffBytes -= item.bytes;
    }
    return { bytes: fileBytes + diffBytes, entries: file.length + diff.length };
  };
}
