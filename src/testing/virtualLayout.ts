/** Happy DOM has no layout engine. Supply deterministic dimensions, not a fake virtualizer. */
export function virtualLayout() {
  const height = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetHeight");
  const width = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetWidth");
  Object.defineProperty(HTMLElement.prototype, "offsetHeight", { configurable: true, get() { return this.classList.contains("chat-virtual-row") ? 180 : 480; } });
  Object.defineProperty(HTMLElement.prototype, "offsetWidth", { configurable: true, get() { return 420; } });
  return () => {
    if (height) Object.defineProperty(HTMLElement.prototype, "offsetHeight", height); else Reflect.deleteProperty(HTMLElement.prototype, "offsetHeight");
    if (width) Object.defineProperty(HTMLElement.prototype, "offsetWidth", width); else Reflect.deleteProperty(HTMLElement.prototype, "offsetWidth");
  };
}
