import type { Window } from "@tauri-apps/api/window";

type GeometryWindow = Pick<Window, "outerPosition" | "scaleFactor" | "onMoved" | "onResized" | "onScaleChanged">;
export interface PetWindowGeometry { readonly x: number; readonly y: number; readonly ratio: number }

// Share one native position/DPI read until a window event invalidates it. A moved
// window must never apply the cursor or geometry response from its old position.
export function createPetWindowGeometry(petWindow: GeometryWindow, report: (error: unknown) => void) {
  let generation = 0;
  let disposed = false;
  let cached: PetWindowGeometry | null = null;
  let pending: Promise<PetWindowGeometry | null> | null = null;
  const invalidate = () => { generation += 1; cached = null; };
  const subscriptions = [petWindow.onMoved(invalidate), petWindow.onResized(invalidate), petWindow.onScaleChanged(invalidate)];
  const ready = Promise.all(subscriptions);
  void ready.catch(report);
  return {
    get generation() { return generation; },
    read(): Promise<PetWindowGeometry | null> {
      if (disposed) return Promise.resolve(null);
      if (cached !== null) return Promise.resolve(cached);
      if (pending !== null) return pending;
      const token = generation;
      pending = ready.then(async () => {
        if (disposed || token !== generation) return null;
        const [position, ratio] = await Promise.all([petWindow.outerPosition(), petWindow.scaleFactor()]);
        if (disposed || token !== generation) return null;
        if (!Number.isFinite(position.x) || !Number.isFinite(position.y) || !Number.isFinite(ratio) || ratio <= 0) throw new Error("Invalid pet window geometry");
        cached = { x: position.x, y: position.y, ratio };
        return cached;
      }).finally(() => { pending = null; });
      return pending;
    },
    dispose() {
      disposed = true;
      invalidate();
      for (const subscription of subscriptions) void subscription.then((stop) => stop()).catch(report);
    },
  };
}
