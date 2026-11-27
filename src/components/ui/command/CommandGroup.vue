<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { CommandGroup, CommandGroupLabel } from '@ui-sdk/vue/command'
import { cn } from 'cn'

const props = defineProps<{ class?: HTMLAttributes['class'], heading?: string }>()
</script>

<template>
  <CommandGroup :class="cn('overflow-hidden p-1 text-foreground', props.class)">
    <CommandGroupLabel v-if="heading" class="px-2 py-1.5 text-xs font-medium text-muted-foreground">{{ heading }}</CommandGroupLabel>
    <slot />
  </CommandGroup>
</template>
