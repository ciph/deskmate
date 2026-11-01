import { render as renderDom, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";

/** Worker rendering is asynchronous; assertions wait for the exact final version. */
export async function renderMarkdown(node: ReactNode) {
  const view = renderDom(node);
  const settled = () => waitFor(() => {
    if (view.container.querySelector('[data-format-pending="true"]')) throw new Error("Markdown still pending");
  });
  await settled();
  return { ...view, rerender: async (next: ReactNode) => { view.rerender(next); await settled(); } };
}
