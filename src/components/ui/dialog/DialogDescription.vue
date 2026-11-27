<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { DialogDescription } from '@ui-sdk/vue/dialog'
import { cn } from 'cn'
const props = defineProps<{ class?: HTMLAttributes['class'] }>()
</script>

<template>
  <DialogDescription :class="cn('text-muted-foreground text-sm', props.class)">
    <slot />
  </DialogDescription>
</template>
