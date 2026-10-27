<script setup lang="ts">
import { AudioLines, Gauge, MoveVertical } from '@lucide/vue'
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Field, FieldDescription, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { Spinner } from '@/components/ui/spinner'
import { formatNumber } from '@/format'
import { useEditorStore } from '@/stores/editor'

// This component owns processing controls; the store owns the operation and selection.
const { t } = useI18n()
const props = defineProps<{ tool?: 'stretch' | 'lufs' | 'dc' }>()
const editor = useEditorStore()
const mode = ref<'stretch' | 'lufs' | null>(null)
const semitones = ref<string | number>(0)
const percent = ref<string | number>(100)
const target = ref<string | number>(-14)
const ceiling = ref<string | number>(-0.3)
let trigger: HTMLElement | null = null
const range = computed(() => editor.selection ?? { start: 0, end: editor.state?.frames ?? 0 })
const seconds = computed(() => (range.value.end - range.value.start) / (editor.state?.sampleRate ?? 44100))
const title = computed(() => t(mode.value === 'lufs' ? 'editor.processing.lufs' : 'editor.processing.stretch'))
const validNumber = (v: string | number, min: number, max: number) => v !== '' && Number.isFinite(Number(v)) && Number(v) >= min && Number(v) <= max
const valid = computed(() => mode.value === 'stretch'
  ? validNumber(semitones.value, -24, 24) && validNumber(percent.value, 25, 400)
    && (Number(semitones.value) !== 0 || Number(percent.value) !== 100) && range.value.end - range.value.start >= 64
  : validNumber(target.value, -36, -5) && validNumber(ceiling.value, -12, 0) && seconds.value >= 0.4)

function show(value: 'stretch' | 'lufs', event: MouseEvent) {
  trigger = event.currentTarget as HTMLElement
  mode.value = value
}
function close(open: boolean) {
  if (!open && !editor.processing)
    mode.value = null
}
function restoreFocus(event: Event) {
  event.preventDefault()
  trigger?.focus()
}
async function run() {
  if (!valid.value || editor.processing)
    return
  const success = await editor.processAudio(mode.value === 'stretch'
    ? { mode: 'stretch', semitones: Number(semitones.value), ratio: Number(percent.value) / 100 }
    : { mode: 'lufs', target: Number(target.value), ceiling: Number(ceiling.value) })
  if (success)
    mode.value = null
}
</script>

<template>
  <Hint v-if="!props.tool || props.tool === 'stretch'" :text="t('editor.processing.stretch')">
    <Button variant="ghost" size="icon-sm" :disabled="!editor.state?.frames || editor.processing" :aria-label="t('editor.processing.stretch')" @click="show('stretch', $event)">
      <AudioLines />
    </Button>
  </Hint>
  <Hint v-if="!props.tool || props.tool === 'lufs'" :text="t('editor.processing.lufs')">
    <Button variant="ghost" size="icon-sm" :disabled="!editor.state?.frames || editor.processing" :aria-label="t('editor.processing.lufs')" @click="show('lufs', $event)">
      <Gauge />
    </Button>
  </Hint>
  <Hint v-if="!props.tool || props.tool === 'dc'" :text="t('editor.processing.dcOffsetHint')">
    <Button variant="ghost" size="icon-sm" :disabled="!editor.state?.frames || editor.processing" :aria-label="t('editor.processing.dcOffset')" @click="editor.removeDcOffset()">
      <MoveVertical />
    </Button>
  </Hint>
  <Dialog :open="mode !== null" @update:open="close">
    <DialogContent
      class="sm:max-w-sm" :show-close-button="!editor.processing"
      @escape-key-down="editor.processing && $event.preventDefault()"
      @interact-outside="editor.processing && $event.preventDefault()"
      @close-auto-focus="restoreFocus"
    >
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription>
          {{ t(editor.selection ? 'editor.slice.ofSelection' : 'editor.slice.ofFile') }} · {{ t('editor.processing.duration', { seconds: formatNumber(seconds, { fixed: 3 }) }) }}
        </DialogDescription>
      </DialogHeader>
      <form class="grid gap-4" :aria-busy="editor.processing" @submit.prevent="run">
        <fieldset :disabled="editor.processing" class="min-w-0">
          <FieldGroup>
            <template v-if="mode === 'stretch'">
              <Field>
                <FieldLabel for="process-pitch">{{ t('editor.processing.pitch') }}</FieldLabel>
                <Input id="process-pitch" v-model="semitones" type="number" min="-24" max="24" step="0.1" required />
                <FieldDescription>{{ t('editor.processing.pitchHelp') }}</FieldDescription>
              </Field>
              <Field>
                <FieldLabel for="process-duration">{{ t('editor.processing.percent') }}</FieldLabel>
                <Input id="process-duration" v-model="percent" type="number" min="25" max="400" step="0.1" required />
                <FieldDescription>{{ t('editor.processing.resultDuration', { seconds: formatNumber(seconds * Number(percent) / 100, { fixed: 3 }) }) }}</FieldDescription>
              </Field>
            </template>
            <template v-else>
              <Field>
                <FieldLabel for="process-lufs">{{ t('editor.processing.target') }}</FieldLabel>
                <Input id="process-lufs" v-model="target" type="number" min="-36" max="-5" step="0.1" required />
              </Field>
              <Field>
                <FieldLabel for="process-ceiling">{{ t('editor.processing.ceiling') }}</FieldLabel>
                <Input id="process-ceiling" v-model="ceiling" type="number" min="-12" max="0" step="0.1" required />
                <FieldDescription>{{ t('editor.processing.lufsHelp') }}</FieldDescription>
              </Field>
            </template>
          </FieldGroup>
        </fieldset>
        <DialogFooter>
          <Button type="button" variant="outline" :disabled="editor.processing" @click="close(false)">{{ t('common.actions.cancel') }}</Button>
          <Button type="submit" :disabled="!valid || editor.processing">
            <Spinner v-if="editor.processing" />
            {{ t(editor.processing ? 'editor.processing.running' : 'editor.processing.apply') }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
