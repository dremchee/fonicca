<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { AlertDialogBackdrop, AlertDialogContent } from '@ui-sdk/vue/alert-dialog'
import { cn } from 'cn'

defineOptions({ inheritAttrs: false })
const props = defineProps<{ class?: HTMLAttributes['class'] }>()
</script>

<template>
  <AlertDialogBackdrop class="fixed inset-0 z-40 bg-black/80" />
  <AlertDialogContent
    v-bind="$attrs"
    :class="cn('ui-popup fixed top-1/2 left-1/2 z-50 m-0 grid w-full max-w-[calc(100%-2rem)] -translate-x-1/2 -translate-y-1/2 gap-4 rounded-lg border bg-background p-6 shadow-lg sm:max-w-lg', props.class)"
  >
    <slot />
  </AlertDialogContent>
</template>
