<script setup lang="ts">
import { computed } from 'vue'
import { hintLines } from '@/hint-text'
import { Kbd } from '@/components/ui/kbd'
import RenderChild from '@/components/ui/RenderChild'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

// Tooltip around a single control: <Hint text="Rescan"><Button … /></Hint>
// An empty text renders the control alone.
const props = withDefaults(defineProps<{
  text: string
  shortcut?: string
  side?: 'top' | 'right' | 'bottom' | 'left'
  disabled?: boolean
}>(), { side: 'bottom' })

// Keep the action and its scope on separate lines. Each line contributes its
// own intrinsic width, rather than wrapping a full-width balanced text box.
const lines = computed(() => hintLines(props.text))
</script>

<template>
  <slot v-if="!text" />
  <Tooltip v-else :delay="500" :disabled="disabled" type="description">
    <TooltipTrigger>
      <template #render="{ props }">
        <RenderChild :trigger-props="props"><slot /></RenderChild>
      </template>
    </TooltipTrigger>
    <TooltipContent :side="side" class="flex max-w-64 items-center justify-center gap-2">
      <!-- min-w-0 lets the text shrink inside the flex row; long paths break anywhere. -->
      <span class="min-w-0 [overflow-wrap:anywhere] [text-wrap:wrap]">
        <span v-for="(line, index) in lines" :key="index" class="block">{{ line }}</span>
      </span>
      <Kbd v-if="shortcut" class="shrink-0">
        {{ shortcut }}
      </Kbd>
    </TooltipContent>
  </Tooltip>
</template>
