import { afterEach, expect, it, vi } from "vitest";
import type { FileContent } from "./types";

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.resetModules();
});

async function parserFixture() {
  vi.useFakeTimers();
  const workers: FakeWorker[] = [];
  class FakeWorker {
    onmessage?: (event: { data: unknown }) => void;
    onerror?: (event: { message: string }) => void;
    messages: { requestId: number }[] = [];
    terminate = vi.fn();
    constructor() {
      workers.push(this);
    }
    postMessage(message: { requestId: number }) {
      this.messages.push(message);
    }
    complete(index: number) {
      this.onmessage?.({
        data: {
          requestId: this.messages[index].requestId,
          diff: { name: "file.ts" },
        },
      });
    }
    fail(message: string) {
      this.onerror?.({ message });
    }
  }
  vi.stubGlobal("Worker", FakeWorker);
  const { parseDiff } = await import("./parse-diff");
  return { workers, parseDiff };
}

it("releases an idle parser, but never terminates outstanding reads", async () => {
  const { workers, parseDiff } = await parserFixture();
  const first = parseDiff({} as FileContent);
  workers[0].complete(0);
  await first;
  await vi.advanceTimersByTimeAsync(10000);
  const second = parseDiff({} as FileContent);
  await vi.advanceTimersByTimeAsync(30000);
  expect(workers[0].terminate).not.toHaveBeenCalled();
  workers[0].complete(1);
  await second;
  await vi.advanceTimersByTimeAsync(15000);
  expect(workers[0].terminate).toHaveBeenCalledOnce();
  const third = parseDiff({} as FileContent);
  expect(workers).toHaveLength(2);
  workers[1].complete(0);
  await third;
  await vi.advanceTimersByTimeAsync(15000);
});

it("keeps an overlapping parse pending when another finishes first", async () => {
  const { workers, parseDiff } = await parserFixture();
  let firstSettled = false;
  const first = parseDiff({} as FileContent).then((diff) => {
    firstSettled = true;
    return diff;
  });
  const second = parseDiff({} as FileContent);
  workers[0].complete(1);
  await second;
  await vi.advanceTimersByTimeAsync(30000);
  expect(firstSettled).toBe(false);
  expect(workers[0].terminate).not.toHaveBeenCalled();

  workers[0].complete(0);
  await first;
  expect(firstSettled).toBe(true);
  await vi.advanceTimersByTimeAsync(15000);
  expect(workers[0].terminate).toHaveBeenCalledOnce();
});

it("rejects every pending parse on worker failure and recovers on the next read", async () => {
  const { workers, parseDiff } = await parserFixture();
  const first = parseDiff({} as FileContent);
  const second = parseDiff({} as FileContent);
  const failures = Promise.all([
    expect(first).rejects.toThrow("Parser crashed"),
    expect(second).rejects.toThrow("Parser crashed"),
  ]);
  workers[0].fail("Parser crashed");
  await failures;
  expect(workers[0].terminate).toHaveBeenCalledOnce();

  let recovered = false;
  const next = parseDiff({} as FileContent).then(() => {
    recovered = true;
  });
  expect(workers).toHaveLength(2);
  await vi.advanceTimersByTimeAsync(30000);
  expect(recovered).toBe(false);
  expect(workers[1].terminate).not.toHaveBeenCalled();
  workers[1].complete(0);
  await next;
  expect(recovered).toBe(true);
  await vi.advanceTimersByTimeAsync(15000);
  expect(workers[1].terminate).toHaveBeenCalledOnce();
});
