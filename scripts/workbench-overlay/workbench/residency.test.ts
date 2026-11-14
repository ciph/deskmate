import { expect, test } from "bun:test"
import { residentFetch } from "./residency"
test("encoded SDK directory headers are admitted using the same decoded directory as the engine", async () => {
  let directory: unknown
  const invoke = async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
    if (command === "sidecar_request_begin") directory = args?.directory
    return "lease" as T
  }
  const response = await residentFetch(invoke, "http://127.0.0.1:99/session", { method: "POST", headers: { "x-opencode-directory": encodeURIComponent("C:/中文 project") } }, (async () => new Response("ok")) as typeof fetch)
  await response.text()
  expect(directory).toBe("C:/中文 project")
})
test("admission lasts through response body, with exactly one release on completion or cancellation", async () => {
  for (const cancel of [false, true]) {
    const commands: string[] = []
    const invoke = async <T>(command: string): Promise<T> => { commands.push(command); return "lease" as T }
    const transport = (async () => new Response("complete body")) as typeof fetch
    const response = await residentFetch(invoke, "http://127.0.0.1:99/session?directory=C%3A%2Fa", undefined, transport)
    expect(commands).toEqual(["sidecar_request_begin"])
    if (cancel) await response.body!.cancel()
    else expect(await response.text()).toBe("complete body")
    expect(commands).toEqual(["sidecar_request_begin", "sidecar_request_end"])
  }
})
test("failed HTTP transport releases admission; global events do not create a directory consumer", async () => {
  const commands: string[] = []
  const invoke = async <T>(command: string): Promise<T> => { commands.push(command); return "lease" as T }
  const failed = (async () => { throw new Error("network") }) as typeof fetch
  await expect(residentFetch(invoke, "http://127.0.0.1:99/session", undefined, failed)).rejects.toThrow("network")
  expect(commands).toEqual(["sidecar_request_begin", "sidecar_request_end"])
  commands.length = 0
  await residentFetch(invoke, "http://127.0.0.1:99/global/event", undefined, (async () => new Response("event")) as typeof fetch)
  expect(commands).toEqual([])
})
test("hidden retired reads wait without transport and remain cancellable", async () => {
  const controller = new AbortController()
  let calls = 0
  const invoke = async <T>(): Promise<T> => { controller.abort(new Error("cancelled")); throw "sidecar_instance_retired" }
  const transport = (async () => { calls++; return new Response("bad") }) as typeof fetch
  await expect(residentFetch(invoke, "http://127.0.0.1:99/session", { signal: controller.signal }, transport)).rejects.toThrow("cancelled")
  expect(calls).toBe(0)
})
