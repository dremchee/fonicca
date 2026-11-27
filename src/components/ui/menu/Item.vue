<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { MenuItem } from '@ui-sdk/vue/menu'
import { cn } from 'cn'

const props = defineProps<{ class?: HTMLAttributes['class'], variant?: 'default' | 'destructive', inset?: boolean }>()
</script>

<template>
  <MenuItem
    :destructive="variant === 'destructive'" :data-inset="inset ? '' : undefined" :data-variant="variant"
    :class="cn('relative flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-hidden select-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*=size-])]:size-4 [&_svg:not([class*=text-])]:text-muted-foreground data-[inset]:pl-8 data-[variant=destructive]:text-destructive data-[variant=destructive]:data-[highlighted]:bg-destructive/15', props.class)"
  >
    <slot />
  </MenuItem>
</template>
