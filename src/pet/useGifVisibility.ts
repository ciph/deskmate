import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { flushSync } from "react-dom";
import { acknowledgePetVisibility, getPetVisibility, onPetNativeVisibility, onPetVisibility, registerPetVisibility } from "../lib/petVisibility";

export interface VisibilityClock {
  readonly schedule: (callback: () => void, delay: number) => number;
  readonly cancel: (token: number) => void;
}
const browserClock: VisibilityClock = {
  schedule: (callback, delay) => window.setTimeout(callback, delay),
  cancel: (token) => window.clearTimeout(token),
};

export function useGifVisibility(personaId: string, gif: boolean, duration: number, clock: VisibilityClock = browserClock) {
  const [departure, setDeparture] = useState({ personaId, leaving: false, visible: false, nativeVisible: false });
  const durationRef = useRef(duration);
  durationRef.current = duration;
  useLayoutEffect(() => {
    setDeparture((current) => ({ ...current, personaId, leaving: false, visible: false }));
  }, [personaId, gif]);
  useEffect(() => {
    let disposed = false;
    let eventObserved = false;
    let nativeEventObserved = false;
    let timer: number | null = null;
    const report = (error: unknown) => console.error("pet visibility unavailable", error instanceof Error ? error.message : String(error));
    const unlisten = onPetVisibility((event) => {
      if (disposed) return;
      eventObserved = true;
      if (timer !== null) clock.cancel(timer);
      flushSync(() => setDeparture((current) => ({ ...current, personaId, leaving: event.phase === "leaving", visible: event.phase === "reset" })));
      timer = clock.schedule(() => {
        timer = null;
        if (!disposed) void acknowledgePetVisibility(event.token).catch(report);
      }, event.phase === "leaving" && gif ? durationRef.current : 0);
    });
    const nativeUnlisten = onPetNativeVisibility((visible) => {
      if (disposed) return;
      nativeEventObserved = true;
      eventObserved = true;
      setDeparture((current) => ({ ...current, personaId, visible, nativeVisible: visible, leaving: visible ? false : current.leaving }));
    });
    void Promise.all([unlisten, nativeUnlisten]).then(async () => {
      if (disposed || personaId === "") return;
      await registerPetVisibility(personaId, gif);
      const [visible, nativeVisible] = await Promise.all([getPetVisibility(), getCurrentWindow().isVisible()]);
      if (!disposed) setDeparture((current) => ({
        ...current, personaId,
        ...(!eventObserved ? { leaving: false, visible } : {}),
        ...(!nativeEventObserved ? { nativeVisible: nativeVisible === true } : {}),
      }));
    }).catch(report);
    return () => {
      disposed = true;
      if (timer !== null) clock.cancel(timer);
      void unlisten.then((stop) => stop()).catch(report);
      void nativeUnlisten.then((stop) => stop()).catch(report);
    };
  }, [personaId, gif, clock]);
  return { leaving: departure.personaId === personaId && departure.leaving, visible: departure.visible, nativeVisible: departure.nativeVisible };
}
