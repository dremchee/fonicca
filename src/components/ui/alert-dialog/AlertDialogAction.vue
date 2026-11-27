<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import { cn } from "cn"
import { AlertDialogClose } from "@ui-sdk/vue/alert-dialog"
import { buttonVariants } from '@/components/ui/button'

const props = defineProps<{ class?: HTMLAttributes["class"] }>()

</script>

<template>
  <AlertDialogClose :class="cn(buttonVariants(), props.class)">
    <slot />
  </AlertDialogClose>
</template>
