import { afterEach, expect, test } from "bun:test";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Avatar } from "./Avatar";
afterEach(cleanup);

test("falls back from persona portrait to pack cover, then initials; a new persona retries", () => {
  const view = render(<Avatar sources={["data:image/png;base64,AAAA", "data:image/png;base64,BBBB"]} label="小著" fallback="小著" />);
  const avatar = screen.getByRole("img", { name: "小著" });
  fireEvent.error(avatar.querySelector("img")!);
  expect(avatar.querySelector("img")!.getAttribute("src")).toBe("data:image/png;base64,BBBB");
  fireEvent.error(avatar.querySelector("img")!);
  expect(avatar.querySelector("img")).toBeNull();
  expect(avatar.textContent).toBe("小著");
  view.rerender(<Avatar sources={["data:image/png;base64,CCCC"]} label="包包" fallback="包包" />);
  expect(screen.getByRole("img", { name: "包包" }).querySelector("img")!.getAttribute("src")).toBe("data:image/png;base64,CCCC");
});
