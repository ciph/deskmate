import { useEffect, useRef, type RefObject } from "react";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";
import type { PetMood } from "../lib/petState";
import type { Settings } from "../lib/settings";
import { PetRenderer } from "./PetRenderer";
import { createPetWindowGeometry } from "./petWindowGeometry";

interface GlbPetViewProps {
  readonly mood: PetMood;
  readonly visible?: boolean;
  readonly settings: Settings;
  readonly width: number;
  readonly height: number;
  readonly rendererRef: RefObject<PetRenderer | null>;
  readonly onReady?: (ready: boolean) => void;
  readonly onError: (message: string | null) => void;
}

export function GlbPetView({ settings, mood, visible = true, width, height, rendererRef, onError, onReady }: GlbPetViewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null) return;
    let renderer: PetRenderer;
    try { renderer = new PetRenderer(canvas); }
    catch (error: unknown) { onError(error instanceof Error ? error.message : String(error)); return; }
    rendererRef.current = renderer;
    return () => {
      rendererRef.current = null;
      renderer.dispose();
    };
  }, [rendererRef, onError]);
  useEffect(() => { rendererRef.current?.setVisible(visible); }, [visible, rendererRef]);
  useEffect(() => { rendererRef.current?.setMood(mood); }, [mood, rendererRef]);
  useEffect(() => {
    const renderer = rendererRef.current;
    if (renderer === null) return;
    let disposed = false;
    onError(null);
    onReady?.(false);
    void renderer.load(settings.personaId).then(() => { if (!disposed) onReady?.(true); }).catch((error: unknown) => {
      if (!disposed) onError(error instanceof Error ? error.message : String(error));
    });
    return () => { disposed = true; };
  }, [settings.personaId, rendererRef, onError, onReady]);
  useEffect(() => {
    rendererRef.current?.setRenderTuning(settings);
    rendererRef.current?.setMouseFollowEnabled(settings.mouseFollow);
  }, [settings, rendererRef]);
  useEffect(() => {
    const renderer = rendererRef.current;
    const canvas = canvasRef.current;
    if (!visible || !settings.mouseFollow || renderer === null || canvas === null) return;
    const petWindow = getCurrentWindow();
    let disposed = false;
    let busy = false;
    const report = (error: unknown) => console.error("pet cursor unavailable", error instanceof Error ? error.message : String(error));
    const geometry = createPetWindowGeometry(petWindow, report);
    const poll = async () => {
      if (disposed || busy) return;
      busy = true;
      const generation = geometry.generation;
      try {
        const origin = await geometry.read();
        if (disposed || origin === null || generation !== geometry.generation) return;
        const cursor = await cursorPosition();
        if (disposed || generation !== geometry.generation) return;
        const bounds = canvas.getBoundingClientRect();
        renderer.setMouseTarget({
          x: ((cursor.x - origin.x) / origin.ratio - bounds.left - bounds.width / 2) / Math.max(bounds.width / 2, 1),
          y: ((cursor.y - origin.y) / origin.ratio - bounds.top - bounds.height / 2) / Math.max(bounds.height / 2, 1),
        });
      } catch (error: unknown) { report(error); }
      finally { busy = false; }
    };
    void poll();
    const interval = window.setInterval(() => { void poll(); }, 40);
    return () => { disposed = true; geometry.dispose(); window.clearInterval(interval); };
  }, [visible, settings.mouseFollow, rendererRef]);
  return <canvas ref={canvasRef} aria-label="3D desktop pet" style={{ width, height, display: "block" }} />;
}
