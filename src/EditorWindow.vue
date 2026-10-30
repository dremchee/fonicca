<script setup lang="ts">
import { debugMonitorOpen } from '@/composables/debugMonitor'
import { defineAsyncComponent, onMounted } from 'vue'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import EditorView from '@/components/EditorView.vue'
import EditorUnsavedDialog from '@/components/EditorUnsavedDialog.vue'
import BasicPitchDialog from '@/components/BasicPitchDialog.vue'
import { useBasicPitchStore } from '@/stores/basicPitch'
import SettingsHost from '@/components/SettingsHost.vue'
import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'
import { useEditorStore } from '@/stores/editor'
import { useSettingsStore } from '@/stores/settings'

const DebugMonitor = defineAsyncComponent(() => import('@/components/DebugMonitor.vue'))

// Root of an editor window, opened as index.html?editor=<sample id>.
const props = defineProps<{ sampleId: number }>()

const editor = useEditorStore()
const settings = useSettingsStore()

onMounted(() => Promise.all([settings.init(), editor.init(props.sampleId), useBasicPitchStore().init()]))
</script>

<template>
  <TooltipProvider :delay-duration="500" :skip-delay-duration="300">
    <EditorView />
    <BasicPitchDialog />
    <SettingsHost wave-target="editor" />
    <DebugMonitor v-if="debugMonitorOpen" />
    <ConfirmDialog />
    <EditorUnsavedDialog />
    <Toaster theme="system" position="bottom-right" />
  </TooltipProvider>
</template>
