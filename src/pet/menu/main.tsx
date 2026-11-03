import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RadialMenu } from "./RadialMenu";
import { MENU_UPDATE, type RadialModel } from "./model";

function PetMenuWindow() {
  const [model, setModel] = useState<RadialModel | null>(null);
  const finished = useRef(false);
  useEffect(() => {
    let disposed = false;
    const apply = (next: RadialModel | null) => {
      if (!disposed && next) setModel(current => current?.id === next.id && current.revision > next.revision ? current : next);
    };
    const listener = listen<RadialModel>(MENU_UPDATE, event => apply(event.payload));
    void listener.then(() => invoke<RadialModel | null>("get_pet_radial_menu")).then(apply).catch(console.error);
    return () => { disposed = true; void listener.then(stop => stop()); };
  }, []);
  const id = model?.id;
  useEffect(() => {
    if (!id) return;
    finished.current = false;
    void invoke("pet_radial_menu_ready", { id }).catch(console.error);
  }, [id]);
  return model && <RadialMenu key={model.id} model={model} onFinish={action => {
    if (finished.current) return;
    finished.current = true;
    void invoke("finish_pet_radial_menu", { id: model.id, action }).catch(error => { finished.current = false; console.error(error); });
  }} />;
}
createRoot(document.getElementById("root")!).render(<PetMenuWindow />);
