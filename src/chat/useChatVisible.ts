import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export function useChatVisible() {
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    let closed = false;
    let revision = 0;
    const subscription = listen<boolean>("deskmate://chat-visibility", event => {
      revision += 1;
      if (!closed) setVisible(event.payload);
    });
    void subscription.then(async () => {
      const ticket = revision;
      const current = await invoke<boolean>("get_chat_visibility");
      if (!closed && ticket === revision) setVisible(current);
    }).catch(() => {});
    return () => { closed = true; void subscription.then(off => off()).catch(() => {}); };
  }, []);
  return visible;
}
