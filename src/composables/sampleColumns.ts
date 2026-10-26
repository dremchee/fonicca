import { createGlobalState, useStorage } from '@vueuse/core'
import { defaultSampleColumns, type SampleColumnWidths } from '@/sample-columns'

export const useSampleColumns = createGlobalState(() => {
  const legacy = useStorage('fonicca-list-details', false)
  const visibility = useStorage('fonicca-list-columns-v1', defaultSampleColumns(legacy.value), undefined, { mergeDefaults: true })
  const widths = useStorage<SampleColumnWidths>('fonicca-list-column-widths-v1', {})
  return { visibility, widths, resetWidths: () => { widths.value = {} } }
})
