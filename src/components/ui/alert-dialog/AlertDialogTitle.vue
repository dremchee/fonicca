<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { AlertDialogTitle } from '@ui-sdk/vue/alert-dialog'
import { cn } from 'cn'
const props = defineProps<{ class?: HTMLAttributes['class'] }>()
</script>

<template>
  <AlertDialogTitle :class="cn('text-lg font-semibold', props.class)">
    <slot />
  </AlertDialogTitle>
</template>
