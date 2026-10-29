import { defaultRangeExtractor, type Range } from "@tanstack/react-virtual";

export function retainedRange(range: Range, pins: ReadonlySet<number>, selection: readonly [number, number] | null, all: boolean) {
  if (all) return Array.from({ length: range.count }, (_, index) => index);
  const result = new Set(defaultRangeExtractor(range));
  for (const pin of pins) if (pin >= 0 && pin < range.count) result.add(pin);
  if (selection) for (let i = Math.max(0, selection[0]); i <= Math.min(range.count - 1, selection[1]); i += 1) result.add(i);
  return [...result].sort((a, b) => a - b);
}
