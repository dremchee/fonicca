<script setup lang="ts">
import { PopoverTrigger } from '@ui-sdk/vue/popover'
import RenderChild from '@/components/ui/RenderChild'

defineProps<{ asChild?: boolean }>()
</script>

<template>
  <PopoverTrigger>
    <template v-if="asChild" #render="{ props }">
      <RenderChild :trigger-props="props"><slot /></RenderChild>
    </template>
    <slot v-if="!asChild" />
  </PopoverTrigger>
</template>
