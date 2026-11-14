import { useCallback, useEffect, useRef } from "react";

/** UI reads only: one request even across hide/show or provider changes. */
export function useVisiblePoll(task: (current: () => boolean) => Promise<void>, visible: boolean, interval: number, initialDelay = 0) {
  const running = useRef(false);
  const pending = useRef<(() => void) | null>(null);
  const request = useRef(() => {});
  const activated = useRef(false);
  const generation = useRef(0);
  useEffect(() => {
    if (!visible) { request.current = () => {}; return; }
    let active = true;
    let started = false;
    const refresh = () => {
      if (!active || !visible) return;
      clearTimeout(initialTimer);
      started = true;
      const ticket = ++generation.current;
      if (running.current) { pending.current = refresh; return; }
      running.current = true;
      void Promise.resolve().then(() => {
        if (active && ticket === generation.current) return task(() => active && ticket === generation.current);
      })
        .catch(() => {})
        .finally(() => {
          running.current = false;
          const next = pending.current; pending.current = null;
          if (next) next();
        });
    };
    request.current = refresh;
    const delay = activated.current ? 0 : initialDelay;
    activated.current = true;
    const initialTimer = setTimeout(refresh, delay);
    const pollTimer = setInterval(() => {
      // A timer tick never queues another read behind a slow request.
      if (started && !running.current) refresh();
    }, interval);
    if (delay === 0) refresh();
    return () => {
      active = false; generation.current += 1;
      clearTimeout(initialTimer); clearInterval(pollTimer);
      if (pending.current === refresh) pending.current = null;
    };
  }, [task, visible, interval, initialDelay]);
  return useCallback(() => request.current(), []);
}
