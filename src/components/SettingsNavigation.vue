<script setup lang="ts">
import type { Component } from 'vue'
import type { SettingsSection } from '@/stores/settings'
import { ArrowLeft, AudioLines, AudioWaveform, Palette, Cpu, Info, Keyboard, Library, Settings2 } from '@lucide/vue'
import { useI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores/settings'

defineProps<{ showBack?: boolean }>()
const { t } = useI18n()
const store = useSettingsStore()

const sections: { id: SettingsSection, icon: Component }[] = [
  { id: 'general', icon: Settings2 },
  { id: 'library', icon: Library },
  { id: 'playback', icon: AudioLines },
  { id: 'waveform', icon: AudioWaveform },
  { id: 'themes', icon: Palette },
  { id: 'analysis', icon: Cpu },
  { id: 'shortcuts', icon: Keyboard },
  { id: 'about', icon: Info },
]
</script>

<template>
  <nav class="min-h-0 overflow-y-auto p-2 flex flex-col gap-px" :aria-label="t('sidebar.settings')">
    <button v-if="showBack" type="button" class="nav-row mb-2" @click="store.dialogOpen = false">
      <ArrowLeft class="size-4 shrink-0" />
      <span>{{ t('sidebar.backToLibrary') }}</span>
    </button>
    <button
      v-for="s in sections" :key="s.id" type="button" class="nav-row"
      :class="{ active: store.section === s.id }" :aria-current="store.section === s.id ? 'page' : undefined"
      @click="store.section = s.id"
    >
      <component :is="s.icon" class="size-4 shrink-0" />
      <span>{{ t(`settings.nav.${s.id}`) }}</span>
    </button>
  </nav>
</template>

<style scoped>
.nav-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  min-height: 28px;
  flex-shrink: 0;
  white-space: nowrap;
  padding: 0 0.5rem;
  border-radius: calc(var(--radius) - 2px);
  color: var(--muted-foreground);
  text-align: left;
}

.nav-row:hover {
  background: var(--accent);
  color: var(--foreground);
}

.nav-row.active {
  background: var(--primary-soft);
  color: var(--primary);
  font-weight: 500;
}

</style>
