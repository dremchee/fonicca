import type { TagRef } from '@/api'
import { createGlobalState, useStorage } from '@vueuse/core'
import { tagColorStyle } from '@/tag-colors'

export const useTagColors = createGlobalState(() => {
  const colors = useStorage<Record<string, string>>('fonicca-tag-colors-v1', {})

  function customColor(tag: TagRef) {
    const value = colors.value[tag.id]
    return typeof value === 'string' && /^#[0-9a-f]{6}$/i.test(value) ? value : undefined
  }
  function tagStyle(tag: TagRef) {
    return tagColorStyle(tag.name, customColor(tag))
  }
  function setColor(tagId: number, color?: string) {
    if (color === undefined)
      delete colors.value[tagId]
    else if (/^#[0-9a-f]{6}$/i.test(color))
      colors.value[tagId] = color
  }
  return { tagStyle, customColor, setColor }
})
