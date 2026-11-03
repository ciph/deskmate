import { afterEach, expect, mock, test } from "bun:test";
import { cleanup, fireEvent, render } from "@testing-library/react";
import { RadialMenu } from "./RadialMenu";
import { findAction, type RadialModel } from "./model";

afterEach(cleanup);
const model: RadialModel = { id: "test", revision: 1, title: "百变小著", theme: "mint", language: "zh-CN", anchor: null, layout: { width: 368, height: 368, centerX: 184, centerY: 184, innerRadius: 110, outerRadius: 174 }, items: [
  { id: "chat", label: "聊天", icon: "chat", enabled: true },
  { id: "forms", label: "形态", icon: "pet", enabled: true, children: [
    { id: "mochi", label: "团子", icon: "pet", enabled: true, checked: true },
    { id: "ghost", label: "幽灵", icon: "pet", enabled: false },
  ] },
  { id: "settings", label: "设置", icon: "general", enabled: true },
] };
test("categories replace the ring, preserve checked and disabled state, and return focus to their parent", () => {
  const finish = mock((_action: string | null) => {});
  const view = render(<RadialMenu model={model} onFinish={finish} />);
  fireEvent.click(view.getByRole("button", { name: "形态" }));
  expect(view.queryByRole("button", { name: "聊天" })).toBeNull();
  expect(view.getByRole("button", { name: "团子" }).getAttribute("aria-pressed")).toBe("true");
  expect(view.getByRole("button", { name: "幽灵" }).hasAttribute("disabled")).toBe(true);
  expect(document.activeElement).toBe(view.getByRole("button", { name: "团子" }));
  fireEvent.click(view.getByRole("button", { name: "返回" }));
  expect(document.activeElement).toBe(view.getByRole("button", { name: "形态" }));
  expect(finish).not.toHaveBeenCalled();
  fireEvent.click(view.getByRole("button", { name: "聊天" }));
  expect(finish).toHaveBeenCalledWith("chat");
});
test("Tab stays in the circle, arrows skip disabled choices, Escape and outside click close", () => {
  const finish = mock((_action: string | null) => {});
  const view = render(<RadialMenu model={model} onFinish={finish} />);
  const dialog = view.getByRole("dialog");
  fireEvent.keyDown(dialog, { key: "Tab", shiftKey: true });
  expect(document.activeElement).toBe(view.getByRole("button", { name: "关闭" }));
  fireEvent.keyDown(dialog, { key: "Tab" });
  expect(document.activeElement).toBe(view.getByRole("button", { name: "聊天" }));
  fireEvent.click(view.getByRole("button", { name: "形态" }));
  fireEvent.keyDown(dialog, { key: "ArrowRight" });
  expect(document.activeElement).toBe(view.getByRole("button", { name: "返回" }));
  fireEvent.keyDown(dialog, { key: "Escape" });
  fireEvent.pointerDown(view.container.querySelector(".radial-dismiss")!);
  expect(finish.mock.calls).toEqual([[null], [null]]);
});
test("settings updates do not reset an open category or steal its focused choice", () => {
  const view = render(<RadialMenu model={model} onFinish={() => {}} />);
  fireEvent.click(view.getByRole("button", { name: "形态" }));
  const choice = view.getByRole("button", { name: "团子" });
  view.rerender(<RadialMenu model={{ ...model, revision: 2, theme: "dark" }} onFinish={() => {}} />);
  expect(document.activeElement).toBe(choice);
  expect(view.queryByRole("button", { name: "聊天" })).toBeNull();
});
test("action lookup rejects groups and disabled leaves", () => {
  expect(findAction(model.items, "forms")).toBeUndefined();
  expect(findAction(model.items, "ghost")).toBeUndefined();
  expect(findAction(model.items, "mochi")?.label).toBe("团子");
});

test("center stays transparent and all controls, including back, live on the ring", () => {
  const view = render(<RadialMenu model={model} onFinish={() => {}} />);
  expect(view.container.querySelector(".radial-center")).toBeNull();
  expect(view.container.querySelector(".radial-usage")).toBeNull();
  expect(view.getByRole("button", { name: "关闭" }).classList.contains("radial-sector")).toBe(true);
  fireEvent.click(view.getByRole("button", { name: "形态" }));
  expect(view.getByRole("button", { name: "返回" }).classList.contains("radial-sector")).toBe(true);
});
