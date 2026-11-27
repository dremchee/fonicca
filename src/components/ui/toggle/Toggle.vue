<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import type { ToggleVariants } from "."
import { cn } from "cn"
import { Toggle } from "@ui-sdk/vue/toggle"
import { toggleVariants } from "."

const props = withDefaults(defineProps<{
  class?: HTMLAttributes["class"]
  variant?: ToggleVariants["variant"]
  size?: ToggleVariants["size"]
}>(), {
  variant: "default",
  size: "default",
})

const model = defineModel<boolean>()
</script>

<template>
  <Toggle
    v-slot="slotProps"
    data-slot="toggle"
    v-model:pressed="model"
    :class="cn(toggleVariants({ variant, size }), props.class)"
  >
    <slot v-bind="slotProps" />
  </Toggle>
</template>
