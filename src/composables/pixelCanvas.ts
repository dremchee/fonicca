import type { Ref } from 'vue'
import { computed, shallowRef } from 'vue'
import { useDevicePixelRatio, useElementBounding, useResizeObserver } from '@vueuse/core'
import { alignDevicePixel } from '@/pixel-grid'

/** Keep CSS canvas bounds, its backing bitmap and overlays on the same device grid. */
export function usePixelCanvas(target: Ref<HTMLElement | null>) {
  const size = shallowRef({ width: 0, height: 0 })
  const bounds = useElementBounding(target)
  const { pixelRatio } = useDevicePixelRatio()
  useResizeObserver(target, ([entry]) => {
    if (entry) size.value = { width: entry.contentRect.width, height: entry.contentRect.height }
  })
  // Splitters move a canvas even when its own size stays the same.
  useResizeObserver(() => target.value?.parentElement, bounds.update)
  const geometry = computed(() => {
    const ratio = pixelRatio.value || 1
    const left = alignDevicePixel(0, bounds.left.value, ratio)
    const top = alignDevicePixel(0, bounds.top.value, ratio)
    return {
      left, top, ratio,
      width: Math.max(0, alignDevicePixel(size.value.width, bounds.left.value, ratio) - left),
      height: Math.max(0, alignDevicePixel(size.value.height, bounds.top.value, ratio) - top),
    }
  })
  const style = computed(() => ({
    left: `${geometry.value.left}px`, top: `${geometry.value.top}px`,
    width: `${geometry.value.width}px`, height: `${geometry.value.height}px`,
  }))
  const snapX = (value: number) => alignDevicePixel(value, bounds.left.value, geometry.value.ratio)
  const snapY = (value: number) => alignDevicePixel(value, bounds.top.value, geometry.value.ratio)
  const localPixel = (value: number) => Math.round(value * geometry.value.ratio) / geometry.value.ratio
  return { geometry, style, snapX, snapY, localPixel, pixelRatio }
}
