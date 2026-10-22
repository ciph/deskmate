import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { act, cleanup, renderHook } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockConvertFileSrc, mockIPC } from "@tauri-apps/api/mocks";
import type { InstalledPack } from "../lib/packs";
import { restoreTauriModuleFixture } from "../testing/tauriModuleFixture";
import { usePersonaAvatars } from "./usePersonaAvatar";

const installed: InstalledPack[] = [{
  packId: "aki", version: "1", personaIds: ["changli", "jinxi"],
  thumbnailPath: "C:/fixture/packs/aki/cover.png",
}];
const assetUrl = (path: string) => `http://asset.localhost/${encodeURIComponent(path)}`;
let listCalls = 0;
let loadPacks: () => Promise<InstalledPack[]>;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

beforeEach(() => {
  restoreTauriModuleFixture();
  listCalls = 0;
  loadPacks = async () => installed;
  mockIPC((command, payload) => {
    if (command === "installed_packs") { listCalls += 1; return loadPacks(); }
    if (command === "plugin:path|resolve_directory") return "C:/fixture";
    if (command === "plugin:path|join") return (payload as { paths: string[] }).paths.join("/");
    throw new Error(`Unexpected avatar command: ${command}`);
  }, { shouldMockEvents: true });
  mockConvertFileSrc("windows");
});

afterEach(async () => {
  cleanup();
  await act(async () => {});
  clearMocks();
});

