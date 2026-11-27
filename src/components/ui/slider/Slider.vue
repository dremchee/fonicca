<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { Slider, SliderControl, SliderIndicator, SliderThumb, SliderTrack } from '@ui-sdk/vue/slider'
import { cn } from 'cn'

const props = defineProps<{ class?: HTMLAttributes['class'], ariaLabel?: string }>()
const model = defineModel<number[]>({ required: true })
const emit = defineEmits<{ valueCommit: [value: number[]] }>()
</script>

<template>
  <Slider
    v-model:value="model" :aria-label="ariaLabel"
    :class="cn('relative flex w-full touch-none items-center select-none data-[disabled]:opacity-50 data-[orientation=vertical]:h-full data-[orientation=vertical]:min-h-44 data-[orientation=vertical]:w-auto data-[orientation=vertical]:flex-col', props.class)"
    @value-commit="emit('valueCommit', $event)"
  >
    <SliderControl class="flex w-full items-center py-2 data-[orientation=vertical]:h-full data-[orientation=vertical]:w-auto data-[orientation=vertical]:px-2 data-[orientation=vertical]:py-0">
      <SliderTrack class="relative grow rounded-full bg-muted data-[orientation=horizontal]:h-1.5 data-[orientation=horizontal]:w-full data-[orientation=vertical]:h-full data-[orientation=vertical]:w-1.5">
        <SliderIndicator class="absolute rounded-full bg-primary data-[orientation=horizontal]:h-full data-[orientation=vertical]:w-full" />
        <SliderThumb
          v-for="(_, index) in model" :key="index" :index="index" :aria-label="ariaLabel"
          class="block size-4 shrink-0 rounded-full border border-primary bg-white shadow-sm ring-ring/50 transition-[color,box-shadow] hover:ring-4 has-[:focus-visible]:ring-4 data-[disabled]:pointer-events-none data-[disabled]:opacity-50"
        />
      </SliderTrack>
    </SliderControl>
  </Slider>
</template>
