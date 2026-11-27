<script setup lang="ts">
import { MenuContextTrigger } from '@ui-sdk/vue/menu'
import RenderChild from '@/components/ui/RenderChild'

defineProps<{ asChild?: boolean }>()
</script>

<template>
  <MenuContextTrigger>
    <template v-if="asChild" #render="{ props }">
      <RenderChild :trigger-props="props"><slot /></RenderChild>
    </template>
    <slot v-if="!asChild" />
  </MenuContextTrigger>
</template>
