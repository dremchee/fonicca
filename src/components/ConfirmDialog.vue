<script setup lang="ts">
import { shallowRef, watch } from 'vue'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { buttonVariants } from '@/components/ui/button'
import { pendingConfirm, settleConfirm } from '@/composables/confirm'

// Keep the last options around so the text does not vanish during the close animation.
const shown = shallowRef(pendingConfirm.value)
watch(pendingConfirm, (v) => {
  if (v)
    shown.value = v
})

// Closing via Cancel, Escape or the action button all report open=false; let the
// action's own click settle first so it is not mistaken for a cancel.
function onOpenChange(open: boolean) {
  if (!open)
    queueMicrotask(() => settleConfirm(false))
}
</script>

<template>
  <AlertDialog :open="!!pendingConfirm" @update:open="onOpenChange">
    <AlertDialogContent v-if="shown">
      <AlertDialogHeader>
        <AlertDialogTitle>{{ shown.title }}</AlertDialogTitle>
        <AlertDialogDescription v-if="shown.description">
          {{ shown.description }}
        </AlertDialogDescription>
      </AlertDialogHeader>
      <AlertDialogFooter>
        <AlertDialogCancel>Cancel</AlertDialogCancel>
        <AlertDialogAction
          :class="shown.destructive ? buttonVariants({ variant: 'destructive' }) : undefined"
          @click="settleConfirm(true)"
        >
          {{ shown.action ?? 'Continue' }}
        </AlertDialogAction>
      </AlertDialogFooter>
    </AlertDialogContent>
  </AlertDialog>
</template>
