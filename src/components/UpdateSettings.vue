<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronRight, Download, RefreshCw } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import { formatSize } from '@/format'
import ReleaseNotes from '@/components/ReleaseNotes.vue'
import { useUpdatesStore } from '@/stores/updates'

const { t } = useI18n()
const updates = useUpdatesStore()
const status = computed(() => {
  if (updates.availability === 'loading') return t('updates.checking')
  if (updates.availability !== 'enabled') return t(`updates.${updates.availability}`)
  if (['available', 'downloading', 'ready'].includes(updates.state.phase)) return t('updates.available', { version: updates.state.version })
  if (updates.state.phase === 'checking') return t('updates.checking')
  if (updates.state.phase === 'current') return t('updates.current')
  if (updates.state.phase === 'installing') return t('updates.installing')
  if (updates.state.phase === 'installed') return t('updates.installed')
  return t('updates.automatic')
})
</script>

<template>
  <section class="rounded-lg border p-4 flex flex-col gap-3" :aria-label="t('updates.title')">
    <div class="flex items-center justify-between gap-4">
      <div class="min-w-0" role="status" aria-live="polite">
        <h3 class="font-medium">{{ t('updates.title') }}</h3>
        <p class="mt-1 text-muted-foreground text-xs">{{ status }}</p>
      </div>
      <template v-if="updates.availability === 'enabled'">
        <Button v-if="updates.state.phase === 'available'" size="sm" @click="updates.download()">
          <Download />{{ t('updates.download') }}
        </Button>
        <Button v-else-if="['ready', 'installed'].includes(updates.state.phase)" size="sm" :disabled="updates.busy" @click="updates.install()">
          {{ t(updates.state.phase === 'installed' ? 'updates.restart' : 'updates.install') }}
        </Button>
        <Button v-else size="sm" variant="outline" :disabled="updates.busy" @click="updates.check()">
          <Spinner v-if="updates.busy" /><RefreshCw v-else />{{ t('updates.check') }}
        </Button>
      </template>
    </div>
    <div v-if="updates.state.phase === 'downloading'" class="flex flex-col gap-2">
      <div class="flex justify-between text-xs text-muted-foreground tabular">
        <span>{{ t('updates.downloading') }}</span>
        <span>{{ updates.percent === null ? formatSize(updates.state.received) : `${updates.percent}%` }}</span>
      </div>
      <progress class="w-full h-1.5 accent-primary" :value="updates.percent ?? undefined" max="100" :aria-label="t('updates.downloading')" />
    </div>
    <p v-if="updates.state.phase === 'ready'" class="text-xs text-muted-foreground">{{ t('updates.ready') }}</p>
    <details v-if="updates.state.notes" class="group border-t pt-3">
      <summary class="flex w-fit cursor-pointer list-none items-center gap-1.5 rounded-sm text-xs font-medium text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring [&::-webkit-details-marker]:hidden">
        <ChevronRight class="size-3.5 shrink-0 group-open:rotate-90" aria-hidden="true" />
        {{ t('updates.notes') }}
      </summary>
      <ReleaseNotes class="mt-3" :notes="updates.state.notes" />
    </details>
    <div v-if="updates.state.error" role="alert" class="text-xs text-destructive">
      <p>{{ t('updates.failed') }}</p>
      <p class="mt-1 break-words select-text">{{ updates.state.error }}</p>
    </div>
  </section>
</template>
