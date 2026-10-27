import { afterEach, expect, jest, test } from "bun:test";
import { createStreamTextBuffer, STREAM_TEXT_INTERVAL_MS, type AssistantTextSnapshot } from "./streamText";

afterEach(() => jest.useRealTimers());

function harness() {
  jest.useFakeTimers();
  const publications: (readonly AssistantTextSnapshot[])[] = [];
  const buffer = createStreamTextBuffer(updates => publications.push(updates));
  return { buffer, publications };
}

test("first content is synchronous, later full snapshots are bounded by 33 ms", () => {
  const { buffer, publications } = harness();
  buffer.push("m", "p", "a");
  expect(publications).toEqual([[{ messageId: "m", text: "a" }]]);
  for (let i = 2; i <= 100; i++) buffer.push("m", "p", "a".repeat(i));
  expect(publications).toHaveLength(1);
  jest.advanceTimersByTime(STREAM_TEXT_INTERVAL_MS - 1);
  expect(publications).toHaveLength(1);
  jest.advanceTimersByTime(1);
  expect(publications).toHaveLength(2);
  expect(publications.at(-1)).toEqual([{ messageId: "m", text: "a".repeat(100) }]);
});

test("multiple part snapshots retain order, replace duplicates and preserve UTF-16 exactly", () => {
  const { buffer, publications } = harness();
  buffer.push("m", "p1", "\uD83D");
  buffer.push("m", "p1", "\uD83D\uDE80 ");
  buffer.push("m", "p2", "第二块");
  buffer.push("m", "p1", "🚀 开始 ");
  buffer.push("m", "p2", "第二块");
  buffer.flush();
  expect(publications.at(-1)).toEqual([{ messageId: "m", text: "🚀 开始 第二块" }]);
  const count = publications.length;
  buffer.push("m", "p2", "第二块");
  jest.advanceTimersByTime(100);
  expect(publications).toHaveLength(count);
});

test("terminal flush drains once; reset cancels queued work and the next generation starts immediately", () => {
  const { buffer, publications } = harness();
  buffer.push("old", undefined, "first");
  buffer.push("old", undefined, "final");
  buffer.flush(); buffer.flush();
  expect(publications.at(-1)).toEqual([{ messageId: "old", text: "final" }]);
  buffer.push("old", undefined, "stale");
  buffer.reset();
  buffer.push("new", undefined, "fresh");
  jest.advanceTimersByTime(100);
  expect(publications).toHaveLength(3);
  expect(publications.at(-1)).toEqual([{ messageId: "new", text: "fresh" }]);
});

test("restored parts update in place and a tool invalidation can republish equal text", () => {
  const { buffer, publications } = harness();
  buffer.restore([{ messageId: "m", partId: "p1", text: "hello " }, { messageId: "m", partId: "p2", text: "world" }]);
  buffer.push("m", "p2", "world!");
  buffer.flush();
  expect(publications).toEqual([[{ messageId: "m", text: "hello world!" }]]);
  buffer.invalidate("m");
  buffer.push("m", "p2", "world!");
  buffer.flush();
  expect(publications).toHaveLength(2);
});

for (const length of [10_000, 50_000]) {
  test(`50 events/second preserves ${length} characters with bounded publications`, () => {
    const { buffer, publications } = harness();
    const payload = ("```ts\nconst rocket = '🚀';\n```\n\n| a | b |\n|---|---|\n| 值 | 😀 |\n").repeat(Math.ceil(length / 30)).slice(0, length);
    for (let i = 1; i <= 250; i++) {
      buffer.push("tail", "part", payload.slice(0, Math.ceil(length * i / 250)));
      jest.advanceTimersByTime(20);
    }
    buffer.flush();
    expect(publications[0]?.[0]?.text).toBe(payload.slice(0, Math.ceil(length / 250)));
    expect(publications.at(-1)).toEqual([{ messageId: "tail", text: payload }]);
    expect(publications.length).toBeLessThanOrEqual(127);
    expect(publications.length).toBeGreaterThan(100);
    console.info(JSON.stringify({ scenario: "C16 deterministic timer workload", length, events: 250, durationMs: 5000, intervalMs: 33, publications: publications.length, finalTextExact: true }));
  });
}


test("an empty restored native part does not defer its first real content", () => {
  const { buffer, publications } = harness();
  buffer.restore([{ messageId: "m", partId: "p", text: "" }]);
  buffer.push("m", "p", "first real content");
  expect(publications).toEqual([[{ messageId: "m", text: "first real content" }]]);
});

test("native deltas require a known text part and snapshots remain authoritative", () => {
  const { buffer, publications } = harness();
  expect(buffer.append("m", "unknown", "hidden")).toBe(false);
  buffer.push("m", "p", "");
  expect(buffer.append("m", "p", "\uD83D")).toBe(true);
  expect(publications.at(-1)).toEqual([{ messageId: "m", text: "\uD83D" }]);
  buffer.append("m", "p", "\uDE80");
  buffer.push("m", "p2", " second");
  buffer.append("m", "p2", " part");
  buffer.flush();
  expect(publications.at(-1)).toEqual([{ messageId: "m", text: "🚀 second part" }]);
  buffer.push("m", "p", "corrected");
  buffer.flush();
  expect(publications.at(-1)).toEqual([{ messageId: "m", text: "corrected second part" }]);
  buffer.reset();
  expect(buffer.append("m", "p", "late")).toBe(false);
  buffer.restore([{ messageId: "restored", partId: "p", text: "old" }]);
  buffer.append("restored", "p", " new");
  buffer.flush();
  expect(publications.at(-1)).toEqual([{ messageId: "restored", text: "old new" }]);
});

for (const length of [10_000, 50_000]) {
  test(`native 50 Hz deltas preserve ${length} UTF-16 characters with bounded publications`, () => {
    const { buffer, publications } = harness();
    const text = "🚀 native delta 中文\n".repeat(length).slice(0, length);
    buffer.push("m", "p", "");
    for (let i = 0; i < 500; i++) {
      buffer.append("m", "p", text.slice(Math.floor(length * i / 500), Math.floor(length * (i + 1) / 500)));
      jest.advanceTimersByTime(20);
    }
    buffer.flush();
    expect(publications.at(-1)).toEqual([{ messageId: "m", text }]);
    expect(publications.length).toBeGreaterThan(200);
    expect(publications.length).toBeLessThanOrEqual(252);
  });
}
