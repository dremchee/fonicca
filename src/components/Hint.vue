<script setup lang="ts">
import { Kbd } from '@/components/ui/kbd'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

// Tooltip around a single control: <Hint text="Rescan"><Button … /></Hint>
withDefaults(defineProps<{
  text: string
  shortcut?: string
  side?: 'top' | 'right' | 'bottom' | 'left'
}>(), { side: 'bottom' })
</script>

<template>
  <Tooltip>
    <TooltipTrigger as-child>
      <slot />
    </TooltipTrigger>
    <TooltipContent :side="side" class="flex items-center gap-2">
      {{ text }}
      <Kbd v-if="shortcut">
        {{ shortcut }}
      </Kbd>
    </TooltipContent>
  </Tooltip>
</template>
