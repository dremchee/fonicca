<script setup lang="ts">
import { AlertTriangle, CircleCheck, CircleX, FolderOpen, HardDrive } from '@lucide/vue'
import { computed, onBeforeUnmount, reactive, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectSeparator, SelectTrigger, SelectValue } from '@/components/ui/select'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { formatSize } from '@/format'

const open = defineModel<boolean>('open', { required: true })

type Channels = 'mono' | 'stereo' | 'keep'

interface Profile {
  id: string
  name: string
  rate: number
  bits: number
  channels: Channels
  /** Longest file name without extension; null for no limit. Not verified against manuals yet. */
  maxName: number | null
  maxSamples: number | null
  /** Folder inside the destination; {kit} is the kit name. */
  folder: string
  hint: string
  /** Shows SP-404 style pad numbers (banks of 16). */
  pads: boolean
}

const PROFILES: Profile[] = [
  { id: 'digitakt', name: 'Elektron Digitakt', rate: 48_000, bits: 16, channels: 'mono', maxName: 24, maxSamples: null, folder: '{kit}', hint: 'Drag the kit folder into Elektron Transfer.', pads: false },
  { id: 'sp404', name: 'Roland SP-404 MkII', rate: 48_000, bits: 16, channels: 'keep', maxName: 16, maxSamples: 160, folder: 'ROLAND/SP-404MKII/IMPORT/{kit}', hint: 'On the SP-404, open the IMPORT folder and load the kit into a project.', pads: true },
]

// Fake data standing in for the collection and the backend's analysis of each file.
const samples = [
  { name: 'Kick Rumble 01', rate: 44_100, bits: 24, ch: 2, dur: 0.8, ok: true },
  { name: 'Kick Punchy Short', rate: 48_000, bits: 16, ch: 1, dur: 0.4, ok: true },
  { name: 'Clap Wide Verb', rate: 44_100, bits: 24, ch: 2, dur: 1.2, ok: true },
  { name: 'Snare Rim Tight', rate: 48_000, bits: 24, ch: 1, dur: 0.3, ok: true },
  { name: 'Hat Closed 909', rate: 44_100, bits: 16, ch: 1, dur: 0.15, ok: true },
  { name: 'Hat Open 909', rate: 44_100, bits: 16, ch: 1, dur: 0.6, ok: true },
  { name: 'Ride Dry', rate: 96_000, bits: 32, ch: 2, dur: 2.4, ok: true },
  { name: 'Perc Wood Block', rate: 48_000, bits: 16, ch: 1, dur: 0.2, ok: true },
  { name: 'Bass Stab Acid 128 Am', rate: 44_100, bits: 24, ch: 1, dur: 1.9, ok: true },
  { name: 'Bass Stab Acid 128 Cm', rate: 44_100, bits: 24, ch: 1, dur: 1.9, ok: true },
  { name: 'Chord Dub Stab Dm', rate: 48_000, bits: 24, ch: 2, dur: 3.1, ok: true },
  { name: 'Vox Hey Chopped', rate: 44_100, bits: 16, ch: 2, dur: 0.7, ok: true },
  { name: 'Loop Top 128 Groove', rate: 44_100, bits: 24, ch: 2, dur: 7.5, ok: true },
  { name: 'Loop Perc 128 Shuffle', rate: 44_100, bits: 24, ch: 2, dur: 7.5, ok: true },
  { name: 'FX Riser Noise', rate: 48_000, bits: 24, ch: 2, dur: 8.0, ok: true },
  { name: 'FX Impact Down', rate: 48_000, bits: 24, ch: 2, dur: 4.2, ok: true },
  { name: 'Tom Low Analog', rate: 44_100, bits: 16, ch: 1, dur: 0.9, ok: true },
  { name: 'Broken Export', rate: 48_000, bits: 24, ch: 2, dur: 1, ok: false },
]

const deviceId = ref('sp404')
const custom = reactive({ rate: '44100', bits: '16', channels: 'keep' as Channels })
const kitName = ref('Techno Kit')
const numbered = ref(true)
const destination = ref('sd')

const profile = computed<Profile>(() => PROFILES.find(p => p.id === deviceId.value) ?? {
  id: 'custom',
  name: 'Custom',
  rate: Number(custom.rate),
  bits: Number(custom.bits),
  channels: custom.channels,
  maxName: null,
  maxSamples: null,
  folder: '{kit}',
  hint: 'Plain WAV files in one folder.',
  pads: false,
})

