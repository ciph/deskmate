import { expect, test } from "bun:test";
import { builtinPersonaAvatar, personaAvatarSources } from "./personaAvatar";
const host = { appDataDir: async () => "/data", join: async (...parts: string[]) => parts.join("/"), convertFileSrc: (path: string) => "asset:" + path };
test("bundled generations use distinct portraits and unknown IDs fall back safely", () => {
  expect(new Set(["xiaozhu", "xiaozhu-nidaime", "xiaozhu-sandaime"].map(id => builtinPersonaAvatar(id)[0])).size).toBe(3);
  expect(builtinPersonaAvatar("../../bad")).toEqual([]);
  expect(builtinPersonaAvatar("pixel-glasses-chibi")).toEqual(builtinPersonaAvatar("xiaozhu"));
});
test("imported avatar is scoped to its installed pack and precedes the pack cover", async () => {
  const installed = [{ packId: "aki", version: "1", personaIds: ["changli"], thumbnailPath: "/data/packs/aki/personas/changli/cover.png" }];
  expect(await personaAvatarSources("changli", installed, host)).toEqual(["asset:/data/packs/aki/personas/changli/avatar.png", "asset:/data/packs/aki/personas/changli/cover.png"]);
  expect(await personaAvatarSources("changli", [], host)).toEqual([]);
  expect(await personaAvatarSources("jinxi", installed, host)).toEqual([]);
});
