<script setup lang="ts">
import { Kbd } from '@/components/ui/kbd'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

// Tooltip around a single control: <Hint text="Rescan"><Button … /></Hint>
// An empty text renders the control alone.
withDefaults(defineProps<{
  text: string
  shortcut?: string
  side?: 'top' | 'right' | 'bottom' | 'left'
}>(), { side: 'bottom' })
</script>

<template>
  <slot v-if="!text" />
  <Tooltip v-else>
    <TooltipTrigger as-child>
      <slot />
    </TooltipTrigger>
    <TooltipContent :side="side" class="flex max-w-80 items-center gap-2">
      <!-- min-w-0 lets the text shrink inside the flex row; long paths break anywhere. -->
      <span class="min-w-0 [overflow-wrap:anywhere]">{{ text }}</span>
      <Kbd v-if="shortcut">
        {{ shortcut }}
      </Kbd>
    </TooltipContent>
  </Tooltip>
</template>
