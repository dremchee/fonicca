<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { MenuContent } from '@ui-sdk/vue/menu'
import { cn } from 'cn'

const props = withDefaults(defineProps<{ class?: HTMLAttributes['class'], sideOffset?: number }>(), {
  sideOffset: 4,
})
</script>

<template>
  <MenuContent
    :side-offset="sideOffset"
    :class="cn('ui-popup m-0 min-w-32 max-h-(--available-height) overflow-y-auto overflow-x-hidden rounded-md border bg-popover p-1 text-popover-foreground shadow-md', props.class)"
  >
    <slot />
  </MenuContent>
</template>
