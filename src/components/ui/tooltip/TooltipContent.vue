<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { TooltipArrow, TooltipContent } from '@ui-sdk/vue/tooltip'
import { cn } from 'cn'

const props = withDefaults(defineProps<{ class?: HTMLAttributes['class'], sideOffset?: number }>(), { sideOffset: 4 })
</script>

<template>
  <TooltipContent
    :side-offset="sideOffset"
    :class="cn('ui-popup m-0 w-fit rounded-md bg-foreground px-3 py-1.5 text-center text-xs text-background [text-wrap:pretty]', props.class)"
  >
    <slot />
    <TooltipArrow class="tooltip-arrow" />
  </TooltipContent>
</template>

<style scoped>
.tooltip-arrow {
  width: 7px;
  height: 7px;
  rotate: 45deg;
  border-radius: 1px;
  background: var(--foreground);
  pointer-events: none;
}
.tooltip-arrow[data-side='top'] { bottom: -3px; }
.tooltip-arrow[data-side='right'] { left: -3px; }
.tooltip-arrow[data-side='bottom'] { top: -3px; }
.tooltip-arrow[data-side='left'] { right: -3px; }
</style>
