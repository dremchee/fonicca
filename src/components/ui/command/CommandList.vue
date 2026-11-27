<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { CommandList } from '@ui-sdk/vue/command'
import { cn } from 'cn'

const props = defineProps<{ class?: HTMLAttributes['class'] }>()
</script>

<template>
  <CommandList :class="cn('max-h-[300px] scroll-py-1 overflow-x-hidden overflow-y-auto', props.class)">
    <slot />
  </CommandList>
</template>