const destinations = computed(() => [
  ...(deviceId.value === 'sp404' ? [{ id: 'sd', label: 'SD_404', path: '/Volumes/SD_404', card: true }] : []),
  { id: 'desktop', label: 'Desktop', path: '~/Desktop', card: false },
])
watch(destinations, (list) => {
  if (!list.some(d => d.id === destination.value))
    destination.value = list[0]!.id
})

const outputFolder = computed(() => {
  const base = destinations.value.find(d => d.id === destination.value)?.path ?? '~/Desktop'
  return `${base}/${profile.value.folder.replace('{kit}', kitName.value.trim() || 'Kit')}/`
})

function pad(i: number) {
  return `${'ABCDEFGHIJ'[Math.floor(i / 16)] ?? '?'}${(i % 16) + 1}`
}

/** Keeps characters every sampler file system takes. */
function safeName(name: string) {
  return name.normalize('NFKD').replace(/[^\w\- ]/g, '').replace(/\s+/g, ' ').trim()
}

type Status = 'ok' | 'warn' | 'skip'

const rows = computed(() => {
  const p = profile.value
  const used = new Set<string>()
  let index = 0
  return samples.map((s) => {
    // Shortened names are expected on samplers and only noted; renames and skips are issues.
    const notes: { text: string, issue: boolean }[] = []
    if (!s.ok)
      return { s, file: '—', pad: '', changes: [], notes: [{ text: 'Cannot be decoded', issue: true }], status: 'skip' as Status, bytes: 0 }
    if (p.maxSamples != null && index >= p.maxSamples)
      return { s, file: '—', pad: '', changes: [], notes: [{ text: `Over the ${p.maxSamples}-sample limit`, issue: true }], status: 'skip' as Status, bytes: 0 }

    const prefix = numbered.value ? `${String(index + 1).padStart(2, '0')} ` : ''
    let base = prefix + safeName(s.name)
    if (p.maxName != null && base.length > p.maxName) {
      base = base.slice(0, p.maxName).trimEnd()
      notes.push({ text: `name shortened to ${p.maxName} characters`, issue: false })
    }
    if (used.has(base.toLowerCase())) {
      const suffix = ` ${index + 1}`
      base = (p.maxName != null ? base.slice(0, p.maxName - suffix.length) : base) + suffix
      notes.push({ text: 'Renamed: another file got the same name', issue: true })
    }
    used.add(base.toLowerCase())

    const changes: string[] = []
    if (s.rate !== p.rate)
      changes.push(`${s.rate / 1000}→${p.rate / 1000} kHz`)
    if (s.bits !== p.bits)
      changes.push(`${s.bits === 32 ? '32f' : s.bits}→${p.bits}-bit`)
    const outCh = p.channels === 'keep' ? s.ch : p.channels === 'mono' ? 1 : 2
    if (outCh !== s.ch)
      changes.push(outCh === 1 ? 'Stereo→mono' : 'Mono→stereo')

    const bytes = Math.round(s.dur * p.rate * (p.bits / 8) * outCh) + 44
    const row = { s, file: `${base}.wav`, pad: p.pads ? pad(index) : String(index + 1), changes, notes, status: (notes.some(n => n.issue) ? 'warn' : 'ok') as Status, bytes }
    index++
    return row
  })
})

const counts = computed(() => ({
  ok: rows.value.filter(r => r.status === 'ok').length,
  warn: rows.value.filter(r => r.status === 'warn').length,
  skip: rows.value.filter(r => r.status === 'skip').length,
}))
const exportable = computed(() => counts.value.ok + counts.value.warn)
const totalBytes = computed(() => rows.value.reduce((n, r) => n + r.bytes, 0))
const issuesOnly = ref(false)
const visibleRows = computed(() => issuesOnly.value ? rows.value.filter(r => r.status !== 'ok') : rows.value)

const phase = ref<'review' | 'exporting' | 'done'>('review')
const done = ref(0)
let timer = 0

function start() {
  phase.value = 'exporting'
  done.value = 0
  timer = window.setInterval(() => {
    done.value++
    if (done.value >= exportable.value) {
      clearInterval(timer)
      phase.value = 'done'
    }
  }, 140)
}

function cancel() {
  clearInterval(timer)
  phase.value = 'review'
  toast('Export cancelled. Files written so far were removed.')
}

watch(open, (v) => {
  if (!v) {
    clearInterval(timer)
    phase.value = 'review'
  }
})
onBeforeUnmount(() => clearInterval(timer))

