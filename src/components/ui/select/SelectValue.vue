<script setup lang="ts">
import { SelectValue } from '@ui-sdk/vue/select'
</script>

<template>
  <SelectValue v-if="$slots.default">
    <slot />
  </SelectValue>
  <SelectValue v-else />
</template>
