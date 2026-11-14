import { expect, test } from "bun:test"
import { readPickedAttachments } from "./attachments"

test("binary chunks preserve file bytes, name, path and callback order, releasing all tokens", async () => {
  const original = new Uint8Array(2 * 1024 * 1024 + 17).map((_, i) => i % 251)
  const released: string[][] = []
  const calls: number[] = []
  const invoke = async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
    if (command === "workbench_pick_files") return [{ name: "large.bin", path: "C:/chosen/large.bin", size: original.length, token: "a" }, { name: "empty", path: "C:/chosen/empty", size: 0, token: "b" }] as T
    if (command === "workbench_release_files") { released.push(args!.tokens as string[]); return undefined as T }
    expect(args!.token).toBe("a")
    expect(args).not.toHaveProperty("path")
    const offset = args!.offset as number
    calls.push(offset)
    return original.slice(offset, offset + 1024 * 1024).buffer as T
  }
  const received: string[] = []
  await readPickedAttachments(invoke, undefined, async (file, path) => {
    received.push(file.name)
    expect(path).toBe("C:/chosen/" + file.name)
    expect(new Uint8Array(await file.arrayBuffer())).toEqual(file.name === "empty" ? new Uint8Array() : original)
  })
  expect(calls).toEqual([0, 1048576, 2097152])
  expect(received).toEqual(["large.bin", "empty"])
  expect(released).toEqual([["a", "b"]])
})

test("read or callback failure releases unread selections too", async () => {
  for (const failure of ["read", "callback"]) {
    let released: unknown
    const invoke = async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
      if (command === "workbench_pick_files") return ["a", "b"].map(token => ({ name: token, path: token, size: 1, token })) as T
      if (command === "workbench_release_files") { released = args!.tokens; return undefined as T }
      if (failure === "read") throw new Error("read failed")
      return new ArrayBuffer(1) as T
    }
    await expect(readPickedAttachments(invoke, undefined, async () => { throw new Error("callback failed") })).rejects.toThrow(failure + " failed")
    expect(released).toEqual(["a", "b"])
  }
})
