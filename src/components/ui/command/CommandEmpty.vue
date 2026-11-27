<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { CommandEmpty } from '@ui-sdk/vue/command'
import { cn } from 'cn'

const props = defineProps<{ class?: HTMLAttributes['class'] }>()
</script>

<template>
  <CommandEmpty :class="cn('py-6 text-center text-sm', props.class)">
    <slot />
  </CommandEmpty>
</template>
