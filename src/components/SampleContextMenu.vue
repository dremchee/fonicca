<script setup lang="ts">
import type { Sample } from '@/api'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { onClickOutside, onKeyStroke, useEventListener } from '@vueuse/core'
import { computed, nextTick, ref } from 'vue'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'

const props = defineProps<{ sample: Sample, x: number, y: number }>()
const emit = defineEmits<{ close: [] }>()

const library = useLibraryStore()
const player = usePlayerStore()
const menu = ref<HTMLElement | null>(null)
const showCollections = ref(false)
const newName = ref('')
const newInput = ref<HTMLInputElement | null>(null)

const count = computed(() => (library.selectedIds.has(props.sample.id) ? library.selectedIds.size : 1))
const countLabel = computed(() => (count.value > 1 ? ` ${count.value} samples` : ''))
const currentCollection = computed(() => {
  const s = library.scope
  return s.kind === 'collection' ? library.collections.find(c => c.id === s.id) : undefined
})

// Keep the menu inside the window.
const style = computed(() => ({
  left: `${Math.min(props.x, window.innerWidth - 230)}px`,
  top: `${Math.min(props.y, window.innerHeight - 260)}px`,
}))

onClickOutside(menu, () => emit('close'))
onKeyStroke('Escape', (e) => {
  e.stopPropagation()
  emit('close')
})
useEventListener(window, 'blur', () => emit('close'))

async function run(action: () => unknown) {
  emit('close')
  await action()
}

async function openNewCollection() {
  showCollections.value = true
  await nextTick()
  newInput.value?.focus()
}

async function submitNew() {
  const name = newName.value.trim()
  if (!name)
    return
  await run(() => library.createCollection(name, true))
}
</script>

<template>
  <Teleport to="body">
    <div ref="menu" class="menu" :style="style" @contextmenu.prevent>
      <button class="item" @click="run(() => player.toggle(sample))">
        <span :class="player.playingId === sample.id ? 'i-lucide-square' : 'i-lucide-play'" />
        {{ player.playingId === sample.id ? 'Stop' : 'Play' }}
        <kbd>Space</kbd>
      </button>
      <div class="sep" />
      <button class="item" @click="run(() => library.toggleFavorite(sample))">
        <span class="i-lucide-star" />
        {{ sample.favorite ? 'Remove from favorites' : 'Add to favorites' }}{{ countLabel }}
        <kbd>F</kbd>
      </button>
      <button class="item" @click="run(() => library.requestTagInput())">
        <span class="i-lucide-tag" />
        Add tag…
        <kbd>T</kbd>
      </button>
      <div class="relative" @mouseenter="showCollections = true" @mouseleave="showCollections = !!newName">
        <button class="item" @click="showCollections = true">
          <span class="i-lucide-layers" />
          Add to collection
          <span class="i-lucide-chevron-right ml-auto text-muted" />
        </button>
        <div v-if="showCollections" class="menu submenu">
          <button
            v-for="c in library.collections" :key="c.id"
            class="item" @click="run(() => library.addToCollection(c.id))"
          >
            <span class="truncate">{{ c.name }}</span>
          </button>
          <div v-if="library.collections.length" class="sep" />
          <input
            ref="newInput"
            v-model="newName"
            placeholder="New collection…"
            class="mx-1 my-0.5 h-6 w-[calc(100%-8px)] px-2 rounded bg-raised outline-none border border-transparent focus:border-accent placeholder:text-muted text-xs"
            @keydown.enter="submitNew"
            @keydown.stop
            @click="openNewCollection"
          >
        </div>
      </div>
      <button
        v-if="currentCollection"
        class="item" @click="run(() => library.removeFromCollection(currentCollection!.id))"
      >
        <span class="i-lucide-minus-circle" />
        <span class="truncate">Remove from “{{ currentCollection.name }}”</span>
      </button>
      <div class="sep" />
      <button class="item" @click="run(() => revealItemInDir(sample.path))">
        <span class="i-lucide-folder-open" />
        Show in Finder
      </button>
    </div>
  </Teleport>
</template>

<style scoped>
.menu {
  position: fixed;
  z-index: 50;
  width: 220px;
  padding: 4px;
  border-radius: 8px;
  border: 1px solid var(--line);
  background: var(--panel);
  box-shadow: 0 8px 28px rgb(0 0 0 / 0.25);
}
.submenu {
  position: absolute;
  left: calc(100% + 4px);
  top: -4px;
  max-height: 280px;
  overflow-y: auto;
}
.item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: 26px;
  padding: 0 8px;
  border-radius: 5px;
  text-align: left;
  white-space: nowrap;
}
.item:hover {
  background: var(--accent);
  color: white;
}
.item:hover kbd,
.item:hover .text-muted {
  color: rgb(255 255 255 / 0.75);
}
kbd {
  margin-left: auto;
  font-family: inherit;
  font-size: 11px;
  color: var(--muted);
}
.sep {
  height: 1px;
  margin: 4px 6px;
  background: var(--line);
}
</style>
