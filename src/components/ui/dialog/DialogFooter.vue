<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import { cn } from "cn"
import { DialogClose } from "@ui-sdk/vue/dialog"
import { Button } from '@/components/ui/button'
import RenderChild from '@/components/ui/RenderChild'

const props = withDefaults(defineProps<{
  class?: HTMLAttributes["class"]
  showCloseButton?: boolean
}>(), {
  showCloseButton: false,
})
</script>

<template>
  <div
    data-slot="dialog-footer"
    :class="cn('flex flex-col-reverse gap-2 sm:flex-row sm:justify-end', props.class)"
  >
    <slot />
    <DialogClose v-if="showCloseButton">
      <template #render="{ props }">
        <RenderChild :trigger-props="props"><Button variant="outline">Close</Button></RenderChild>
      </template>
    </DialogClose>
  </div>
</template>
