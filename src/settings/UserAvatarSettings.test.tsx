import { afterEach, expect, mock, test } from "bun:test";
import * as core from "@tauri-apps/api/core";
import * as event from "@tauri-apps/api/event";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { legacySettingsFixture } from "../testing/settingsFixtures";
import { dict } from "../lib/i18n";
import type { Settings } from "../lib/settings";
const t = dict("zh-CN");
const avatar = "data:image/png;base64,AAAA";
let stored = legacySettingsFixture();
let failure = false;
const invoke = mock(async (command: string, args?: { settings: Settings }) => {
  if (command === "get_settings") return { ...stored };
  if (command === "get_pet_visibility_error") return null;
  if (command === "app_version") return "0.4.18";
  if (command === "set_settings") {
    if (failure) throw new Error("disk full");
    stored = { ...args!.settings };
  }
});
mock.module("@tauri-apps/api/core", () => ({ ...core, invoke }));
mock.module("@tauri-apps/api/event", () => ({ ...event, listen: async () => () => {} }));
mock.module("../lib/userAvatar", () => ({ prepareUserAvatar: async () => avatar, userAvatarSource: (value: string | null) => value || undefined }));
const { default: SettingsApp } = await import("./SettingsApp");
afterEach(() => { cleanup(); stored = legacySettingsFixture(); failure = false; invoke.mockClear(); });

test("avatar saves immediately with pending general settings and survives remount", async () => {
  const view = render(<SettingsApp />);
  fireEvent.click(await screen.findByRole("checkbox", { name: t.chatLarge }));
  fireEvent.change(screen.getByLabelText(t.changeAvatar, { selector: "input" }), {
    target: { files: [new File(["a"], "a.png", { type: "image/png" })] },
  });
  await waitFor(() => expect(stored.userAvatar).toBe(avatar));
  expect(stored.chatLarge).toBe(true);
  expect(stored.theme).toBe(legacySettingsFixture().theme);
  view.unmount();
  render(<SettingsApp />);
  const preview = await screen.findByRole("img", { name: t.myAvatar });
  expect(preview.querySelector("img")!.getAttribute("src")).toBe(avatar);
});

test("failed native save reconciles with stored settings and keeps the old avatar", async () => {
  stored = legacySettingsFixture({ userAvatar: avatar });
  failure = true;
  render(<SettingsApp />);
  fireEvent.click(await screen.findByRole("button", { name: t.resetAvatar }));
  expect((await screen.findByRole("alert")).textContent).toBe(t.avatarSaveFailed);
  expect(screen.getByRole("img", { name: t.myAvatar }).querySelector("img")!.getAttribute("src")).toBe(avatar);
  expect(stored.userAvatar).toBe(avatar);
});
