import { afterEach, expect, mock, spyOn, test } from "bun:test";
import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { UsageCloud } from "./UsageCloud";
import { usageCloud, usageError } from "./usageCloudData";
import { dict } from "../../lib/i18n";
import type { RadialModel } from "./model";

afterEach(cleanup);
const model: RadialModel = { id: "cloud", revision: 0, title: "AI 用量", theme: "mint", language: "zh-CN", anchor: null, items: [] };
const t = dict("zh-CN");
const local = { localAvailable: true, todayTokens: 10, todayRequests: 1, topModels: [] };
function animationEnd(element: Element, name = "pet-usage-fade-out") {
  const event = new Event("animationend", { bubbles: true });
  Object.defineProperty(event, "animationName", { value: name });
  fireEvent(element, event);
}

test("shows percentage and yuan with the existing 10000-unit conversion; clicking the values closes", () => {
  const usage = usageCloud({ remainingCny: 27260000, limitCny: 30000000, remainingPct: 91,
    daysUntilReset: 3, todayCostCny: 100, todayRequests: 1, topModels: [] }, t);
  const close = mock(() => {});
  const view = render(<UsageCloud model={model} usage={usage} onClose={close} />);
  expect(view.getByText("91%")).toBeDefined();
  expect(view.getByText("¥2726.00")).toBeDefined();
  expect(document.activeElement).toBe(view.getByRole("button", { name: "AI 用量 · 单击关闭" }));
  fireEvent.click(view.getByText("¥2726.00"));
  expect(close).not.toHaveBeenCalled();
  fireEvent.keyDown(view.getByRole("button"), { key: "Escape" });
  animationEnd(view.container.firstElementChild!, "pet-usage-fade-in");
  expect(close).not.toHaveBeenCalled();
  animationEnd(view.container.firstElementChild!);
  animationEnd(view.container.firstElementChild!);
  expect(close).toHaveBeenCalledTimes(1);
});

test("a loading or unavailable cloud remains clickable", () => {
  const close = mock(() => {});
  const view = render(<UsageCloud model={model} usage={{ label: "", percent: null, amount: "—", note: t.aiUsageLoading }} onClose={close} />);
  expect(view.getByRole("status").textContent).toContain(t.aiUsageLoading);
  fireEvent.click(view.getByText(t.aiUsageLoading));
  expect(close).not.toHaveBeenCalled();
  animationEnd(view.container.firstElementChild!);
  expect(close).toHaveBeenCalledTimes(1);
});

test("waits for native visibility before fading in, and a fresh session can fade in again", () => {
  const usage = { label: "", percent: null, amount: "—", note: t.aiUsageLoading };
  const view = render(<UsageCloud key="first" model={model} usage={usage} visible={false} onClose={() => {}} />);
  expect(view.container.firstElementChild!.getAttribute("data-phase")).toBe("waiting");
  view.rerender(<UsageCloud key="first" model={model} usage={usage} visible onClose={() => {}} />);
  expect(view.container.firstElementChild!.getAttribute("data-phase")).toBe("visible");
  fireEvent.keyDown(view.getByRole("button"), { key: "Escape" });
  expect(view.container.firstElementChild!.getAttribute("data-phase")).toBe("leaving");
  view.rerender(<UsageCloud key="second" model={{ ...model, id: "second" }} usage={usage} visible={false} onClose={() => {}} />);
  expect(view.container.firstElementChild!.getAttribute("data-phase")).toBe("waiting");
});

test("closing during fade-in starts at the current opacity, ignores child animation events, and has a fallback", async () => {
  const close = mock(() => {});
  const view = render(<UsageCloud model={model} usage={{ label: "", percent: null, amount: "—", note: "" }} onClose={close} />);
  const root = view.container.firstElementChild as HTMLElement;
  root.style.opacity = "0.35";
  fireEvent.click(view.getByRole("button"));
  expect(root.style.getPropertyValue("--pet-usage-fade-from")).toBe("0.35");
  animationEnd(view.getByRole("button"));
  expect(close).not.toHaveBeenCalled();
  await act(async () => { await new Promise(resolve => setTimeout(resolve, 450)); });
  expect(close).toHaveBeenCalledTimes(1);
});

test("unmount cancels a pending close so it cannot dismiss the next cloud", async () => {
  const close = mock(() => {});
  const view = render(<UsageCloud model={model} usage={{ label: "", percent: null, amount: "—", note: "" }} onClose={close} />);
  fireEvent.click(view.getByRole("button"));
  view.unmount();
  await act(async () => { await new Promise(resolve => setTimeout(resolve, 450)); });
  expect(close).not.toHaveBeenCalled();
});

test("reduced motion closes without waiting for an animation event", async () => {
  const original = window.matchMedia.bind(window);
  const media = spyOn(window, "matchMedia").mockImplementation(query => {
    const result = original(query);
    Object.defineProperty(result, "matches", { value: true });
    return result;
  });
  try {
    const close = mock(() => {});
    const view = render(<UsageCloud model={model} usage={{ label: "", percent: null, amount: "—", note: "" }} onClose={close} />);
    fireEvent.click(view.getByRole("button"));
    await act(async () => { await new Promise(resolve => setTimeout(resolve, 10)); });
    expect(close).toHaveBeenCalledTimes(1);
  } finally { media.mockRestore(); }
});

test("OpenRouter uses its key limit and does not invent percentages for unlimited or zero limits", () => {
  const usage = { ...local, kind: "openrouter" as const, limitUsd: 20, remainingUsd: 5, todayCostUsd: 1 };
  expect(usageCloud(usage, t)).toEqual({ label: t.aiUsageKeyRemaining, percent: 25, amount: "$5.00", note: "" });
  expect(usageCloud({ ...usage, limitUsd: null, remainingUsd: null }, t)).toEqual({ label: t.aiUsageKeyRemaining, percent: null, amount: "—", note: t.aiUsageNoKeyLimit });
  expect(usageCloud({ ...usage, limitUsd: 0, remainingUsd: 0 }, t).percent).toBeNull();
});

test("DeepSeek preserves currencies without inventing a limit; local usage has no account balance", () => {
  expect(usageCloud({ ...local, kind: "deepseek", isAvailable: true, balances: [{ currency: "CNY", totalBalance: "12.34" }, { currency: "USD", totalBalance: "5.60" }] }, t))
    .toEqual({ label: t.aiUsageAccountBalance, percent: null, amount: "¥12.34 · $5.60", note: "" });
  expect(usageCloud({ ...local, kind: "local" }, t)).toEqual({ label: t.aiUsageAccountBalance, percent: null, amount: "—", note: t.aiUsageAccountUnavailable });
});

test("request errors produce safe localized statuses without leaking raw errors", () => {
  expect(usageError(new Error("status:401"), t)).toBe(t.aiUsageUnauthorized);
  expect(usageError("empty_key", t)).toBe(t.aiUsageMissingKey);
  expect(usageError("deepseek_auth_failed", t)).toBe(t.aiUsageKeyInvalid);
  expect(usageError("network: secret-key", t)).toBe(t.aiUsageUnavailable);
});
