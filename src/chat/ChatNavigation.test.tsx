import { afterEach, describe, expect, test } from "bun:test";
import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { createRef } from "react";
import { ChatNavigation, conversationTurns } from "./ChatNavigation";

const originalResizeObserver = globalThis.ResizeObserver;
const originalRequestAnimationFrame = globalThis.requestAnimationFrame;
const originalCancelAnimationFrame = globalThis.cancelAnimationFrame;
afterEach(() => {
  cleanup();
  globalThis.ResizeObserver = originalResizeObserver;
  globalThis.requestAnimationFrame = originalRequestAnimationFrame;
  globalThis.cancelAnimationFrame = originalCancelAnimationFrame;
});

function observeNavigation() {
  const instances: ProbeResizeObserver[] = [];
  const frames = new Map<number, FrameRequestCallback>();
  let nextFrame = 0;
  class ProbeResizeObserver {
    readonly observed = new Set<Element>();
    observeCalls = 0;
    unobserveCalls = 0;
    disconnectCalls = 0;
    constructor(readonly callback: ResizeObserverCallback) { instances.push(this); }
    observe(element: Element) { this.observeCalls += 1; this.observed.add(element); }
    unobserve(element: Element) { this.unobserveCalls += 1; this.observed.delete(element); }
    disconnect() { this.disconnectCalls += 1; this.observed.clear(); }
    resize() { this.callback([], this); }
  }
  globalThis.ResizeObserver = ProbeResizeObserver;
  globalThis.requestAnimationFrame = (callback) => { frames.set(++nextFrame, callback); return nextFrame; };
  globalThis.cancelAnimationFrame = (id) => { frames.delete(id); };
  return {
    instances,
    frames,
    flush() {
      act(() => {
        const callbacks = [...frames.values()];
        frames.clear();
        for (const callback of callbacks) callback(0);
      });
    },
    counts() {
      return {
        constructed: instances.length,
        observe: instances.reduce((sum, item) => sum + item.observeCalls, 0),
        unobserve: instances.reduce((sum, item) => sum + item.unobserveCalls, 0),
        disconnect: instances.reduce((sum, item) => sum + item.disconnectCalls, 0),
      };
    },
  };
}

function timelineFixture(count: number) {
  const list = document.createElement("div");
  const timeline = document.createElement("div");
  timeline.append(list);
  Object.defineProperties(list, {
    clientHeight: { configurable: true, value: 400 },
    scrollHeight: { configurable: true, value: count * 200 },
  });
  Object.defineProperty(timeline, "clientHeight", { value: 600 });
  const rows = new Map<string, HTMLElement>();
  const messages = Array.from({ length: count }, (_, index) => [
    { id: `u${index}`, role: "user", text: `Question ${index}` },
    { id: `a${index}`, role: "assistant", text: `Reply ${index}` },
  ]).flat();
  for (const [index, message] of messages.entries()) {
    const row = document.createElement("div");
    row.dataset.messageId = message.id;
    row.textContent = message.text;
    row.getBoundingClientRect = () => ({ top: index * 100 - list.scrollTop } as DOMRect);
    list.append(row);
    rows.set(message.id, row);
  }
  const ref = createRef<HTMLDivElement>();
  ref.current = list;
  return { list, rows, messages, ref };
}

async function flushMutations() {
  await act(async () => { await new Promise((resolve) => setTimeout(resolve, 0)); });
}

