import type { ComputedRef, Ref } from 'vue'
import type { SAMPLE_COLUMNS } from '@/sample-columns'
import { useEventListener, useResizeObserver } from '@vueuse/core'
import { onScopeDispose, watch } from 'vue'
import { useSampleTable } from '@/composables/sampleTable'
import { sampleTableLayout } from '@/sample-columns'

export function useSampleColumnWidths(options: {
  columns: ComputedRef<readonly typeof SAMPLE_COLUMNS[number][]>
  viewport: Ref<HTMLElement | null>
  content: Ref<HTMLElement | null>
  enabled: () => boolean
}) {
  const { table, setViewportWidth } = useSampleTable()
  let viewportWidth = 0

  // Keep sizing reads outside SampleList's render effect. Drag frames update
  // CSS without asking Vue to patch the virtual rows and their waveforms.
  function writeLayout() {
    const element = options.content.value
    if (!element) return
    if (!options.enabled()) {
      element.style.removeProperty('--sample-columns')
      element.style.removeProperty('min-width')
      return
    }
    const layout = sampleTableLayout(options.columns.value, table.atoms.columnSizing.get(), viewportWidth)
    element.style.setProperty('--sample-columns', layout['--sample-columns'])
    element.style.minWidth = layout.minWidth
  }

  function writeResizeGuide() {
    const content = options.content.value
    const handle = content?.querySelector('.resize-handle:hover')
      ?? content?.querySelector('.resize-handle:focus-visible')
    const active = table.atoms.columnResizing.get().isResizingColumn
      || handle?.closest<HTMLElement>('.column-heading')?.dataset.column
    for (const guide of options.content.value?.querySelectorAll<HTMLElement>('.column-guide') ?? [])
      guide.classList.toggle('active', guide.dataset.column === active)
  }

  useEventListener(options.content, ['pointerover', 'pointerout', 'focusin', 'focusout', 'keydown'], (event) => {
    if (event.target instanceof Element && event.target.closest('.resize-handle')) writeResizeGuide()
  })

  useResizeObserver(options.viewport, ([entry]) => {
    viewportWidth = entry.contentRect.width
    setViewportWidth(viewportWidth)
    writeLayout()
  })
  watch([options.columns, options.content, options.enabled], () => {
    writeLayout()
    writeResizeGuide()
  }, { flush: 'post', immediate: true })
  const sizing = table.atoms.columnSizing.subscribe(writeLayout)
  const resizing = table.atoms.columnResizing.subscribe(writeResizeGuide)
  onScopeDispose(() => {
    sizing.unsubscribe()
    resizing.unsubscribe()
  })
}
