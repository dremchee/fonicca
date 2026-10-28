<script setup lang="ts">
import type { TooltipProviderProps } from "reka-ui"
import { TooltipProvider } from "reka-ui"

const props = withDefaults(defineProps<TooltipProviderProps>(), {
  delayDuration: 0,
  // Popovers focus their first control on open; pointer-driven focus should
  // not reveal a tooltip before the user actually hovers that control.
  ignoreNonKeyboardFocus: true,
})
</script>

<template>
  <TooltipProvider v-bind="props">
    <slot />
  </TooltipProvider>
</template>
