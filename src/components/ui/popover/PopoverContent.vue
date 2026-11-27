<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { PopoverContent } from '@ui-sdk/vue/popover'
import { cn } from 'cn'

const props = withDefaults(defineProps<{
  class?: HTMLAttributes['class']
  align?: 'start' | 'center' | 'end'
  sideOffset?: number
}>(), { align: 'center', sideOffset: 4 })
</script>

<template>
  <PopoverContent
    :align="align" :side-offset="sideOffset"
    :class="cn('ui-popup m-0 w-72 max-w-(--available-width) rounded-md border bg-popover p-4 text-popover-foreground shadow-md outline-hidden', props.class)"
  >
    <slot />
  </PopoverContent>
</template>
