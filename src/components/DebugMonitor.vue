<script setup lang="ts">
import { Activity, Pause, Play, RotateCcw, X } from '@lucide/vue'
import { computed, useTemplateRef, watch } from 'vue'
import { useDraggable, useWindowSize } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { debugBytes, debugMonitorOpen, useDebugSampler } from '@/composables/debugMonitor'
import { useEditorStore } from '@/stores/editor'

const { t } = useI18n()
const editor = useEditorStore()
const { snapshot, history, error, paused, baseline, peak, reset } = useDebugSampler()
const header = useTemplateRef<HTMLElement>('header')
const panel = useTemplateRef<HTMLElement>('panel')
const { width, height } = useWindowSize()
const { x, y } = useDraggable(panel, {
  handle: header,
  initialValue: { x: Math.max(8, window.innerWidth - 372), y: 72 },
  preventDefault: true,
  onStart: (_pos, event) => { if ((event.target as Element).closest('button')) return false },
})
watch([width, height, x, y], () => {
  const nextX = Math.max(8, Math.min(x.value, width.value - 368))
  const nextY = Math.max(8, Math.min(y.value, height.value - 80))
  if (nextX !== x.value) x.value = nextX
  if (nextY !== y.value) y.value = nextY
})
const status = computed(() => editor.loading ? 'loading' : editor.saving ? 'saving' : editor.processing ? 'processing' : editor.playing ? 'playing' : editor.open ? 'ready' : 'idle')
const current = computed(() => snapshot.value?.editor)
const delta = computed(() => snapshot.value?.processBytes != null && baseline.value != null ? snapshot.value.processBytes - baseline.value : null)
const points = computed(() => {
  const values = history.value
  if (values.length < 2) return ''
  const low = Math.min(...values)
  const span = Math.max(1024 * 1024, Math.max(...values) - low)
  return values.map((n, i) => `${i * 312 / (values.length - 1)},${38 - (n - low) * 32 / span}`).join(' ')
})
const cursor = computed(() => {
  if (!editor.state) return '—'
  return `${((editor.playhead ?? editor.cursor) / editor.state.sampleRate).toFixed(3)} s`
})
</script>

