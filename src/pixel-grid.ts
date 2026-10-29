/** Snap a local coordinate using its absolute origin and display scale. */
export function alignDevicePixel(value: number, origin: number, ratio: number): number {
  return Math.round((origin + value) * ratio) / ratio - origin
}
