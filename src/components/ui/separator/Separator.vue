<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import { cn } from "cn"
import { Separator } from "@ui-sdk/vue/separator"

const props = withDefaults(defineProps<
  { orientation?: "horizontal" | "vertical", decorative?: boolean, class?: HTMLAttributes["class"] }
>(), {
  orientation: "horizontal",
  decorative: true,
})

</script>

<template>
  <Separator
    data-slot="separator"
    :orientation="orientation" :decorative="decorative"
    :class="
      cn(
        'bg-border shrink-0 data-[orientation=horizontal]:h-px data-[orientation=horizontal]:w-full data-[orientation=vertical]:h-full data-[orientation=vertical]:w-px',
        props.class,
      )
    "
  />
</template>
