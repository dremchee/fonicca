<script setup lang="ts">
import { useTagColors } from '@/composables/tagColors'
import type { TagRef } from '@/api'
import { X } from '@lucide/vue'
import { useResizeObserver } from '@vueuse/core'
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import TagPicker from '@/components/TagPicker.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { useLibraryStore } from '@/stores/library'
import { visibleTagCount } from '@/tag-overflow'

const props = defineProps<{ tags: TagRef[], count: number }>()
const { t } = useI18n()
const { tagStyle } = useTagColors()
const library = useLibraryStore()
const box = ref<HTMLElement | null>(null)
const measuring = ref<HTMLElement | null>(null)
const picker = ref<HTMLElement | null>(null)
const widths = ref<number[]>([])
const available = ref(0)
const counterWidth = ref(28)
const pickerWidth = ref(56)
const naturalWidth = computed(() => widths.value.reduce((sum, width) => sum + width, 0)
  + Math.max(0, widths.value.length - 1) * 4 + pickerWidth.value + (props.tags.length ? 4 : 0))
const visible = computed(() => visibleTagCount(widths.value, available.value, counterWidth.value))
const hidden = computed(() => props.tags.slice(visible.value))

function measure() {
  if (!box.value || !measuring.value || !picker.value)
    return
  widths.value = [...measuring.value.querySelectorAll<HTMLElement>('[data-tag-measure]')].map(el => el.getBoundingClientRect().width)
  counterWidth.value = measuring.value.querySelector<HTMLElement>('[data-counter-measure]')?.getBoundingClientRect().width ?? 28
  pickerWidth.value = picker.value.getBoundingClientRect().width
  available.value = Math.max(0, box.value.getBoundingClientRect().width - pickerWidth.value - (props.tags.length ? 4 : 0))
}
useResizeObserver(box, measure)
useResizeObserver(measuring, measure)
useResizeObserver(picker, measure)
onMounted(measure)
watch(() => props.tags, async () => { await nextTick(); measure() }, { deep: true })
</script>

<template>
  <div ref="box" class="relative shrink-0 max-w-[40%] min-w-0 flex items-center gap-1" :style="{ width: `${naturalWidth}px` }">
    <div class="absolute invisible pointer-events-none size-0 overflow-hidden" aria-hidden="true" inert>
      <div ref="measuring" class="flex w-max items-center gap-1">
        <Badge v-for="tag in tags" :key="tag.id" data-tag-measure variant="secondary" :style="tagStyle(tag)" class="tag-color h-5 shrink-0 pr-1 text-[11px]">
          {{ tag.name }} <span class="inline-flex"><X class="size-3" /></span>
        </Badge>
        <Button data-counter-measure variant="ghost" size="xs" class="h-5 px-1.5 text-[11px]" tabindex="-1">+{{ tags.length }}</Button>
      </div>
    </div>
    <Badge v-for="tag in tags.slice(0, visible)" :key="tag.id" variant="secondary" :style="tagStyle(tag)" class="tag-color h-5 shrink-0 pr-1 text-[11px]">
      {{ tag.name }}
      <button class="rounded-full opacity-60 hover:opacity-100 focus-visible:opacity-100 outline-none focus-visible:ring-2 focus-visible:ring-ring/50" :aria-label="t('player.removeTag', { name: tag.name })" @click="library.removeTag(tag.id)">
        <X class="size-3" />
      </button>
    </Badge>
    <Popover v-if="hidden.length">
      <PopoverTrigger as-child>
        <Button variant="ghost" size="xs" class="h-5 shrink-0 px-1.5 text-[11px]" :aria-label="t('player.hiddenTags', { n: hidden.length })">+{{ hidden.length }}</Button>
      </PopoverTrigger>
      <PopoverContent align="start" class="w-64 max-h-64 overflow-y-auto p-3" :aria-label="t('player.hiddenTags', { n: hidden.length })">
        <div class="flex flex-wrap gap-1">
          <Badge v-for="tag in hidden" :key="tag.id" variant="secondary" :style="tagStyle(tag)" class="tag-color max-w-full h-5 pr-1 text-[11px]">
            <span class="truncate">{{ tag.name }}</span>
            <button class="shrink-0 rounded-full opacity-60 hover:opacity-100 focus-visible:opacity-100 outline-none focus-visible:ring-2 focus-visible:ring-ring/50" :aria-label="t('player.removeTag', { name: tag.name })" @click="library.removeTag(tag.id)"><X class="size-3" /></button>
          </Badge>
        </div>
      </PopoverContent>
    </Popover>
    <span ref="picker" class="shrink-0 inline-flex"><TagPicker :assigned="tags" :count="count" /></span>
  </div>
</template>