function formatSpec(p: Profile) {
  const ch = p.channels === 'keep' ? 'channels as is' : p.channels
  return `WAV · ${p.rate / 1000} kHz · ${p.bits}-bit · ${ch}`
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent
      class="p-0 gap-0 sm:max-w-5xl h-[min(620px,calc(100vh-4rem))] grid-rows-[auto_minmax(0,1fr)_auto] overflow-hidden"
      @interact-outside="phase === 'exporting' && $event.preventDefault()"
    >
      <header class="h-13 shrink-0 px-6 flex items-center gap-2 border-b border-border">
        <DialogTitle class="text-base font-semibold">
          Export kit
        </DialogTitle>
        <span class="text-muted-foreground">· Techno Kit, {{ samples.length }} samples</span>
        <DialogDescription class="sr-only">
          Convert the collection for a hardware sampler and write it to a folder or card.
        </DialogDescription>
      </header>

      <div class="min-h-0 grid grid-cols-[288px_minmax(0,1fr)]">
        <!-- Options -->
        <div class="border-r border-border p-5 flex flex-col gap-5 overflow-y-auto" :class="{ 'opacity-60 pointer-events-none': phase !== 'review' }">
          <div class="field">
            <Label>Device</Label>
            <Select v-model="deviceId">
              <SelectTrigger size="sm" class="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem v-for="p in PROFILES" :key="p.id" :value="p.id">
                  {{ p.name }}
                </SelectItem>
                <SelectSeparator />
                <SelectItem value="custom">
                  Custom…
                </SelectItem>
              </SelectContent>
            </Select>
            <p class="hint tabular">
              {{ formatSpec(profile) }}
            </p>
          </div>

          <template v-if="deviceId === 'custom'">
            <div class="grid grid-cols-2 gap-3">
              <div class="field">
                <Label>Sample rate</Label>
                <Select v-model="custom.rate">
                  <SelectTrigger size="sm" class="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem v-for="r in ['44100', '48000', '96000']" :key="r" :value="r">
                      {{ Number(r) / 1000 }} kHz
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div class="field">
                <Label>Bit depth</Label>
                <Select v-model="custom.bits">
                  <SelectTrigger size="sm" class="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem v-for="b in ['16', '24']" :key="b" :value="b">
                      {{ b }}-bit
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
            <div class="field">
              <Label>Channels</Label>
              <ToggleGroup
                :model-value="custom.channels"
                type="single"
                variant="outline"
                size="sm"
                class="w-full"
                @update:model-value="$event && (custom.channels = $event as Channels)"
              >
                <ToggleGroupItem value="keep" class="flex-1">
                  As is
                </ToggleGroupItem>
                <ToggleGroupItem value="mono" class="flex-1">
                  Mono
                </ToggleGroupItem>
                <ToggleGroupItem value="stereo" class="flex-1">
                  Stereo
                </ToggleGroupItem>
              </ToggleGroup>
            </div>
          </template>

          <div class="field">
            <Label for="kit-name">Kit name</Label>
            <Input id="kit-name" v-model="kitName" class="h-8" />
          </div>

          <div class="field">
            <Label>Destination</Label>
            <Select v-model="destination">
              <SelectTrigger size="sm" class="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem v-for="d in destinations" :key="d.id" :value="d.id">
                  <component :is="d.card ? HardDrive : FolderOpen" class="size-4" />
                  {{ d.label }}
                </SelectItem>
                <SelectSeparator />
                <SelectItem value="choose" @select.prevent="toast('Opens a folder picker')">
                  Choose folder…
                </SelectItem>
              </SelectContent>
            </Select>
            <p class="hint break-all select-text">
              {{ outputFolder }}
            </p>
          </div>

          <label class="flex items-center gap-2">
            <Checkbox v-model="numbered" />
            <span>Number files in collection order</span>
          </label>

          <p class="hint mt-auto">
            {{ profile.hint }}
          </p>
        </div>

        <!-- Review -->
        <div class="min-w-0 min-h-0 flex flex-col">
          <div class="h-10 shrink-0 px-4 flex items-center gap-4 border-b border-border text-xs tabular">
            <span class="flex items-center gap-1.5"><CircleCheck class="size-3.5 text-primary" /> {{ counts.ok }} ready</span>
            <span v-if="counts.warn" class="flex items-center gap-1.5"><AlertTriangle class="size-3.5 text-amber-500" /> {{ counts.warn }} renamed</span>
            <span v-if="counts.skip" class="flex items-center gap-1.5"><CircleX class="size-3.5 text-destructive" /> {{ counts.skip }} skipped</span>
            <label class="ml-auto flex items-center gap-2 text-muted-foreground">
              <Checkbox v-model="issuesOnly" />
              Only issues
            </label>
          </div>
          <div class="flex-1 overflow-y-auto">
            <table class="w-full text-left table-fixed">
              <colgroup>
                <col class="w-14">
                <col>
                <col class="w-52">
                <col class="w-10">
              </colgroup>
              <thead class="sticky top-0 bg-background text-[11px] text-muted-foreground">
                <tr>
                  <th>
                    {{ profile.pads ? 'Pad' : '#' }}
                  </th>
                  <th>File</th>
                  <th>Conversion</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                <tr v-for="r in visibleRows" :key="r.s.name" :class="{ 'text-muted-foreground': r.status === 'skip' }">
                  <td class="tabular text-muted-foreground">
                    {{ r.pad }}
                  </td>
                  <td>
                    <div class="truncate font-medium" :title="r.file">
                      {{ r.file }}
                    </div>
                    <div class="truncate text-[11px] text-muted-foreground">
                      {{ r.s.name }}<template v-for="n in r.notes" :key="n.text">
                        · <span :class="!n.issue ? '' : r.status === 'skip' ? 'text-destructive' : 'text-amber-600 dark:text-amber-400'">{{ n.text }}</span>
                      </template>
                    </div>
                  </td>
                  <td>
                    <div class="flex flex-wrap gap-1">
                      <span v-for="c in r.changes" :key="c" class="chip">{{ c }}</span>
                      <span v-if="r.status !== 'skip' && !r.changes.length" class="text-[11px] text-muted-foreground">Copied as is</span>
                    </div>
                  </td>
                  <td>
                    <CircleCheck v-if="r.status === 'ok'" class="size-4 text-primary" />
                    <AlertTriangle v-else-if="r.status === 'warn'" class="size-4 text-amber-500" />
                    <CircleX v-else class="size-4 text-destructive" />
                  </td>
                </tr>
              </tbody>
            </table>
            <p v-if="!visibleRows.length" class="p-8 text-center text-muted-foreground">
              No issues
            </p>
          </div>
        </div>
      </div>

      <footer class="h-14 shrink-0 px-5 flex items-center gap-3 border-t border-border">
        <template v-if="phase === 'review'">
          <span class="text-muted-foreground tabular">{{ exportable }} files · {{ formatSize(totalBytes) }}</span>
          <Button variant="ghost" size="sm" class="ml-auto" @click="open = false">
            Cancel
          </Button>
          <Button size="sm" :disabled="!exportable || !kitName.trim()" @click="start">
            Export {{ exportable }} files
          </Button>
        </template>
        <template v-else-if="phase === 'exporting'">
          <div class="flex-1 flex items-center gap-3">
            <span class="text-muted-foreground tabular w-40">Converting {{ done }} of {{ exportable }}…</span>
            <div class="flex-1 h-1 rounded-full bg-muted overflow-hidden">
              <div class="h-full bg-primary transition-[width]" :style="{ width: `${done / exportable * 100}%` }" />
            </div>
          </div>
          <Button variant="outline" size="sm" @click="cancel">
            Stop
          </Button>
        </template>
        <template v-else>
          <CircleCheck class="size-4 text-primary" />
          <span>Exported {{ exportable }} files to {{ outputFolder }}</span>
          <Button variant="outline" size="sm" class="ml-auto" @click="toast(`Reveal ${outputFolder}`)">
            <FolderOpen /> Show in Finder
          </Button>
          <Button size="sm" @click="open = false">
            Done
          </Button>
        </template>
      </footer>
    </DialogContent>
  </Dialog>
</template>

<style scoped>
.field {
  display: flex;
  flex-direction: column;
  gap: 0.375rem;
}

.hint {
  font-size: 12px;
  color: var(--muted-foreground);
}

th {
  height: 2rem;
  font-weight: 500;
  padding-right: 0.75rem;
  border-bottom: 1px solid var(--border);
}

td {
  padding: 0.375rem 0.75rem 0.375rem 0;
  border-bottom: 1px solid var(--border);
  vertical-align: middle;
}

th:first-child,
td:first-child {
  padding-left: 1rem;
}

.chip {
  font-size: 11px;
  line-height: 1.25rem;
  padding: 0 0.375rem;
  border-radius: calc(var(--radius) - 4px);
  background: var(--muted);
  color: var(--muted-foreground);
  white-space: nowrap;
}
</style>
