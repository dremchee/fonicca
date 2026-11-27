<script setup lang="ts">
import { MenuTrigger } from '@ui-sdk/vue/menu'
import RenderChild from '@/components/ui/RenderChild'

defineProps<{ asChild?: boolean }>()
</script>

<template>
  <MenuTrigger>
    <template v-if="asChild" #render="{ props }">
      <RenderChild :trigger-props="props"><slot /></RenderChild>
    </template>
    <slot v-if="!asChild" />
  </MenuTrigger>
</template>
