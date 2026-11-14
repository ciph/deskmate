import { createContext } from "react";
/** Native anchor clicks (including keyboard activation) can reveal another section. */
export const MarkdownAnchorContext = createContext<(href: string) => boolean>(() => false);
