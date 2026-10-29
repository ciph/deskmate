/** Fetch SSE response with explicit lifetime; opening history now awaits headers. */
export function eventStreamFixture(signal?: AbortSignal | null, receive?: (send: (event: object) => void) => void) {
  return new Response(new ReadableStream<Uint8Array>({ start(controller) {
    let closed = false;
    signal?.addEventListener("abort", () => { if (!closed) { closed = true; controller.close(); } }, { once: true });
    receive?.(event => { if (!closed) controller.enqueue(new TextEncoder().encode(`data: ${JSON.stringify(event)}\n\n`)); });
  } }), { headers: { "Content-Type": "text/event-stream" } });
}
