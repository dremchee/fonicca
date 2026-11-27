import { StorageSerializers, useDraggable, useStorage } from '@vueuse/core'
import { computed, shallowRef, type Ref } from 'vue'

interface Position { x: number, y: number }

const keyOffsets: Partial<Record<string, Position>> = {
  ArrowLeft: { x: -16, y: 0 }, ArrowRight: { x: 16, y: 0 },
  ArrowUp: { x: 0, y: -16 }, ArrowDown: { x: 0, y: 16 },
}

export function useMiniPlayerPosition(panel: Ref<HTMLElement | null>) {
  const container = computed(() => panel.value?.parentElement)
  const saved = useStorage<Position | null>('fonicca-mini-player-position', null, undefined, {
    serializer: StorageSerializers.object,
  })
  const moving = shallowRef<Position | null>(null)
  const { isDragging } = useDraggable(panel, {
    containerElement: container,
    restrictInView: true,
    preventDefault: true,
    onStart: (_, event) => {
      if (!event.isPrimary || !(event.target instanceof Element)
        || event.target.closest('button, input, [data-player-control]')) return false
      panel.value?.setPointerCapture(event.pointerId)
    },
    onMove: position => { moving.value = position },
    onEnd: (_, event) => {
      if (event.type === 'pointerup' && moving.value) saved.value = moving.value
      moving.value = null
    },
  })

  const style = computed(() => {
    const position = moving.value ?? saved.value
    if (!position || !Number.isFinite(position.x) || !Number.isFinite(position.y)) return
    return { '--mini-player-x': `${position.x}px`, '--mini-player-y': `${position.y}px` }
  })

  function moveWithKeys(event: KeyboardEvent) {
    if (event.key === 'Home') {
      event.preventDefault()
      saved.value = null
      return
    }
    const delta = keyOffsets[event.key]
    if (!delta || !panel.value || !container.value) return
    event.preventDefault()
    const box = panel.value.getBoundingClientRect()
    const bounds = container.value.getBoundingClientRect()
    saved.value = { x: box.left - bounds.left + delta.x, y: box.top - bounds.top + delta.y }
  }

  return { isDragging, style, moveWithKeys }
}
