<script setup lang="ts">
import type { TagRef } from '@/api'
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ColorPicker } from '@/components/ui/color-picker'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { useTagColors } from '@/composables/tagColors'

const props = defineProps<{ tag: TagRef }>()
const { t } = useI18n()
const { tagStyle, customColor, setColor } = useTagColors()
const open = ref(false)
const draft = ref('#000000')
watch(open, (value) => {
  if (value)
    draft.value = customColor(props.tag) ?? tagStyle(props.tag)['--tag-light-ink']
})
function save() {
  setColor(props.tag.id, draft.value)
  open.value = false
}
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <button
        type="button" class="grid size-4 shrink-0 place-items-center rounded-full outline-none focus-visible:ring-2 focus-visible:ring-ring hover:bg-accent"
        :aria-label="t('sidebar.tagColor', { name: tag.name })" :title="t('sidebar.tagColor', { name: tag.name })"
        @mousedown.stop @click.stop @dblclick.stop
      >
        <span class="tag-dot" :style="tagStyle(tag)" aria-hidden="true" />
      </button>
    </PopoverTrigger>
    <PopoverContent side="right" align="start" class="w-72 p-3" @click.stop @mousedown.stop @dblclick.stop>
      <p class="mb-3 truncate text-xs font-medium">{{ t('sidebar.tagColor', { name: tag.name }) }}</p>
      <ColorPicker v-model="draft" :formats="['hex']" :show-alpha="false" show-actions @save="save" @cancel="open = false" />
    </PopoverContent>
  </Popover>
</template>
