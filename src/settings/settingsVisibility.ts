import { createContext, useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export const SettingsVisibilityContext = createContext(true);

/** One native visibility observer for the settings tree, including native hide. */
export function useSettingsWindowVisible() {
  const [visible, setVisible] = useState(() => !isTauri() && !document.hidden);
  useEffect(() => {
    let closed = false, revision = 0;
    const publish = (native: boolean) => { if (!closed) setVisible(native && !document.hidden); };
    const probe = () => {
      if (closed) return;
      if (document.hidden) publish(false);
      const ticket = ++revision;
      if (!isTauri()) { publish(true); return; }
      void invoke<boolean>("get_settings_visibility").then(value => {
        if (ticket === revision) publish(value);
      }, () => { if (ticket === revision) publish(false); });
    };
    const subscription = isTauri() ? listen<boolean>("deskmate://settings-visibility", event => {
      revision += 1; publish(event.payload);
    }) : Promise.resolve(() => {});
    void subscription.then(probe, probe);
    window.addEventListener("focus", probe);
    window.addEventListener("blur", probe);
    document.addEventListener("visibilitychange", probe);
    return () => {
      closed = true; revision += 1;
      window.removeEventListener("focus", probe); window.removeEventListener("blur", probe);
      document.removeEventListener("visibilitychange", probe);
      void subscription.then(off => off(), () => {});
    };
  }, []);
  return visible;
}
