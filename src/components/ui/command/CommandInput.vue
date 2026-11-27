<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { Search } from '@lucide/vue'
import { CommandInput } from '@ui-sdk/vue/command'
import { cn } from 'cn'

defineOptions({ inheritAttrs: false })
const props = defineProps<{ class?: HTMLAttributes['class'] }>()
</script>

<template>
  <div data-slot="command-input-wrapper" class="flex h-9 items-center gap-2 border-b px-3">
    <Search class="size-4 shrink-0 opacity-50" aria-hidden="true" />
    <CommandInput
      v-bind="$attrs" autofocus
      :class="cn('flex h-10 w-full rounded-md bg-transparent py-3 text-sm outline-hidden placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50', props.class)"
    />
  </div>
</template>
