import { afterEach, expect, it, vi } from "vitest";
import type { FileContent } from "./types";

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.resetModules();
});

it("releases an idle parser, but never terminates outstanding reads", async () => {
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
  }
  vi.stubGlobal("Worker", FakeWorker);
  const { parseDiff } = await import("./parse-diff");
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
