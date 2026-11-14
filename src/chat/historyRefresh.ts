/** One in-flight read, trailing invalidation, and bounded recovery polling. */
export function historyRefresh<T>(options: {
  visible(): boolean;
  read(): Promise<T | null>;
  apply(value: T): void;
  active: boolean;
}) {
  let disposed = false;
  let loading = false;
  let dirty = false;
  let debounce: ReturnType<typeof setTimeout> | undefined;
  const visible = () => !disposed && options.visible() && document.visibilityState !== "hidden";
  const load = async () => {
    if (!visible()) return;
    if (loading) { dirty = true; return; }
    dirty = false;
    loading = true;
    try {
      const value = await options.read();
      if (!disposed && visible() && value !== null) options.apply(value);
    } catch {
      // A failed observation is retried by the next event or recovery tick.
    } finally {
      loading = false;
      if (!disposed && dirty && visible()) invalidate();
    }
  };
  const invalidate = () => {
    dirty = true;
    if (!visible() || debounce !== undefined) return;
    debounce = globalThis.setTimeout(() => { debounce = undefined; void load(); }, 250);
  };
  const resume = () => { if (visible()) void load(); };
  document.addEventListener("visibilitychange", resume);
  const timer = options.active ? globalThis.setInterval(() => void load(), 30_000) : undefined;
  return { load, invalidate, dispose() {
    disposed = true;
    document.removeEventListener("visibilitychange", resume);
    globalThis.clearTimeout(debounce);
    globalThis.clearInterval(timer);
  } };
}
