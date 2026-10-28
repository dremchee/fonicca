<script setup lang="ts">
import { computed } from 'vue'
import { Kbd } from '@/components/ui/kbd'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

// Tooltip around a single control: <Hint text="Rescan"><Button … /></Hint>
// An empty text renders the control alone.
// Around a menu, select or popover, wrap the whole root in <span> inside the Hint, not
// just its trigger: the tooltip's popper context would otherwise capture the trigger
// as its anchor and the popup would open off screen.
const props = withDefaults(defineProps<{
  text: string
  shortcut?: string
  side?: 'top' | 'right' | 'bottom' | 'left'
}>(), { side: 'bottom' })

// Keep the action and its scope on separate lines. Each line contributes its
// own intrinsic width, rather than wrapping a full-width balanced text box.
const lines = computed(() => {
  const scope = props.text.match(/^(.*?)\s+(\([^()]+\))$/)
  return scope && props.text.length > 40
    ? [scope[1]!, scope[2]!]
    : props.text.split('\n')
})
</script>

<template>
  <slot v-if="!text" />
  <Tooltip v-else>
    <TooltipTrigger as-child>
      <slot />
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
