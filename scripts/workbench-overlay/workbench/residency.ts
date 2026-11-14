type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>
function retryDelay(signal: AbortSignal) {
  return new Promise<void>((resolve, reject) => {
    if (signal.aborted) { reject(signal.reason); return }
    const abort = () => { clearTimeout(timer); reject(signal.reason) }
    const timer = setTimeout(() => { signal.removeEventListener("abort", abort); resolve() }, 1000)
    signal.addEventListener("abort", abort, { once: true })
  })
}
/** Hold admission until the response body ends; a hidden retired view waits for reopening. */
export async function residentFetch(invoke: Invoke, input: RequestInfo | URL, init?: RequestInit, transport: typeof fetch = fetch): Promise<Response> {
  const request = new Request(input, init)
  const url = new URL(request.url)
  const header = request.headers.get("x-opencode-directory")
  let headerDirectory = header
  if (header) { try { headerDirectory = decodeURIComponent(header) } catch { /* server validates malformed headers */ } }
  // Global events contain no directory instance and must remain connected after disposal.
  if (url.pathname === "/global/event" || url.pathname === "/global/health") return transport(input, init)
  let token: string
  for (;;) {
    try {
      token = await invoke<string>("sidecar_request_begin", {
        directory: url.searchParams.get("directory") ?? headerDirectory,
        background: request.method === "GET" || request.method === "HEAD",
      })
      break
    } catch (error) {
      if (String(error) !== "sidecar_instance_retired") throw error
      await retryDelay(request.signal)
    }
  }
  let released = false
  const release = () => { if (!released) { released = true; void invoke("sidecar_request_end", { token }).catch(() => undefined) } }
  try {
    const response = await transport(input, init)
    // Passive scoped events may sleep with a hidden idle view. Global disposal closes
    // their instance; reconnect admission waits until the view is shown again.
    if (url.pathname === "/event") { release(); return response }
    if (!response.body) { release(); return response }
    const reader = response.body.getReader()
    const body = new ReadableStream<Uint8Array>({
      async pull(controller) {
        try {
          const result = await reader.read()
          if (result.done) { release(); controller.close() } else controller.enqueue(result.value)
        } catch (error) { release(); controller.error(error) }
      },
      async cancel(reason) { try { await reader.cancel(reason) } finally { release() } },
    })
    return new Response(body, { status: response.status, statusText: response.statusText, headers: response.headers })
  } catch (error) { release(); throw error }
}