describe("usePersonaAvatars", () => {
  test("keeps built-in record and array references for equivalent reordered and duplicate ID lists", () => {
    const { result, rerender } = renderHook(({ ids }: { ids: readonly string[] }) => usePersonaAvatars(ids), {
      initialProps: { ids: ["xiaozhu", "xiaozhu-nidaime", "xiaozhu"] },
    });
    const first = result.current;
    expect(Object.keys(first)).toEqual(["xiaozhu", "xiaozhu-nidaime"]);
    expect(first.xiaozhu?.[0]).toBe("/persona-avatars/xiaozhu.png");
    expect(first["xiaozhu-nidaime"]?.[0]).toBe("/persona-avatars/xiaozhu-nidaime.png");
    for (let update = 0; update < 20; update += 1) {
      rerender({ ids: ["xiaozhu-nidaime", "xiaozhu", "xiaozhu-nidaime"] });
      expect(result.current).toBe(first);
      expect(result.current.xiaozhu).toBe(first.xiaozhu);
      expect(result.current["xiaozhu-nidaime"]).toBe(first["xiaozhu-nidaime"]);
    }
    expect(listCalls).toBe(0);
    rerender({ ids: ["xiaozhu-sandaime"] });
    expect(result.current).not.toBe(first);
    expect(Object.keys(result.current)).toEqual(["xiaozhu-sandaime"]);
    expect(result.current["xiaozhu-sandaime"]?.[0]).toBe("/persona-avatars/xiaozhu-sandaime.png");
    expect(listCalls).toBe(0);
  });

  test("keeps pending and loaded imported references on equivalent input and replaces the changed persona key", async () => {
    const firstLoad = deferred<InstalledPack[]>();
    loadPacks = () => firstLoad.promise;
    const { result, rerender } = renderHook(({ ids }: { ids: readonly string[] }) => usePersonaAvatars(ids), {
      initialProps: { ids: ["changli", "xiaozhu"] },
    });
    const pending = result.current;
    expect(pending.changli).toEqual([]);
    rerender({ ids: ["xiaozhu", "changli", "changli"] });
    expect(result.current).toBe(pending);
    expect(listCalls).toBe(1);
    await act(async () => { firstLoad.resolve(installed); });
    const loaded = result.current;
    expect(loaded).not.toBe(pending);
    expect(loaded.changli).toEqual([
      assetUrl("C:/fixture/packs/aki/personas/changli/avatar.png"),
      assetUrl("C:/fixture/packs/aki/cover.png"),
    ]);
    rerender({ ids: ["changli", "xiaozhu", "changli"] });
    expect(result.current).toBe(loaded);
    expect(result.current.changli).toBe(loaded.changli);
    expect(result.current.xiaozhu).toBe(loaded.xiaozhu);
    expect(listCalls).toBe(1);
    const secondLoad = deferred<InstalledPack[]>();
    loadPacks = () => secondLoad.promise;
    rerender({ ids: ["jinxi"] });
    expect(Object.keys(result.current)).toEqual(["jinxi"]);
    expect(result.current.jinxi).toEqual([]);
    expect(listCalls).toBe(2);
    await act(async () => { secondLoad.resolve(installed); });
    expect(result.current.jinxi?.[0]).toBe(assetUrl("C:/fixture/packs/aki/personas/jinxi/avatar.png"));
    expect(result.current.changli).toBeUndefined();
  });

  test("refreshes installed portraits with the import digest and then keeps the refreshed references stable", async () => {
    const { result, rerender } = renderHook(({ ids }: { ids: readonly string[] }) => usePersonaAvatars(ids), {
      initialProps: { ids: ["changli"] },
    });
    await act(async () => {});
    const initial = result.current;
    const digest = "a".repeat(64);
    await act(async () => { await emit("deskmate://pack-imported", { packId: "aki", sha256: digest }); });
    const refreshed = result.current;
    expect(listCalls).toBe(2);
    expect(refreshed).not.toBe(initial);
    expect(refreshed.changli).not.toBe(initial.changli);
    expect(refreshed.changli).toEqual(initial.changli!.map(source => `${source}?avatarVersion=${digest}`));
    rerender({ ids: ["changli", "changli"] });
    expect(result.current).toBe(refreshed);
    expect(result.current.changli).toBe(refreshed.changli);
    expect(listCalls).toBe(2);
  });

  test("an older import request cannot replace the latest digest or its source references", async () => {
    const { result } = renderHook(() => usePersonaAvatars(["changli"]));
    await act(async () => {});
    const oldLoad = deferred<InstalledPack[]>();
    const newLoad = deferred<InstalledPack[]>();
    loadPacks = () => oldLoad.promise;
    await act(async () => { await emit("deskmate://pack-imported", { packId: "aki", sha256: "b".repeat(64) }); });
    loadPacks = () => newLoad.promise;
    await act(async () => { await emit("deskmate://pack-imported", { packId: "aki", sha256: "c".repeat(64) }); });
    await act(async () => { newLoad.resolve([{ ...installed[0]!, thumbnailPath: "C:/fixture/new-cover.png" }]); });
    const latest = result.current;
    expect(latest.changli?.[0]).toEndWith(`?avatarVersion=${"c".repeat(64)}`);
    expect(latest.changli?.[1]).toBe(`${assetUrl("C:/fixture/new-cover.png")}?avatarVersion=${"c".repeat(64)}`);
    await act(async () => { oldLoad.resolve(installed); });
    expect(result.current).toBe(latest);
    expect(result.current.changli).toBe(latest.changli);
    expect(listCalls).toBe(3);
  });

  test("switching personas ignores a late result for the old key", async () => {
    const oldLoad = deferred<InstalledPack[]>();
    const newLoad = deferred<InstalledPack[]>();
    loadPacks = () => oldLoad.promise;
    const { result, rerender } = renderHook(({ ids }: { ids: readonly string[] }) => usePersonaAvatars(ids), {
      initialProps: { ids: ["changli"] },
    });
    loadPacks = () => newLoad.promise;
    rerender({ ids: ["jinxi"] });
    await act(async () => { newLoad.resolve(installed); });
    const latest = result.current;
    await act(async () => { oldLoad.resolve(installed); });
    expect(result.current).toBe(latest);
    expect(Object.keys(result.current)).toEqual(["jinxi"]);
    expect(result.current.jinxi?.[0]).toBe(assetUrl("C:/fixture/packs/aki/personas/jinxi/avatar.png"));
    expect(listCalls).toBe(2);
  });

  test("unmount removes the import listener and a late load cannot render again", async () => {
    const pending = deferred<InstalledPack[]>();
    loadPacks = () => pending.promise;
    let renders = 0;
    const { unmount } = renderHook(() => { renders += 1; return usePersonaAvatars(["changli"]); });
    await act(async () => {});
    const mountedRenders = renders;
    unmount();
    await act(async () => {});
    await act(async () => { await emit("deskmate://pack-imported", { packId: "aki", sha256: "d".repeat(64) }); });
    expect(listCalls).toBe(1);
    await act(async () => { pending.resolve(installed); });
    expect(renders).toBe(mountedRenders);
    expect(listCalls).toBe(1);
  });

  test("failed loading has a stable empty fallback and a later import can recover", async () => {
    loadPacks = async () => { throw new Error("Pack index unavailable"); };
    const { result, rerender } = renderHook(({ ids }: { ids: readonly string[] }) => usePersonaAvatars(ids), {
      initialProps: { ids: ["changli"] },
    });
    await act(async () => {});
    const fallback = result.current;
    expect(fallback.changli).toEqual([]);
    rerender({ ids: ["changli", "changli"] });
    expect(result.current).toBe(fallback);
    expect(result.current.changli).toBe(fallback.changli);
    loadPacks = async () => installed;
    await act(async () => { await emit("deskmate://pack-imported", { packId: "aki", sha256: "e".repeat(64) }); });
    expect(result.current.changli?.[0]).toBe(`${assetUrl("C:/fixture/packs/aki/personas/changli/avatar.png")}?avatarVersion=${"e".repeat(64)}`);
    expect(listCalls).toBe(2);
  });
});
