<script setup lang="ts">
import type { BatchDone, BatchProgress, Destination } from '@/composables/batch'
import type { Curve } from '@/stores/editor'
import { listen } from '@tauri-apps/api/event'
import { open as pickFolder } from '@tauri-apps/plugin-dialog'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { useLocalStorage, watchDebounced } from '@vueuse/core'
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import BatchNormalizeStep from '@/components/BatchNormalizeStep.vue'
import BatchLoudnessReport from '@/components/BatchLoudnessReport.vue'
import { Field, FieldDescription, FieldLabel } from '@/components/ui/field'
import type { BatchForm } from '@/batch-recipe'
import { batchDefaults, batchRecipe, restoreBatchForm, validBatchForm } from '@/batch-recipe'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { BATCH_DONE, BATCH_PROGRESS, batchApi, batchFor } from '@/composables/batch'
import { formatNumber, shortPath } from '@/format'

const { t } = useI18n()

// The dialog's settings: every step with its values, whether it is on or not, so
// switching a step off and on again keeps them. Kept between runs.
const form = useLocalStorage<BatchForm>('batch.form', { ...batchDefaults }, { mergeDefaults: true })
const presets = useLocalStorage<{ name: string, form: BatchForm }[]>('batch.presets', [])
const destination = useLocalStorage<Destination>('batch.destination', { mode: 'folder', path: '' })
const preset = ref('')

const ids = computed(() => batchFor.value ?? [])
const open = computed({
  get: () => batchFor.value !== null,
  set: (v) => {
    if (!v && !running.value)
      batchFor.value = null
  },
})

const KEEP = 'keep'
const curves: Curve[] = ['smooth', 'linear', 'exponential']
const rates = ['44100', '48000', '88200', '96000']
const tokens = ['name', 'n', 'category', 'bpm', 'key', 'kind']
const tokenLabel = (token: string) => `{${token}}`

const inPlace = computed(() => destination.value.mode === 'inPlace')
const folder = computed(() => (destination.value.mode === 'folder' ? destination.value.path : ''))
const anyStep = computed(() => {
  const f = form.value
  return f.trim || f.dcOffset || f.normalize || f.fades || f.format || (f.rename && !inPlace.value)
})
const canRun = computed(() => anyStep.value && validBatchForm(form.value) && (inPlace.value || folder.value !== ''))

async function chooseFolder() {
  const path = await pickFolder({ directory: true, defaultPath: folder.value || undefined })
  if (typeof path === 'string')
    destination.value = { mode: 'folder', path }
}

// --- Presets ----------------------------------------------------------------------

const naming = ref(false)
const presetName = ref('')

function applyPreset(name: unknown) {
  const p = presets.value.find(p => p.name === name)
  if (p) {
    form.value = restoreBatchForm(p.form)
    preset.value = p.name
  }
}

function savePreset() {
  const name = presetName.value.trim()
  if (!name)
    return
  presets.value = [...presets.value.filter(p => p.name !== name), { name, form: { ...form.value } }]
  preset.value = name
  naming.value = false
  presetName.value = ''
}

function deletePreset() {
  presets.value = presets.value.filter(p => p.name !== preset.value)
  preset.value = ''
}

// Editing the settings leaves the preset they came from.
watch(form, () => {
  const p = presets.value.find(p => p.name === preset.value)
  if (p && JSON.stringify(restoreBatchForm(p.form)) !== JSON.stringify(form.value))
    preset.value = ''
}, { deep: true })

// --- Name preview -------------------------------------------------------------

const examples = ref<string[]>([])
watchDebounced([() => form.value.template, () => form.value.rename, ids], async () => {
  if (!form.value.rename || !ids.value.length) {
    examples.value = []
    return
  }
  try {
    examples.value = await batchApi.preview(ids.value.slice(0, 3), form.value.template, ids.value.length)
  }
  catch {
    examples.value = []
  }
}, { debounce: 150, immediate: true })

const templateInput = ref<InstanceType<typeof Input> | null>(null)
function insertToken(token: string) {
  const el = templateInput.value?.$el as HTMLInputElement | undefined
  const text = `{${token}}`
  const at = el?.selectionStart ?? form.value.template.length
  const end = el?.selectionEnd ?? at
  form.value.template = form.value.template.slice(0, at) + text + form.value.template.slice(end)
  requestAnimationFrame(() => {
    el?.focus()
    el?.setSelectionRange(at + text.length, at + text.length)
  })
}

