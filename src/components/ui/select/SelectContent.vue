<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { SelectContent } from '@ui-sdk/vue/select'
import { cn } from 'cn'

const props = defineProps<{ class?: HTMLAttributes['class'] }>()
</script>

<template>
  <SelectContent
    align="start" :side-offset="4" :align-item-with-trigger="false"
    :class="cn('m-0 min-w-[max(8rem,var(--anchor-width))] max-w-(--available-width) max-h-(--available-height) overflow-x-hidden overflow-y-auto rounded-md border bg-popover p-1 text-popover-foreground shadow-md ui-popup', props.class)"
  >
    <slot />
  </SelectContent>
</template>