<template>
  <Teleport to="body">
    <section ref="panel" role="region" :aria-label="t('debug.title')" class="debug-monitor" :style="{ left: `${x}px`, top: `${y}px`, maxHeight: `calc(100vh - ${y + 8}px)` }">
      <header ref="header" class="monitor-header">
        <Activity class="size-4 text-muted-foreground" />
        <h2 class="flex-1 text-sm font-medium">{{ t('debug.title') }}</h2>
        <Button variant="ghost" size="icon-xs" :aria-label="t(paused ? 'debug.resume' : 'debug.pause')" :aria-pressed="paused" @click="paused = !paused">
          <Play v-if="paused" class="size-3.5" /><Pause v-else class="size-3.5" />
        </Button>
        <Button variant="ghost" size="icon-xs" :aria-label="t('debug.close')" @click="debugMonitorOpen = false"><X class="size-4" /></Button>
      </header>
      <div class="monitor-content">
        <p v-if="snapshot?.demo" class="text-xs text-muted-foreground mb-2">{{ t('debug.demo') }}</p>
        <p v-if="error" role="status" class="text-destructive text-xs">{{ t('debug.error') }}</p>
        <div class="flex items-center justify-between text-xs text-muted-foreground">
          <span>{{ t(paused ? 'debug.paused' : 'debug.live') }}</span>
          <Button variant="ghost" size="xs" @click="reset"><RotateCcw class="size-3" />{{ t('debug.reset') }}</Button>
        </div>
        <div class="metrics-main">
          <div><span class="metric-label">{{ t('debug.process') }}</span><strong class="text-xl font-medium">{{ debugBytes(snapshot?.processBytes) }}</strong></div>
          <div><span class="metric-label">CPU</span><strong class="text-xl font-medium">{{ snapshot?.cpuPercent == null ? '—' : `${snapshot.cpuPercent.toFixed(1)}%` }}</strong></div>
        </div>
        <svg v-if="points" viewBox="0 0 312 42" class="w-full h-10 text-primary" aria-hidden="true"><polyline :points="points" fill="none" stroke="currentColor" stroke-width="1.5" /></svg>
        <dl class="metric-list">
          <div><dt>{{ t('debug.peak') }}</dt><dd>{{ debugBytes(peak) }}</dd></div>
          <div><dt>{{ t('debug.delta') }}</dt><dd>{{ delta != null && delta > 0 ? '+' : '' }}{{ debugBytes(delta) }}</dd></div>
          <div><dt>{{ t('debug.available') }}</dt><dd>{{ debugBytes(snapshot?.availableBytes) }}</dd></div>
        </dl>
        <div class="section-title">{{ t('debug.editor') }} <span class="status-label">{{ t(`debug.status.${status}`) }}</span></div>
        <p v-if="editor.sample" class="truncate text-xs mb-2">{{ editor.sample.name }}</p>
        <p v-if="snapshot?.editorBusy" class="text-xs text-muted-foreground">{{ t('debug.busy') }}</p>
        <dl v-else-if="current && current.sampleId === editor.sample?.id" class="metric-list">
          <div><dt>{{ t('debug.pcmRam') }}</dt><dd>{{ debugBytes(current.ramBytes) }}</dd></div>
          <div><dt>{{ t('debug.pcmDisk') }}</dt><dd>{{ debugBytes(current.diskBytes) }}</dd></div>
          <div><dt>{{ t('debug.logical') }}</dt><dd>{{ debugBytes(current.logicalBytes) }}</dd></div>
          <div><dt>{{ t('debug.cache') }}</dt><dd>{{ debugBytes(current.cacheBytes) }}</dd></div>
          <div><dt>{{ t('debug.storage') }}</dt><dd>{{ current.storageCount }}</dd></div>
          <div><dt>{{ t('debug.history') }}</dt><dd>{{ current.undoSteps }} / {{ current.redoSteps }}</dd></div>
          <div><dt>{{ t('debug.decode') }}</dt><dd>{{ current.openedMs.toFixed(1) }} ms</dd></div>
          <div><dt>{{ t('debug.wave') }}</dt><dd>{{ current.lastWaveMs == null ? '—' : `${current.lastWaveMs.toFixed(1)} ms` }}</dd></div>
          <div><dt>{{ t('debug.cursor') }}</dt><dd>{{ cursor }}</dd></div>
        </dl>
        <p v-else class="text-xs text-muted-foreground">{{ t(editor.loading ? 'debug.waiting' : 'debug.noTrack') }}</p>
        <p class="monitor-note">{{ t('debug.note') }}</p>
      </div>
    </section>
  </Teleport>
</template>

<style scoped>
.debug-monitor { position: fixed; z-index: 60; width: min(360px, calc(100vw - 16px)); overflow: auto; border: 1px solid var(--border); border-radius: 12px; background: var(--popover); color: var(--popover-foreground); box-shadow: 0 12px 36px #0004; }
.monitor-header { display: flex; align-items: center; gap: 8px; padding: 10px 12px; border-bottom: 1px solid var(--border); cursor: grab; touch-action: none; }
.monitor-header:active { cursor: grabbing; }
.monitor-content { padding: 8px 16px 14px; }
.metrics-main { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin: 6px 0 10px; font-variant-numeric: tabular-nums; }
.metric-label { display: block; font-size: 11px; color: var(--muted-foreground); margin-bottom: 2px; }
.metric-list { display: grid; gap: 5px; font-size: 11px; }
.metric-list > div { display: flex; justify-content: space-between; gap: 16px; }
.metric-list dt { color: var(--muted-foreground); }
.metric-list dd { font-variant-numeric: tabular-nums; white-space: nowrap; }
.section-title { display: flex; justify-content: space-between; gap: 8px; border-top: 1px solid var(--border); margin: 12px 0 8px; padding-top: 12px; font-size: 12px; font-weight: 500; }
.status-label { font-size: 11px; color: var(--muted-foreground); font-weight: normal; }
.monitor-note { margin-top: 12px; color: var(--muted-foreground); font-size: 10px; line-height: 1.5; }
</style>
