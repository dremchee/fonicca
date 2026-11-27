<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { Check } from '@lucide/vue'
import { SelectItem, SelectItemIndicator, SelectItemText } from '@ui-sdk/vue/select'
import { cn } from 'cn'

const props = defineProps<{ value: string | number, label?: string, class?: HTMLAttributes['class'] }>()
</script>

<template>
  <SelectItem
    :value="value" :label="label"
    :class="cn('relative flex w-full cursor-default select-none items-center gap-2 rounded-sm py-1.5 pr-8 pl-2 text-sm outline-hidden data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50', props.class)"
  >
    <SelectItemText class="flex min-w-0 items-center gap-2">
      <slot />
    </SelectItemText>
    <SelectItemIndicator class="absolute right-2 flex size-3.5 items-center justify-center">
      <slot name="indicator-icon">
        <Check class="size-4" aria-hidden="true" />
      </slot>
    </SelectItemIndicator>
  </SelectItem>
</template>
