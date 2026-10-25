<script setup lang="ts">
import type { DuplicateGroup, Sample } from '@/api'
import { Copy, Search } from '@lucide/vue'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { api } from '@/api'
import CategoryIcon from '@/components/icons/CategoryIcon.vue'
import { CommandGroup, CommandItem } from '@/components/ui/command'
import { Spinner } from '@/components/ui/spinner'
import { matchingDuplicateGroups } from '@/duplicate-search'
import { folderName, formatDuration, formatNumber, shortPath } from '@/format'

const props = defineProps<{
  open: boolean
  query: string
  mode: 'all' | 'tracks' | 'duplicates' | 'commands'
  duplicateView: 'groups' | 'files'
}>()
const emit = defineEmits<{
  sample: [sample: Sample]
  duplicates: [query: string, groupKey?: string, copyId?: number]
  library: [query: string]
  state: [value: { loading: boolean, count: number, error: boolean }]
}>()
const { t } = useI18n()
const tracks = ref<Sample[]>([])
const duplicateGroups = ref<DuplicateGroup[]>([])
const loading = ref(false)
const error = ref(false)
const query = computed(() => props.query.trim())
const duplicateRows = computed(() => props.duplicateView === 'files'
  ? duplicateGroups.value.flatMap(group => group.copies.map(copy => ({ group, copy }))).slice(0, 8)
  : duplicateGroups.value.slice(0, 6).map(group => ({ group, copy: group.copies[0]! })))
let duplicateSnapshot: Promise<DuplicateGroup[]> | null = null

watch([() => props.open, query, () => props.mode], ([open, q, mode], _, cleanup) => {
  let cancelled = false
  tracks.value = []
  duplicateGroups.value = []
  error.value = false
  loading.value = false
  if (!open) duplicateSnapshot = null
  if (!open || mode === 'commands' || (!q && mode !== 'duplicates')) return
  loading.value = true
  const timer = setTimeout(async () => {
    const requests = await Promise.allSettled([
      mode !== 'duplicates' && q ? api.listSamples({ query: q, groupStems: false }, 0, 8) : Promise.resolve([]),
      mode !== 'tracks' ? (duplicateSnapshot ??= api.duplicateGroups().catch(e => { duplicateSnapshot = null; throw e })) : Promise.resolve([]),
    ])
    if (cancelled) return
    const [samples, duplicates] = requests
    tracks.value = samples.status === 'fulfilled' ? samples.value : []
    duplicateGroups.value = duplicates.status === 'fulfilled' ? matchingDuplicateGroups(duplicates.value, q) : []
    error.value = requests.some(result => result.status === 'rejected')
    loading.value = false
  }, 180)
  cleanup(() => { cancelled = true; clearTimeout(timer) })
}, { immediate: true, flush: 'sync' })
watch([loading, tracks, duplicateRows, error], () => emit('state', {
  loading: loading.value, count: tracks.value.length + duplicateRows.value.length, error: error.value,
}), { immediate: true })
</script>

<template>
  <p v-if="mode === 'tracks' && !query" class="px-3 py-6 text-center text-sm text-muted-foreground">{{ t('palette.startTyping') }}</p>
  <div v-if="loading" role="status" class="flex items-center gap-2 px-3 py-3 text-xs text-muted-foreground">
    <Spinner class="size-3" /> {{ t('palette.searching') }}
  </div>
  <p v-if="error" role="status" class="px-3 py-2 text-xs text-destructive">{{ t('palette.searchFailed') }}</p>
  <CommandGroup v-if="tracks.length" :heading="t('palette.groups.tracks')">
    <CommandItem v-for="sample in tracks" :key="sample.id" :value="`result-track-${sample.id}`" :keywords="[query]" @select="emit('sample', sample)">
      <CategoryIcon :category="sample.category" :stem="sample.stemKind" />
      <span class="min-w-0 flex-1">
        <span class="block truncate">{{ sample.name }}</span>
        <span class="block truncate text-xs text-muted-foreground">{{ sample.path }}</span>
      </span>
      <span class="shrink-0 text-xs text-muted-foreground tabular">{{ formatDuration(sample.duration) }}</span>
    </CommandItem>
    <CommandItem :value="'result-all-tracks'" :keywords="[query]" @select="emit('library', query)">
      <Search /> {{ t('palette.showAllResults') }}
    </CommandItem>
  </CommandGroup>
  <CommandGroup v-if="duplicateRows.length" :heading="t('palette.groups.duplicates')">
    <CommandItem v-for="{ group, copy } in duplicateRows" :key="`${group.key}:${copy.id}`" :value="`result-duplicate-${group.key}-${copy.id}`" :keywords="[query]" @select="emit('duplicates', query || folderName(copy.path), group.key, copy.id)">
      <Copy />
      <span class="min-w-0 flex-1">
        <span class="block truncate">{{ folderName(copy.path) }}</span>
        <span class="block truncate text-xs text-muted-foreground">{{ duplicateView === 'files' ? copy.path : `${t('palette.copies', { n: formatNumber(group.copies.length) }, group.copies.length)} · ${shortPath(copy.path, 0, 3)}` }}</span>
      </span>
      <span class="shrink-0 text-xs text-muted-foreground">{{ t(group.verified ? 'palette.verified' : 'palette.verifying') }}</span>
    </CommandItem>
    <CommandItem :value="'result-all-duplicates'" :keywords="[query]" @select="emit('duplicates', query)">
      <Copy /> {{ t('palette.showAllDuplicates') }}
    </CommandItem>
  </CommandGroup>
</template>
