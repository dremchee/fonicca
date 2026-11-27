<script setup lang="ts">
import type { HTMLAttributes } from "vue"
import { Switch, SwitchThumb } from "@ui-sdk/vue/switch"
import { cn } from "cn"

const props = defineProps<{ class?: HTMLAttributes["class"] }>()
const model = defineModel<boolean>()
</script>

<template>
  <Switch
    data-slot="switch"
    v-model="model"
    :class="cn(
      'peer data-[checked]:bg-primary data-[unchecked]:bg-input focus-visible:border-ring focus-visible:ring-ring/50 inline-flex h-[1.15rem] w-8 shrink-0 items-center rounded-full border border-transparent shadow-xs transition-all outline-none focus-visible:ring-[3px] disabled:cursor-not-allowed disabled:opacity-50',
      props.class,
    )"
  >
    <SwitchThumb
      data-slot="switch-thumb"
      class="bg-white pointer-events-none block size-4 rounded-full ring-0 transition-transform data-[checked]:translate-x-[calc(100%-2px)] data-[unchecked]:translate-x-0"
    >
      <slot name="thumb" />
    </SwitchThumb>
  </Switch>
</template>
