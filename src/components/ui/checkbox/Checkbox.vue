<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import { Check } from "@lucide/vue"
import { Checkbox, CheckboxIndicator } from "@ui-sdk/vue/checkbox"
import { cn } from "cn"

const props = defineProps<{ class?: HTMLAttributes["class"] }>()
const model = defineModel<boolean | "indeterminate">({ default: false })
</script>

<template>
  <Checkbox
    v-slot="slotProps"
    data-slot="checkbox"
    v-model="model"
    :class="
      cn('peer border-input data-[checked]:bg-primary data-[checked]:text-primary-foreground data-[checked]:border-primary focus-visible:border-ring focus-visible:ring-ring/50 aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive size-4 shrink-0 rounded-[4px] border shadow-xs transition-shadow outline-none focus-visible:ring-3 disabled:cursor-not-allowed disabled:opacity-50',
         props.class)"
  >
    <CheckboxIndicator
      data-slot="checkbox-indicator"
      class="grid place-content-center text-current transition-none"
    >
      <slot v-bind="slotProps">
        <Check class="size-3.5" />
      </slot>
    </CheckboxIndicator>
  </Checkbox>
</template>
