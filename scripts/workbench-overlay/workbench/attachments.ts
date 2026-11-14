interface PickedFile { name: string; path: string; size: number; token: string }
type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>

/** Binary IPC avoids base64/atob copies; only one file is assembled at a time. */
export async function readPickedAttachments(
  invoke: Invoke,
  options: { title?: string; extensions?: string[] } | undefined,
  onFile: (file: File, path: string) => Promise<void>,
) {
  const picked = await invoke<PickedFile[]>("workbench_pick_files", { title: options?.title, extensions: options?.extensions })
  try {
    for (const item of picked) {
      const parts: ArrayBuffer[] = []
      let offset = 0
      while (offset < item.size) {
        const chunk = await invoke<ArrayBuffer>("workbench_read_file", { token: item.token, offset })
        if (!(chunk instanceof ArrayBuffer) || chunk.byteLength === 0 || chunk.byteLength > 1024 * 1024 || offset + chunk.byteLength > item.size) {
          throw new Error("Invalid attachment chunk")
        }
        parts.push(chunk)
        offset += chunk.byteLength
      }
      const file = new File(parts, item.name)
      parts.length = 0
      await onFile(file, item.path)
    }
  } finally {
    await invoke("workbench_release_files", { tokens: picked.map(item => item.token) }).catch(() => undefined)
  }
}
