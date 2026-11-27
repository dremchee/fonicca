<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { X } from '@lucide/vue'
import { DialogBackdrop, DialogClose, DialogContent } from '@ui-sdk/vue/dialog'
import { cn } from 'cn'
import { useI18n } from 'vue-i18n'

defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<{ class?: HTMLAttributes['class'], showCloseButton?: boolean }>(), {
  showCloseButton: true,
})
const { t } = useI18n()
</script>

<template>
  <DialogBackdrop class="fixed inset-0 z-40 bg-black/80" />
  <DialogContent
    v-bind="$attrs"
    :class="cn('ui-popup fixed top-1/2 left-1/2 z-50 m-0 grid w-full max-w-[calc(100%-2rem)] -translate-x-1/2 -translate-y-1/2 gap-4 rounded-lg border bg-background p-6 shadow-lg sm:max-w-lg', props.class)"
  >
    <slot />
    <DialogClose
      v-if="showCloseButton" :aria-label="t('common.actions.close')"
      class="ring-offset-background focus:ring-ring absolute top-4 right-4 rounded-xs opacity-70 transition-opacity hover:opacity-100 focus:ring-2 focus:ring-offset-2 focus:outline-hidden disabled:pointer-events-none"
    >
      <X class="size-4" aria-hidden="true" />
    </DialogClose>
  </DialogContent>
</template>