// --- Running --------------------------------------------------------------------

const running = ref(false)
const progress = ref<BatchProgress | null>(null)
const result = ref<BatchDone | null>(null)
const unlisten: (() => void)[] = []

async function run() {
  if (!canRun.value || running.value)
    return
  result.value = null
  progress.value = { done: 0, total: ids.value.length, name: '' }
  running.value = true
  try {
    unlisten.push(await listen<BatchProgress>(BATCH_PROGRESS, e => (progress.value = e.payload)))
    unlisten.push(await listen<BatchDone>(BATCH_DONE, (e) => {
      result.value = e.payload
      finish()
    }))
    await batchApi.run(ids.value, batchRecipe(form.value, inPlace.value), destination.value)
  }
  catch (e) {
    toast.error(String(e))
    finish()
  }
}

function finish() {
  running.value = false
  unlisten.splice(0).forEach(u => u())
}
onBeforeUnmount(finish)

// A new set of samples starts with a clean result.
watch(batchFor, () => {
  if (!running.value) {
    result.value = null
    progress.value = null
  }
})
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-lg max-h-[90vh] overflow-y-auto">
      <DialogHeader>
        <DialogTitle>{{ t('batch.title', { n: formatNumber(ids.length) }, ids.length) }}</DialogTitle>
        <DialogDescription>{{ t('batch.description') }}</DialogDescription>
      </DialogHeader>

      <!-- Presets -->
      <div class="flex items-center gap-2 text-sm">
        <span class="text-muted-foreground shrink-0">{{ t('batch.presets.label') }}</span>
        <template v-if="!naming">
          <Select :model-value="preset" :disabled="!presets.length" @update:model-value="applyPreset">
            <SelectTrigger size="sm" class="flex-1 min-w-0" :aria-label="t('batch.presets.label')">
              <SelectValue>{{ preset || t('batch.presets.none') }}</SelectValue>
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="p in presets" :key="p.name" :value="p.name">
                {{ p.name }}
              </SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" size="sm" @click="naming = true; presetName = preset">
            {{ t('batch.presets.save') }}
          </Button>
          <Button v-if="preset" variant="ghost" size="sm" class="text-muted-foreground" @click="deletePreset()">
            {{ t('batch.presets.delete') }}
          </Button>
        </template>
        <form v-else class="flex flex-1 gap-2" @submit.prevent="savePreset()">
          <Input v-model="presetName" :placeholder="t('batch.presets.name')" class="h-8 flex-1" autofocus @keydown.esc.stop="naming = false" />
          <Button type="submit" size="sm" :disabled="!presetName.trim()">
            {{ t('common.actions.save') }}
          </Button>
          <Button type="button" variant="ghost" size="sm" @click="naming = false">
            {{ t('common.actions.cancel') }}
          </Button>
        </form>
      </div>

      <fieldset :disabled="running" class="flex flex-col divide-y divide-border rounded-lg border border-border text-sm">
        <!-- Trim -->
        <section class="p-3 flex flex-col gap-3">
          <label class="flex items-center justify-between font-medium">
            {{ t('batch.trim.title') }}
            <Switch v-model="form.trim" />
          </label>
          <div v-if="form.trim" class="grid grid-cols-2 gap-3">
            <label class="flex flex-col gap-1 text-muted-foreground">
              {{ t('batch.trim.threshold') }}
              <div class="flex items-center gap-1.5"><Input v-model.number="form.thresholdDb" type="number" min="-100" max="0" class="h-8" /> {{ t('common.units.dB') }}</div>
            </label>
            <label class="flex flex-col gap-1 text-muted-foreground">
              {{ t('batch.trim.pad') }}
              <div class="flex items-center gap-1.5"><Input v-model.number="form.padMs" type="number" min="0" class="h-8" /> {{ t('batch.msUnit') }}</div>
            </label>
          </div>
        </section>

        <section class="p-3">
          <Field orientation="horizontal">
            <FieldLabel for="batch-dc-offset" class="font-medium">{{ t('batch.dcOffset.title') }}</FieldLabel>
            <Switch id="batch-dc-offset" v-model="form.dcOffset" />
          </Field>
          <FieldDescription v-if="form.dcOffset" class="mt-2">{{ t('batch.dcOffset.hint') }}</FieldDescription>
        </section>

        <BatchNormalizeStep
          v-model:enabled="form.normalize" v-model:mode="form.normalizeMode" v-model:peak-db="form.peakDb"
          v-model:target="form.targetLufs" v-model:ceiling="form.ceilingDb"
        />

        <!-- Fades -->
        <section class="p-3 flex flex-col gap-3">
          <label class="flex items-center justify-between font-medium">
            {{ t('batch.fades.title') }}
            <Switch v-model="form.fades" />
          </label>
          <div v-if="form.fades" class="grid grid-cols-3 gap-3">
            <label class="flex flex-col gap-1 text-muted-foreground">
              {{ t('batch.fades.in') }}
              <div class="flex items-center gap-1.5"><Input v-model.number="form.fadeInMs" type="number" min="0" class="h-8" /> {{ t('batch.msUnit') }}</div>
            </label>
            <label class="flex flex-col gap-1 text-muted-foreground">
              {{ t('batch.fades.out') }}
              <div class="flex items-center gap-1.5"><Input v-model.number="form.fadeOutMs" type="number" min="0" class="h-8" /> {{ t('batch.msUnit') }}</div>
            </label>
            <div class="flex flex-col gap-1 text-muted-foreground">
              {{ t('batch.fades.curve') }}
              <Select v-model="form.curve">
                <SelectTrigger size="sm" :aria-label="t('batch.fades.curve')">
                  <SelectValue>{{ t(`editor.curves.${form.curve}`) }}</SelectValue>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="c in curves" :key="c" :value="c">
                    {{ t(`editor.curves.${c}`) }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </section>

        <!-- Format -->
        <section class="p-3 flex flex-col gap-3">
          <label class="flex items-center justify-between font-medium">
            {{ t('batch.format.title') }}
            <Switch v-model="form.format" />
          </label>
          <div v-if="form.format" class="grid grid-cols-3 gap-3 text-muted-foreground">
            <div class="flex flex-col gap-1">
              {{ t('batch.format.channels') }}
              <Select v-model="form.channels">
                <SelectTrigger size="sm" :aria-label="t('batch.format.channels')">
                  <SelectValue>{{ form.channels === KEEP ? t('batch.format.keep') : form.channels === '1' ? t('batch.format.mono') : t('batch.format.stereo') }}</SelectValue>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem :value="KEEP">
                    {{ t('batch.format.keep') }}
                  </SelectItem>
                  <SelectItem value="1">
                    {{ t('batch.format.mono') }}
                  </SelectItem>
                  <SelectItem value="2">
                    {{ t('batch.format.stereo') }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div class="flex flex-col gap-1">
              {{ t('batch.format.sampleRate') }}
              <Select v-model="form.sampleRate">
                <SelectTrigger size="sm" :aria-label="t('batch.format.sampleRate')">
                  <SelectValue>{{ form.sampleRate === KEEP ? t('batch.format.keep') : `${formatNumber(Number(form.sampleRate))} Hz` }}</SelectValue>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem :value="KEEP">
                    {{ t('batch.format.keep') }}
                  </SelectItem>
                  <SelectItem v-for="r in rates" :key="r" :value="r">
                    {{ formatNumber(Number(r)) }} Hz
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div class="flex flex-col gap-1">
              {{ t('batch.format.bitDepth') }}
              <Select v-model="form.bitDepth">
                <SelectTrigger size="sm" :aria-label="t('batch.format.bitDepth')">
                  <SelectValue>{{ form.bitDepth === KEEP ? t('batch.format.keep') : form.bitDepth === '32' ? t('batch.format.float') : t('batch.format.bits', { n: form.bitDepth }) }}</SelectValue>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem :value="KEEP">
                    {{ t('batch.format.keep') }}
                  </SelectItem>
                  <SelectItem value="16">
                    {{ t('batch.format.bits', { n: 16 }) }}
                  </SelectItem>
                  <SelectItem value="24">
                    {{ t('batch.format.bits', { n: 24 }) }}
                  </SelectItem>
                  <SelectItem value="32">
                    {{ t('batch.format.float') }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </section>

        <!-- Rename -->
        <section class="p-3 flex flex-col gap-3">
          <label class="flex items-center justify-between font-medium" :class="inPlace && 'opacity-50'">
            {{ t('batch.rename.title') }}
            <Switch v-model="form.rename" :disabled="inPlace" />
          </label>
          <p v-if="inPlace" class="text-xs text-muted-foreground -mt-2">
            {{ t('batch.rename.inPlace') }}
          </p>
          <div v-else-if="form.rename" class="flex flex-col gap-2">
            <Input ref="templateInput" v-model="form.template" class="h-8 font-mono text-xs" :aria-label="t('batch.rename.template')" />
            <div class="flex flex-wrap items-center gap-1 text-xs text-muted-foreground">
              {{ t('batch.rename.tokens') }}
              <button
                v-for="tok in tokens" :key="tok" type="button"
                class="rounded border border-border px-1.5 py-0.5 font-mono hover:bg-accent hover:text-accent-foreground"
                @click="insertToken(tok)"
              >
                {{ tokenLabel(tok) }}
              </button>
            </div>
            <p v-if="examples.length" class="text-xs text-muted-foreground truncate">
              {{ t('batch.rename.preview') }}: <span class="text-foreground">{{ examples.map(n => `${n}.wav`).join(', ') }}</span>
            </p>
          </div>
        </section>

        <!-- Destination -->
        <section class="p-3 flex flex-col gap-2">
          <span class="font-medium">{{ t('batch.destination.title') }}</span>
          <label class="flex items-center gap-2">
            <input type="radio" :checked="!inPlace" class="accent-primary" @change="destination = { mode: 'folder', path: folder }">
            {{ t('batch.destination.folder') }}
          </label>
          <div v-if="!inPlace" class="ml-6 flex items-center gap-2 min-w-0">
            <span class="min-w-0 flex-1 truncate text-xs" :class="folder ? 'text-foreground' : 'text-muted-foreground'" :title="folder">
              {{ folder ? shortPath(folder) : t('batch.destination.none') }}
            </span>
            <Button variant="outline" size="sm" @click="chooseFolder()">
              {{ t('batch.destination.choose') }}
            </Button>
          </div>
          <label class="flex items-center gap-2">
            <input type="radio" :checked="inPlace" class="accent-primary" @change="destination = { mode: 'inPlace' }">
            {{ t('batch.destination.inPlace') }}
          </label>
          <p v-if="inPlace" class="ml-6 text-xs text-muted-foreground">
            {{ t('batch.destination.inPlaceHint') }}
          </p>
        </section>
      </fieldset>

      <!-- Progress and result -->
      <div v-if="running && progress" class="flex flex-col gap-1.5 text-sm">
        <div class="h-1.5 rounded-full bg-muted overflow-hidden">
          <div class="h-full bg-primary transition-[width]" :style="{ width: `${(progress.done / Math.max(1, progress.total)) * 100}%` }" />
        </div>
        <div class="flex justify-between gap-2 text-xs text-muted-foreground">
          <span class="truncate">{{ progress.name }}</span>
          <span class="tabular shrink-0">{{ t('batch.running', { done: formatNumber(progress.done), total: formatNumber(progress.total) }) }}</span>
        </div>
      </div>
      <div v-else-if="result" class="flex flex-col gap-1 text-sm">
        <div class="flex items-center gap-2">
          <span>{{ t('batch.done.written', { n: formatNumber(result.written) }, result.written) }}</span>
          <span v-if="result.failed.length" class="text-destructive">· {{ t('batch.done.failed', { n: formatNumber(result.failed.length) }, result.failed.length) }}</span>
          <span v-if="result.cancelled" class="text-muted-foreground">· {{ t('batch.done.cancelled') }}</span>
          <Button v-if="result.dir && result.written" variant="link" size="sm" class="ml-auto h-auto p-0" @click="revealItemInDir(result.dir)">
            {{ t('batch.done.reveal') }}
          </Button>
        </div>
        <BatchLoudnessReport :reports="result.loudness ?? []" />
        <ul v-if="result.failed.length" class="max-h-24 overflow-y-auto text-xs text-muted-foreground">
          <li v-for="f in result.failed" :key="f.name" class="truncate" :title="f.error">
            {{ f.name }}: {{ f.error }}
          </li>
        </ul>
      </div>
      <p v-else-if="!anyStep" class="text-xs text-muted-foreground">
        {{ t('batch.nothing') }}
      </p>

      <DialogFooter>
        <Button v-if="running" variant="outline" @click="batchApi.cancel()">
          {{ t('batch.cancel') }}
        </Button>
        <template v-else>
          <Button variant="outline" @click="open = false">
            {{ result ? t('batch.close') : t('common.actions.cancel') }}
          </Button>
          <Button :disabled="!canRun" @click="run()">
            {{ t('batch.run') }}
          </Button>
        </template>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
