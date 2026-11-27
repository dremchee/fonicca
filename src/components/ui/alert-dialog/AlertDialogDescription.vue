<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { AlertDialogDescription } from '@ui-sdk/vue/alert-dialog'
import { cn } from 'cn'
const props = defineProps<{ class?: HTMLAttributes['class'] }>()
</script>

<template>
  <AlertDialogDescription :class="cn('text-muted-foreground text-sm', props.class)">
    <slot />
  </AlertDialogDescription>
</template>
