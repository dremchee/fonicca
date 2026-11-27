<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { DialogTitle } from '@ui-sdk/vue/dialog'
import { cn } from 'cn'
const props = defineProps<{ class?: HTMLAttributes['class'] }>()
</script>

<template>
  <DialogTitle :class="cn('text-lg font-semibold leading-none', props.class)">
    <slot />
  </DialogTitle>
</template>
