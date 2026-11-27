<script setup lang="ts" generic="T">
import { Select as UiSelect } from '@ui-sdk/vue/select'

const model = defineModel<T>()
</script>

<template>
  <UiSelect :value="model" @update:value="model = $event ?? undefined">
    <slot />
  </UiSelect>
</template>
