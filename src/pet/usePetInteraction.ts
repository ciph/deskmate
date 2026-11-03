import { useCallback, useEffect, useLayoutEffect, useRef, type KeyboardEvent, type MouseEvent, type RefObject } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { pointInPetContent } from "./gifGeometry";

const report = (error: unknown) => console.error("pet interaction failed", error instanceof Error ? error.message : String(error));
type InteractionSurface = { readonly onActivate?: () => void; readonly onDragChange?: (dragging: boolean) => void; readonly onHeldChange?: (held: boolean) => void; readonly onInteract?: () => void; readonly gif: boolean; readonly active: boolean; readonly identity: string; readonly root: RefObject<HTMLDivElement | null> };
export function usePetInteraction(surface: InteractionSurface, setLocked: (locked: boolean) => void) {
  const callbacks = useRef(surface);
  useLayoutEffect(() => { callbacks.current = surface; });
  const downAt = useRef<{ readonly x: number; readonly y: number; readonly t: number } | null>(null);
  const holding = useRef(false);
  const dragging = useRef(false);
  const persistentHold = useRef(false);
  const epoch = useRef(0);
  const reset = useCallback(() => { const wasHeld = holding.current || downAt.current !== null; epoch.current += 1; downAt.current = null; holding.current = false; persistentHold.current = false; setLocked(false); if (dragging.current) { dragging.current = false; callbacks.current.onDragChange?.(false); } if (wasHeld) callbacks.current.onHeldChange?.(false); }, [setLocked]);
  useEffect(() => {
    reset();
    // Native dragging can cancel WebView input; its physical-button poll owns release.
    const cancelPointer = () => { if (!dragging.current && !persistentHold.current) reset(); };
    window.addEventListener("blur", cancelPointer);
    window.addEventListener("pointercancel", cancelPointer);
    const mouseup = () => { if (!holding.current) reset(); };
    window.addEventListener("mouseup", mouseup);
    const hidden = () => { if (document.hidden) reset(); };
    document.addEventListener("visibilitychange", hidden);
    return () => { reset(); window.removeEventListener("blur", cancelPointer); window.removeEventListener("pointercancel", cancelPointer); window.removeEventListener("mouseup", mouseup); document.removeEventListener("visibilitychange", hidden); };
  }, [reset, surface.active, surface.identity]);
  const hit = (event: MouseEvent) => surface.active && (!surface.gif || (surface.root.current !== null && pointInPetContent(surface.root.current, event.clientX, event.clientY)));
  const hold = (persistOnBlur = false) => { persistentHold.current = persistOnBlur; epoch.current += 1; holding.current = true; setLocked(true); callbacks.current.onHeldChange?.(true); const generation = epoch.current; return () => { if (generation === epoch.current) reset(); }; };
  const activate = () => { callbacks.current.onInteract?.(); if (callbacks.current.onActivate) callbacks.current.onActivate(); else void invoke("toggle_chat").catch(report); };
  return {
    hit, hold,
    onKeyDown: (event: KeyboardEvent) => {
      if (!surface.active || event.repeat || (event.key !== "Enter" && event.key !== " ")) return;
      event.preventDefault(); activate();
    },
    onMouseDown: (event: MouseEvent) => {
      if (event.button !== 0 || !hit(event)) return;
      downAt.current = { x: event.screenX, y: event.screenY, t: Date.now() };
      setLocked(true); callbacks.current.onHeldChange?.(true);
    },
    onMouseMove: (event: MouseEvent) => {
      const start = downAt.current;
      if (start === null || Math.hypot(event.screenX - start.x, event.screenY - start.y) <= 4) return;
      downAt.current = null;
      const release = hold();
      dragging.current = true; callbacks.current.onDragChange?.(true);
      const generation = epoch.current;
      void (async () => {
        await getCurrentWindow().startDragging();
        while (generation === epoch.current) {
          const pressed = await invoke<boolean>("pet_primary_button_down");
          if (!pressed || generation !== epoch.current) return;
          await new Promise<void>((resolve) => window.setTimeout(resolve, 40));
        }
      })().catch(report).finally(release);
    },
    onMouseUp: (event: MouseEvent) => {
      if (holding.current) return;
      const start = downAt.current;
      reset();
      if (event.button === 0 && start !== null && Date.now() - start.t < 400 && hit(event)) activate();
    },
  };
}
