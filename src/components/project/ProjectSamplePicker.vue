<script setup lang="ts">
import type { Sample } from '@/api'
import { watchDebounced } from '@vueuse/core'
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { api } from '@/api'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { formatDuration } from '@/format'
import { useProjectStore } from '@/stores/project'
const open = defineModel<boolean>({ required: true })
const project = useProjectStore()
const { t } = useI18n()
const query = ref('')
const rows = ref<Sample[]>([])
const chosen = ref(new Set<number>())
const error = ref('')
const loading = ref(false)
let revision = 0
async function search() {
  const version = ++revision
  if (!open.value) return
  loading.value = true; error.value = ''
  try {
    const result = await api.listSamples({ query: query.value }, 0, 100)
    if (version === revision) rows.value = result
  }
  catch (e) { if (version === revision) error.value = String(e) }
  finally { if (version === revision) loading.value = false }
}
watch(open, value => { if (value) { chosen.value = new Set(); void search() } else revision++ })
watchDebounced(query, search, { debounce: 200 })
async function add() { await project.addSamples([...chosen.value]); if (!project.error) open.value = false }
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader><DialogTitle>{{ t('project.addSamples') }}</DialogTitle><DialogDescription>{{ t('project.emptyHint') }}</DialogDescription></DialogHeader>
      <Input v-model="query" type="search" :placeholder="t('project.search')" :aria-label="t('project.search')" />
      <p v-if="error" role="alert" class="text-xs text-destructive">{{ error }}</p>
      <div class="h-72 overflow-y-auto divide-y divide-border" :aria-busy="loading">
        <label v-for="sample in rows" :key="sample.id" class="flex items-center gap-2 py-2 text-xs cursor-pointer">
          <input type="checkbox" :checked="chosen.has(sample.id)" :disabled="!chosen.has(sample.id) && chosen.size >= 32" @change="chosen.has(sample.id) ? chosen.delete(sample.id) : chosen.add(sample.id)">
          <span class="min-w-0 flex-1 truncate" :title="sample.path">{{ sample.name }}</span><span class="tabular text-muted-foreground">{{ formatDuration(sample.duration) }}</span>
        </label>
        <p v-if="!rows.length && !loading" class="py-4 text-xs text-muted-foreground">{{ t('project.noSamples') }}</p>
      </div>
      <DialogFooter><Button :disabled="!chosen.size" @click="add">{{ t('project.choose') }}<span v-if="chosen.size"> · {{ chosen.size }}</span></Button></DialogFooter>
    </DialogContent>
  </Dialog>
</template>
