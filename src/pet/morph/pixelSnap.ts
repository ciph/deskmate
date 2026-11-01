/** Design units per glasses cell; must match MorphSignature. */
export const GLASSES_CELL = 6;

export interface ScreenGrid {
  /** CSS pixels per SVG user unit. */
  readonly scale: number;
  /** CSS pixel position of the SVG user-space origin. */
  readonly x: number;
  readonly y: number;
  /** Device pixels per CSS pixel. */
  readonly ratio: number;
}

/**
 * Keeps the pixel glasses on whole device pixels under any system scale.
 * Each cell is resized to an integer number of device pixels and the glasses'
 * origin is nudged (at most half a device pixel) onto the device grid. Soft
 * parts of the face keep their unsnapped, smooth translation.
 */
export function glassesTransform(grid: ScreenGrid | null, face: readonly number[]): string {
  if (!grid || !(grid.scale > 0) || !(grid.ratio > 0)) return "";
  const [faceX = 0, faceY = 0] = face;
  const unit = grid.scale * grid.ratio;
  const cell = Math.max(1, Math.round(GLASSES_CELL * unit));
  const snap = (origin: number, offset: number) => {
    const device = (origin + offset * grid.scale) * grid.ratio;
    return (Math.round(device) - device) / unit;
  };
  const r = (v: number) => Number(v.toFixed(4));
  return `translate(${r(snap(grid.x, faceX))} ${r(snap(grid.y, faceY))}) scale(${r(cell / (GLASSES_CELL * unit))})`;
}
