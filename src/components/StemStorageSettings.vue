<script setup lang="ts">
import { FolderOpen } from '@lucide/vue'
import { open } from '@tauri-apps/plugin-dialog'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import { useSettingsStore } from '@/stores/settings'

const settings = useSettingsStore()
const { t } = useI18n()
async function choose() {
  try {
    const folder = await open({ directory: true, defaultPath: settings.settings.stemsFolder || undefined })
    if (typeof folder === 'string') await settings.update({ stemsFolder: folder })
  }
  catch (error) { toast.error(String(error)) }
}
</script>

<template>
  <section class="rounded-[var(--radius)] border border-border bg-card p-4 flex flex-col gap-2" :aria-label="t('stems.storageTitle')">
    <h3 class="text-sm font-medium">{{ t('stems.storageTitle') }}</h3>
    <p class="text-xs text-muted-foreground">{{ t('stems.storageDescription') }}</p>
    <p class="text-xs break-all select-text" :title="settings.settings.stemsFolder || undefined">{{ settings.settings.stemsFolder || t('stems.besideOriginal') }}</p>
    <div class="flex flex-wrap gap-2">
      <Button size="sm" variant="outline" @click="choose"><FolderOpen />{{ t('stems.choose') }}</Button>
      <Button v-if="settings.settings.stemsFolder" size="sm" variant="ghost" @click="settings.update({ stemsFolder: null })">{{ t('stems.useOriginalFolder') }}</Button>
    </div>
  </section>
</template>
