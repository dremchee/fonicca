<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { CommandItem } from '@ui-sdk/vue/command'
import { cn } from 'cn'

const props = defineProps<{ class?: HTMLAttributes['class'], value: string }>()
</script>

<template>
  <CommandItem :value="value" :class="cn('relative flex cursor-default select-none items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-hidden data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*=size-])]:size-4 [&_svg:not([class*=text-])]:text-muted-foreground', props.class)">
    <slot />
  </CommandItem>
</template>
