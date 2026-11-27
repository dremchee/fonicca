<script setup lang="ts">
import type { VariantProps } from "class-variance-authority"
import type { HTMLAttributes } from "vue"
import { cn } from "cn"
import { Toggle } from "@ui-sdk/vue/toggle"
import { inject } from "vue"
import { toggleVariants } from '@/components/ui/toggle'

type ToggleGroupVariants = VariantProps<typeof toggleVariants> & {
  spacing?: number
}

const props = defineProps<{
  value: string
  class?: HTMLAttributes["class"]
  variant?: ToggleGroupVariants["variant"]
  size?: ToggleGroupVariants["size"]
}>()

const context = inject<ToggleGroupVariants>("toggleGroup")
</script>

<template>
  <Toggle
    v-slot="slotProps"
    data-slot="toggle-group-item"
    :data-variant="context?.variant || variant"
    :data-size="context?.size || size"
    :data-spacing="context?.spacing"
    :value="value"
    :class="cn(
      toggleVariants({
        variant: context?.variant || variant,
        size: context?.size || size,
      }),
      'w-auto min-w-0 shrink-0 px-3 focus:z-10 focus-visible:z-10',
      'data-[spacing=0]:rounded-none data-[spacing=0]:shadow-none data-[spacing=0]:first:rounded-l-md data-[spacing=0]:last:rounded-r-md data-[spacing=0]:data-[variant=outline]:border-l-0 data-[spacing=0]:data-[variant=outline]:first:border-l',
      props.class)"
  >
    <slot v-bind="slotProps" />
  </Toggle>
</template>
