<script setup lang="ts">
import { onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import ProjectView from '@/components/project/ProjectView.vue'
import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'
import { useProjectWindow } from '@/composables/useProjectWindow'
import { useProjectStore } from '@/stores/project'
import { useSettingsStore } from '@/stores/settings'
const project = useProjectStore()
const settings = useSettingsStore()
const { t } = useI18n()
useProjectWindow()
onMounted(() => settings.init())
watch(() => project.document.name, name => {
  document.title = `${name} — Fonicca`
  void getCurrentWindow().setTitle(document.title).catch(console.error)
}, { immediate: true })
</script>

<template>
  <TooltipProvider :delay-duration="500" :skip-delay-duration="300">
    <div class="h-full flex flex-col">
      <header class="h-10 shrink-0 pl-22 pr-4 flex items-center border-b border-border text-xs text-muted-foreground" data-tauri-drag-region>{{ t('project.windowTitle') }}</header>
      <div class="flex-1 min-h-0"><ProjectView /></div>
    </div>
    <ConfirmDialog />
    <Toaster theme="system" position="bottom-right" />
  </TooltipProvider>
</template>
