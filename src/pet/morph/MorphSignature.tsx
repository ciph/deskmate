import type { Ref } from "react";
import { palette } from "./model.json";
import { GLASSES_CELL as CELL } from "./pixelSnap";

// One cell is 6 × 6 design units. # is black; o is a square reflection.
// The frame is mirrored; both reflections face the same light source.
const GLASSES_GRID = [
  "..########......########..",
  "###########....###########",
  "###o#############o########",
  ".########################.",
  ".##########....##########.",
  "..########......########..",
  "..########......########..",
  "...######........######...",
] as const;
function pixels(color: "#" | "o") {
  return GLASSES_GRID.flatMap((row, y) =>
    Array.from(row, (pixel, x) =>
      pixel === color
        ? `M${(x - 13) * CELL} ${(y - 4) * CELL}h${CELL}v${CELL}h-${CELL}z`
        : "",
    ),
  ).join("");
}
const frame = pixels("#"), reflection = pixels("o");

/** The four bodies and the exported avatar share this rigid face artwork. */
export function MorphSignature({ glasses }: { readonly glasses?: Ref<SVGGElement> }) {
  return (
    <g data-morph-signature="true">
      <g ref={glasses} data-pixel-glasses="true" shapeRendering="crispEdges">
        <path d={frame} fill={palette.ink} />
        <path d={reflection} fill={palette.body} />
      </g>
      <ellipse cx="-51" cy="35" rx="13" ry="8.5" fill={palette.pink} />
      <ellipse cx="51" cy="35" rx="13" ry="8.5" fill={palette.pink} />
    </g>
  );
}
