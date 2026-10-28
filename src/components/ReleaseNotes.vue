<script setup lang="ts">
import { computed } from 'vue'
import { parseReleaseNotes } from '@/release-notes'

const props = defineProps<{ notes: string }>()
const blocks = computed(() => parseReleaseNotes(props.notes))
</script>

<template>
  <div class="release-notes max-h-64 overflow-y-auto overscroll-contain pr-2 text-[13px] leading-relaxed text-muted-foreground select-text break-words">
    <template v-for="(block, index) in blocks" :key="index">
      <h4 v-if="block.kind === 'heading'" class="font-medium text-foreground">{{ block.text }}</h4>
      <ul v-else-if="block.kind === 'list'" class="list-none space-y-2">
        <li v-for="(item, itemIndex) in block.items" :key="itemIndex" class="flex items-start gap-2.5">
          <span aria-hidden="true" class="mt-[0.65em] size-1 shrink-0 rounded-full bg-primary/60" />
          <span>{{ item }}</span>
        </li>
      </ul>
      <p v-else>{{ block.text }}</p>
    </template>
  </div>
</template>

<style scoped>
.release-notes > * + * { margin-top: 12px; }
.release-notes > h4:not(:first-child) { margin-top: 20px; }
</style>
