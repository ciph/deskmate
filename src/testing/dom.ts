import "./tauriModuleFixture";
import { GlobalRegistrator } from "@happy-dom/global-registrator";

// Component tests need a DOM before React renders. Bun runs this file once per
// test process via `bunfig.toml` [test].preload.
GlobalRegistrator.register();

// Happy DOM deliberately has no CSS layout. The real virtualizer needs a
// viewport to mount rows in ChatApp integration tests; unrelated elements keep
// their original dimensions and no production list implementation is mocked.
for (const property of ["offsetHeight", "offsetWidth"] as const) {
  const original = Object.getOwnPropertyDescriptor(HTMLElement.prototype, property);
  Object.defineProperty(HTMLElement.prototype, property, { configurable: true, get() {
    if (this.classList.contains("chat-virtual-row")) return property === "offsetHeight" ? 180 : 420;
    if (this.classList.contains("chat-virtual-list")) return property === "offsetHeight" ? 500 : 420;
    if (this.classList.contains("chat-navigation-rail")) return property === "offsetHeight" ? 320 : 28;
    return original?.get?.call(this) ?? 0;
  } });
}
