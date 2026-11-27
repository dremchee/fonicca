<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { CommandSeparator } from '@ui-sdk/vue/command'
import { cn } from 'cn'

const props = defineProps<{ class?: HTMLAttributes['class'] }>()
</script>

<template>
  <CommandSeparator :class="cn('-mx-1 h-px bg-border', props.class)">
    <slot />
  </CommandSeparator>
</template>
