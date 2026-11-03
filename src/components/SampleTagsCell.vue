<script setup lang="ts">
import type { Sample } from '@/api'
import { Plus } from '@lucide/vue'
import { useResizeObserver } from '@vueuse/core'
import { shallowRef, useTemplateRef } from 'vue'
import { useI18n } from 'vue-i18n'
import TagPicker from '@/components/TagPicker.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { useTagColors } from '@/composables/tagColors'

defineProps<{ sample: Sample }>()
const { t } = useI18n()
const { tagStyle } = useTagColors()
const tagsViewport = useTemplateRef<HTMLElement>('tagsViewport')
const tagsContent = useTemplateRef<HTMLElement>('tagsContent')
const overflowing = shallowRef(false)

function measureOverflow() {
  overflowing.value = !!tagsViewport.value && !!tagsContent.value
    && tagsContent.value.getBoundingClientRect().width > tagsViewport.value.getBoundingClientRect().width + 0.5
}
useResizeObserver(tagsViewport, measureOverflow)
useResizeObserver(tagsContent, measureOverflow)
</script>

<template>
  <span class="sample-tags relative min-w-0 h-full flex items-center" @mousedown.stop @click.stop @dblclick.stop @contextmenu.stop>
    <span ref="tagsViewport" class="tags-viewport min-w-0 w-full overflow-hidden" :class="{ 'tags-overflow': overflowing }">
      <span ref="tagsContent" class="w-max flex items-center gap-1">
        <Badge
          v-for="tag in sample.tags" :key="tag.id" variant="secondary" class="tag-color h-5 shrink-0 text-[11px]"
          :style="tagStyle(tag)" :data-sample-hint="tag.name" data-grace-area-trigger
        >{{ tag.name }}</Badge>
      </span>
    </span>
    <TagPicker :sample-id="sample.id" :assigned="sample.tags" :count="1">
      <template #trigger="{ saving }">
        <Button
          variant="ghost" size="icon-xs" class="tag-add text-muted-foreground data-[state=open]:bg-accent"
          :disabled="saving" :aria-label="t('list.tags.add', { name: sample.name })"
          :data-sample-hint="t('list.menu.addTag')" data-grace-area-trigger data-hint-side="bottom"
        ><Plus /></Button>
      </template>
    </TagPicker>
  </span>
</template>

<style scoped>
.tags-overflow {
  -webkit-mask-image: linear-gradient(to right, #000 calc(100% - 20px), transparent);
  mask-image: linear-gradient(to right, #000 calc(100% - 20px), transparent);
}
.tag-add {
  position: absolute;
  right: 0;
  top: 50%;
  transform: translateY(-50%);
  isolation: isolate;
}
.tag-add::before {
  content: '';
  position: absolute;
  inset: -4px -2px -4px -16px;
  z-index: -1;
  border-radius: inherit;
  background: linear-gradient(to right, transparent, var(--sample-row-surface) 45%);
  pointer-events: none;
}
</style>
