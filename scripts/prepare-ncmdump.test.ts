import { afterAll, afterEach, expect, spyOn, test } from "bun:test";
import { download } from "./prepare-ncmdump";

const primary = "https://github.com/taurusxin/ncmdump/releases/download/1.5.1/ncmdump-1.5.1-windows-amd64.zip";
const assetId = 300702365;
const fallback = `https://api.github.com/repos/taurusxin/ncmdump/releases/assets/${assetId}`;
const fetchSpy = spyOn(globalThis, "fetch");
const warning = spyOn(console, "warn").mockImplementation(() => {});
afterEach(() => { fetchSpy.mockReset(); warning.mockClear(); });
afterAll(() => { fetchSpy.mockRestore(); warning.mockRestore(); });

test("uses the original release bytes without another download when available", async () => {
  fetchSpy.mockResolvedValueOnce(new Response(new Uint8Array([1, 2, 3])));
  expect(await download(primary, assetId)).toEqual(new Uint8Array([1, 2, 3]));
  expect(fetchSpy).toHaveBeenCalledTimes(1);
});

test("a release URL 504 falls back once to the same pinned asset API", async () => {
  fetchSpy.mockResolvedValueOnce(new Response("gateway timeout", { status: 504 }));
  fetchSpy.mockResolvedValueOnce(new Response(new Uint8Array([4, 5, 6])));
  expect(await download(primary, assetId)).toEqual(new Uint8Array([4, 5, 6]));
  expect(fetchSpy).toHaveBeenCalledTimes(2);
  expect(fetchSpy.mock.calls[0][0]).toBe(primary);
  expect(fetchSpy.mock.calls[1][0]).toBe(fallback);
  for (const [, options] of fetchSpy.mock.calls) {
    expect(options?.headers).toEqual({ Accept: "application/octet-stream" });
    expect(options?.signal).toBeInstanceOf(AbortSignal);
  }
});

test("network and fallback failures remain fatal after two attempts", async () => {
  fetchSpy.mockRejectedValueOnce(new Error("connection reset"));
  fetchSpy.mockResolvedValueOnce(new Response("unavailable", { status: 503 }));
  await expect(download(primary, assetId)).rejects.toThrow(`${fallback}: 503`);
  expect(fetchSpy).toHaveBeenCalledTimes(2);
});
