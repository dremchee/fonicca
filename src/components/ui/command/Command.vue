<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { Command } from '@ui-sdk/vue/command'
import { cn } from 'cn'

const props = defineProps<{ class?: HTMLAttributes['class'] }>()
</script>

<template>
  <Command :class="cn('flex h-full w-full flex-col overflow-hidden rounded-md bg-popover text-popover-foreground', props.class)">
    <slot />
  </Command>
</template>