describe("conversation navigation", () => {
  test("groups replies under user turns and excludes artifact/tool-only rows", () => {
    expect(conversationTurns([
      { id: "intro", role: "assistant", text: "Welcome" },
      { id: "u1", role: "user", text: "First question" },
      { id: "a1", role: "assistant", text: "**First** answer" },
      { id: "file", role: "artifact" },
      { id: "a2", role: "assistant", text: "More detail" },
      { id: "u2", role: "user", text: "" },
    ])).toEqual([
      { id: "u1", title: "First question", summary: "First answer More detail" },
      { id: "u2", title: "", summary: "" },
    ]);
  });

  test("previews on focus, dismisses with Escape and jumps only on activation", () => {
    const list = document.createElement("div");
    const timeline = document.createElement("div");
    timeline.append(list);
    const target = document.createElement("div");
    target.dataset.messageId = "u1";
    list.append(target);
    const ref = createRef<HTMLDivElement>();
    ref.current = list;
    let jumps = 0;
    list.scrollTo = () => { jumps += 1; };
    const { getByRole, queryByRole } = render(<ChatNavigation listRef={ref} lang="zh-CN" onNavigate={() => {}}
      messages={[{ id: "u1", role: "user", text: "第一问" }, { id: "a1", role: "assistant", text: "第一答" }, { id: "u2", role: "user", text: "第二问" }]} />);
    const button = getByRole("button", { name: "第 1 轮：第一问" });
    fireEvent.focus(button);
    expect(getByRole("tooltip").textContent).toContain("第一答");
    expect(jumps).toBe(0);
    fireEvent.keyDown(button, { key: "Escape" });
    expect(queryByRole("tooltip")).toBeNull();
    fireEvent.click(button);
    expect(jumps).toBe(1);
  });

  test("500 turns retain one observer across 50 text updates and observe only changed rows", async () => {
    const probe = observeNavigation();
    const fixture = timelineFixture(500);
    let messages = fixture.messages;
    const props = { listRef: fixture.ref, lang: "en", onNavigate() {} };
    const view = render(<ChatNavigation {...props} messages={messages} />);
    const initial = probe.counts();
    for (let index = 0; index < 50; index += 1) {
      messages = messages.map((message) => message.id === "a499" ? { ...message, text: `${message.text} delta${index}` } : message);
      fixture.rows.get("a499")!.textContent = messages.at(-1)!.text;
      view.rerender(<ChatNavigation {...props} messages={messages} />);
    }
    await flushMutations();
    const afterText = probe.counts();
    console.info("navigation-observer-counts", JSON.stringify({ turns: 500, messages: 1000, textUpdates: 50, initial, afterText }));
    expect(initial).toEqual({ constructed: 1, observe: 1001, unobserve: 0, disconnect: 0 });
    expect(afterText).toEqual(initial);
    expect(probe.frames.size).toBe(0);

    const added = document.createElement("div");
    added.dataset.messageId = "u500";
    fixture.list.append(added);
    messages = [...messages, { id: "u500", role: "user", text: "New turn" }];
    view.rerender(<ChatNavigation {...props} messages={messages} />);
    await flushMutations();
    expect(probe.counts()).toEqual({ constructed: 1, observe: 1002, unobserve: 0, disconnect: 0 });
    fixture.rows.get("u0")!.remove();
    fixture.rows.get("a0")!.remove();
    messages = messages.filter((message) => message.id !== "u0" && message.id !== "a0");
    view.rerender(<ChatNavigation {...props} messages={messages} />);
    await flushMutations();
    expect(probe.counts()).toEqual({ constructed: 1, observe: 1002, unobserve: 2, disconnect: 0 });

    const replacement = document.createElement("div");
    replacement.dataset.messageId = "u500";
    added.replaceWith(replacement);
    await flushMutations();
    expect(probe.instances[0]!.observed.has(added)).toBe(false);
    expect(probe.instances[0]!.observed.has(replacement)).toBe(true);
    console.info("navigation-observer-delta-counts", JSON.stringify(probe.counts()));
    view.unmount();
    expect(probe.counts().disconnect).toBe(1);
    expect(probe.frames.size).toBe(0);
  });

  test("resize after a late image or collapse updates the active turn, rail visibility and jump target", async () => {
    const probe = observeNavigation();
    const fixture = timelineFixture(3);
    fixture.list.scrollTop = 200;
    Object.defineProperty(fixture.list, "scrollHeight", { configurable: true, value: 1000 });
    let secondTop = 0;
    fixture.rows.get("u1")!.getBoundingClientRect = () => ({ top: secondTop } as DOMRect);
    let navigations = 0;
    let jumpTop = -1;
    fixture.list.scrollTo = (options) => { if (typeof options === "object") jumpTop = options.top ?? -1; };
    const view = render(<ChatNavigation listRef={fixture.ref} lang="en" messages={fixture.messages} onNavigate={() => { navigations += 1; }} />);
    const first = view.getByRole("button", { name: "Turn 1：Question 0" });
    const second = view.getByRole("button", { name: "Turn 2：Question 1" });
    const rail = view.getByRole("navigation");
    Object.defineProperties(rail, { clientHeight: { value: 16 } });
    Object.defineProperties(first, { offsetTop: { value: 0 }, offsetHeight: { value: 16 } });
    Object.defineProperties(second, { offsetTop: { value: 32 }, offsetHeight: { value: 16 } });
    expect(second.getAttribute("aria-current")).toBe("step");

    const image = document.createElement("img");
    fixture.rows.get("a0")!.append(image);
    secondTop = 80;
    probe.instances[0]!.resize();
    probe.instances[0]!.resize();
    expect(probe.frames.size).toBe(1);
    probe.flush();
    expect(first.getAttribute("aria-current")).toBe("step");
    expect(rail.scrollTop).toBe(0);
    secondTop = 0;
    probe.instances[0]!.resize();
    probe.flush();
    expect(second.getAttribute("aria-current")).toBe("step");
    expect(rail.scrollTop).toBe(32);
    expect(probe.counts()).toEqual({ constructed: 1, observe: 7, unobserve: 0, disconnect: 0 });
    fireEvent.focus(second);
    expect(view.getByRole("tooltip").textContent).toContain("Reply 1");
    fireEvent.keyDown(second, { key: "Escape" });
    expect(view.queryByRole("tooltip")).toBeNull();
    fireEvent.click(second);
    expect(navigations).toBe(1);
    expect(jumpTop).toBe(188);
    fixture.list.scrollTop = 200;
    Object.defineProperty(fixture.list, "scrollHeight", { configurable: true, value: 600 });
    fireEvent.scroll(fixture.list);
    probe.flush();
    expect(view.getByRole("button", { name: "Turn 3：Question 2" }).getAttribute("aria-current")).toBe("step");
  });

  test("reordering keeps subscriptions, session IDs refresh and disabled navigation releases observers", async () => {
    const probe = observeNavigation();
    const fixture = timelineFixture(2);
    const props = { listRef: fixture.ref, lang: "en", onNavigate() {} };
    const view = render(<ChatNavigation {...props} messages={fixture.messages} />);
    const initial = probe.counts();
    fixture.list.append(fixture.rows.get("a0")!);
    await flushMutations();
    expect(probe.counts()).toEqual(initial);
    probe.flush();

    const switched = fixture.messages.map((message) => ({ ...message, id: `next-${message.id}`, text: `Next ${message.text}` }));
    for (const [id, row] of fixture.rows) row.dataset.messageId = `next-${id}`;
    view.rerender(<ChatNavigation {...props} messages={switched} />);
    expect(probe.frames.size).toBe(1);
    probe.flush();
    const first = view.getByRole("button", { name: "Turn 1：Next Question 0" });
    expect(first.getAttribute("aria-current")).toBe("step");
    fireEvent.focus(first);
    expect(view.getByRole("tooltip").textContent).toContain("Next Reply 0");
    expect(probe.counts()).toEqual(initial);

    view.rerender(<ChatNavigation {...props} messages={switched.slice(0, 2)} />);
    expect(view.queryByRole("navigation")).toBeNull();
    expect(view.queryByRole("tooltip")).toBeNull();
    expect(probe.counts().disconnect).toBe(1);
    expect(probe.frames.size).toBe(0);
    fixture.list.append(document.createElement("div"));
    await flushMutations();
    expect(probe.frames.size).toBe(0);
    expect(probe.counts().observe).toBe(initial.observe);

    view.rerender(<ChatNavigation {...props} messages={switched} />);
    expect(probe.counts()).toEqual({ constructed: 2, observe: 11, unobserve: 0, disconnect: 1 });
    expect(view.queryByRole("tooltip")).toBeNull();
    expect(view.getByRole("button", { name: "Turn 1：Next Question 0" }).getAttribute("aria-current")).toBe("step");
    fireEvent.scroll(fixture.list);
    expect(probe.frames.size).toBe(1);
    view.unmount();
    expect(probe.frames.size).toBe(0);
    expect(probe.counts().disconnect).toBe(2);
  });

});
