import { describe, expect, test } from "bun:test";
import { avatarCrop, prepareUserAvatar, userAvatarSource } from "./userAvatar";

describe("avatar image boundaries", () => {
  test("centers landscape and portrait crops without stretching", () => {
    expect(avatarCrop(600, 400)).toEqual({ x: 100, y: 0, size: 400 });
    expect(avatarCrop(400, 600)).toEqual({ x: 0, y: 100, size: 400 });
    expect(avatarCrop(128, 128)).toEqual({ x: 0, y: 0, size: 128 });
    for (const [w, h] of [[0, 100], [NaN, 10], [9000, 1], [8192, 8192]]) expect(() => avatarCrop(w, h)).toThrow();
  });
  test("does not render arbitrary URLs or oversized stored values", () => {
    expect(userAvatarSource("data:image/png;base64,AAAA")).toBeDefined();
    for (const value of [null, undefined, "https://example.com/a.png", "data:image/svg+xml;base64,AAAA", "data:image/png;base64,!!!!", "x".repeat(100000)]) expect(userAvatarSource(value)).toBeUndefined();
  });
  test("rejects unsupported and oversized files before decoding", async () => {
    await expect(prepareUserAvatar(new File(["svg"], "a.svg", { type: "image/svg+xml" }))).rejects.toThrow("avatarInvalid");
    await expect(prepareUserAvatar(new File([new Uint8Array(5 * 1024 * 1024 + 1)], "a.png", { type: "image/png" }))).rejects.toThrow("avatarTooLarge");
  });
});
