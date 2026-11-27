<script setup lang="ts">
import type { VariantProps } from "class-variance-authority"
import type { HTMLAttributes } from "vue"
import type { toggleVariants } from '@/components/ui/toggle'
import { cn } from "cn"
import { ToggleGroup } from "@ui-sdk/vue/toggle"
import { computed, provide } from "vue"

type ToggleGroupVariants = VariantProps<typeof toggleVariants>

const props = withDefaults(defineProps<{
  type?: "single" | "multiple"
  class?: HTMLAttributes["class"]
  variant?: ToggleGroupVariants["variant"]
  size?: ToggleGroupVariants["size"]
  spacing?: number
}>(), {
  spacing: 0,
})

const model = defineModel<string | string[]>()
const values = computed(() => Array.isArray(model.value) ? model.value : model.value ? [model.value] : [])

provide("toggleGroup", {
  variant: props.variant,
  size: props.size,
  spacing: props.spacing,
})

function update(value: string[]) {
  model.value = props.type === "multiple" ? value : value[0]
}
</script>

<template>
  <ToggleGroup
    v-slot="slotProps"
    data-slot="toggle-group"
    :data-size="size"
    :data-variant="variant"
    :data-spacing="spacing"
    :style="{
      '--gap': spacing,
    }"
    :multiple="type === 'multiple'" :value="values" @update:value="update"
    :class="cn('group/toggle-group flex w-fit items-center gap-[--spacing(var(--gap))] rounded-md data-[spacing=default]:data-[variant=outline]:shadow-xs', props.class)"
  >
    <slot v-bind="slotProps" />
  </ToggleGroup>
</template>
