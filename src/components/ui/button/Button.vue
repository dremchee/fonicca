<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import type { ButtonVariants } from "."
import { cn } from "cn"
import { Button } from "@ui-sdk/vue/button"
import { buttonVariants } from "."

interface Props {
  variant?: ButtonVariants["variant"]
  size?: ButtonVariants["size"]
  class?: HTMLAttributes["class"]
}

const props = defineProps<Props>()
</script>

<template>
  <Button
    data-slot="button"
    :data-variant="variant"
    :data-size="size"
    :class="cn(buttonVariants({ variant, size }), props.class)"
  >
    <slot />
  </Button>
</template>
