import { afterEach, expect, mock, test } from "bun:test";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { dict } from "../lib/i18n";
const prepared = mock(async () => "data:image/png;base64,AAAA");
mock.module("../lib/userAvatar", () => ({ prepareUserAvatar: prepared, userAvatarSource: (value: string | null) => value || undefined }));
const { UserAvatarControl } = await import("./UserAvatarControl");
const t = dict("zh-CN");
afterEach(() => { cleanup(); prepared.mockClear(); });

test("cancel does not save, selection saves the thumbnail, and reset saves null", async () => {
  const save = mock(async () => {});
  const view = render(<UserAvatarControl value={null} onSave={save} t={t} />);
  const input = screen.getByLabelText(t.changeAvatar, { selector: "input" });
  fireEvent.change(input, { target: { files: [] } });
  expect(save).not.toHaveBeenCalled();
  fireEvent.change(input, { target: { files: [new File(["a"], "a.png", { type: "image/png" })] } });
  await waitFor(() => expect(save).toHaveBeenCalledWith("data:image/png;base64,AAAA"));
  view.rerender(<UserAvatarControl value="data:image/png;base64,AAAA" onSave={save} t={t} />);
  await waitFor(() => expect(screen.getByRole("button", { name: t.resetAvatar }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: t.resetAvatar }));
  await waitFor(() => expect(save).toHaveBeenCalledWith(null));
});
test("failed persistence reports an error and keeps the old preview", async () => {
  render(<UserAvatarControl value="data:image/png;base64,OLD" onSave={async () => { throw new Error("disk full"); }} t={t} />);
  fireEvent.click(screen.getByRole("button", { name: t.resetAvatar }));
  expect((await screen.findByRole("alert")).textContent).toBe(t.avatarSaveFailed);
  expect(screen.getByRole("img", { name: t.myAvatar }).querySelector("img")!.getAttribute("src")).toBe("data:image/png;base64,OLD");
});
