<script setup lang="ts">
import type { Curve, Marker } from '@/stores/editor'
import type { EditorToolAction } from '@/editor-tools'
import { ArrowLeftRight, ArrowUpToLine, ClipboardPaste, Copy, Crop, Eraser,
  IterationCw, Maximize2, Minimize2, Music, Play, Redo2, Repeat, Scissors, Square,
  Undo2, Volume1, Volume2, VolumeX, X } from '@lucide/vue'
import { useI18n } from 'vue-i18n'
import { EDITOR_TOOLS, EDITOR_TOGGLE_CLASS } from '@/editor-tools'
import ToolbarOverflow from '@/components/ToolbarOverflow.vue'
import EditorProcessing from '@/components/EditorProcessing.vue'
import EditorSaveMenu from '@/components/EditorSaveMenu.vue'
import EditorSlicer from '@/components/EditorSlicer.vue'
import FadeIcon from '@/components/icons/FadeIcon.vue'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import { Toggle } from '@/components/ui/toggle'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'

const props = defineProps<{ embedded: boolean, expanded: boolean, canUndo: boolean, canRedo: boolean,
  hasSelection: boolean, hasLoop: boolean, hasClipboard: boolean, playing: boolean,
  canExportMidi: boolean, beatFrames: number, downbeat: number, zeroSnap: boolean }>()
const emit = defineEmits<{ action: [action: EditorToolAction], close: [], toggleExpand: [] }>()
const curve = defineModel<Curve>('curve', { required: true })
const looping = defineModel<boolean>('looping', { required: true })
const preview = defineModel<Marker[]>('preview', { required: true })
const { t } = useI18n()
const curves: { value: Curve, label: string }[] = [
  { value: 'smooth', label: 'editor.curves.smooth' },
  { value: 'linear', label: 'editor.curves.linear' },
  { value: 'exponential', label: 'editor.curves.exponential' },
]
</script>

<template>
    <div class="h-11 shrink-0 px-3 flex items-center gap-1 border-b border-border">
      <ToolbarOverflow :items="EDITOR_TOOLS">
        <template #default="{ item }">
          <Hint v-if="item === 'undo'" :text="t('editor.toolbar.undo')" shortcut="⌘Z">
              <Button variant="ghost" size="icon-sm" :disabled="!canUndo" :aria-label="t('editor.toolbar.undo')" @click="emit('action', 'undo')">
                <Undo2 />
              </Button>
          </Hint>
          <Hint v-if="item === 'redo'" :text="t('editor.toolbar.redo')" shortcut="⇧⌘Z">
              <Button variant="ghost" size="icon-sm" :disabled="!canRedo" :aria-label="t('editor.toolbar.redo')" @click="emit('action', 'redo')">
                <Redo2 />
              </Button>
          </Hint>
          <Hint v-if="item === 'play'" :text="playing ? t('editor.toolbar.stop') : hasSelection ? t('editor.toolbar.playSelection') : t('editor.toolbar.playFromCursor')" :shortcut="t('editor.toolbar.space')">
              <Button variant="ghost" size="icon-sm" :aria-label="t(playing ? 'editor.toolbar.stop' : 'common.actions.play')" @click="emit('action', 'play')">
                <component :is="playing ? Square : Play" />
              </Button>
          </Hint>
          <Hint v-if="item === 'loop'" :text="t('editor.toolbar.loopHint')" shortcut="L">
              <Toggle v-model="looping" size="sm" :class="EDITOR_TOGGLE_CLASS" :aria-label="t('editor.toolbar.loop')">
                <Repeat />
              </Toggle>
          </Hint>
          <Hint v-if="item === 'loopPoints'" :text="hasSelection ? t('editor.toolbar.setLoopHint') : t('editor.toolbar.clearLoopHint')" shortcut="⇧L">
              <Button
                variant="ghost" size="icon-sm"
                :class="hasLoop && 'text-primary hover:text-primary'"
                :disabled="!hasSelection && !hasLoop"
                :aria-label="t('editor.toolbar.loopPoints')"
                @click="emit('action', 'loopPoints')"
              >
                <IterationCw />
              </Button>
          </Hint>
          <Hint v-if="item === 'crop'" :text="t('editor.toolbar.cropHint')">
              <Button variant="ghost" size="icon-sm" :disabled="!hasSelection" :aria-label="t('editor.toolbar.crop')" @click="emit('action', 'crop')">
                <Crop />
              </Button>
          </Hint>
          <Hint v-if="item === 'delete'" :text="t('editor.toolbar.deleteHint')" shortcut="⌫">
              <Button variant="ghost" size="icon-sm" :disabled="!hasSelection" :aria-label="t('editor.toolbar.delete')" @click="emit('action', 'delete')">
                <Eraser />
              </Button>
          </Hint>
          <Hint v-if="item === 'cut'" :text="t('editor.toolbar.cutHint')" shortcut="⌘X">
              <Button variant="ghost" size="icon-sm" :disabled="!hasSelection" :aria-label="t('editor.toolbar.cut')" @click="emit('action', 'cut')">
                <Scissors />
              </Button>
          </Hint>
          <Hint v-if="item === 'copy'" :text="t('editor.toolbar.copyHint')" shortcut="⌘C">
              <Button variant="ghost" size="icon-sm" :disabled="!hasSelection" :aria-label="t('editor.toolbar.copy')" @click="emit('action', 'copy')">
                <Copy />
              </Button>
          </Hint>
          <Hint v-if="item === 'paste'" :text="hasSelection ? t('editor.toolbar.pasteOverHint') : t('editor.toolbar.pasteHint')" shortcut="⌘V">
              <Button variant="ghost" size="icon-sm" :disabled="!hasClipboard" :aria-label="t('editor.toolbar.paste')" @click="emit('action', 'paste')">
                <ClipboardPaste />
              </Button>
          </Hint>
          <Hint v-if="item === 'silence'" :text="t('editor.toolbar.silenceHint')">
              <Button variant="ghost" size="icon-sm" :disabled="!hasSelection" @click="emit('action', 'silence')">
                <VolumeX />
              </Button>
          </Hint>
          <Hint v-if="item === 'fadeIn'" :text="t('editor.toolbar.fadeInHint')">
              <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.fadeIn')" @click="emit('action', 'fadeIn')">
                <FadeIcon direction="in" :curve="curve" />
              </Button>
          </Hint>
          <Hint v-if="item === 'fadeOut'" :text="t('editor.toolbar.fadeOutHint')">
              <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.fadeOut')" @click="emit('action', 'fadeOut')">
                <FadeIcon direction="out" :curve="curve" />
              </Button>
          </Hint>
          <Hint v-if="item === 'curve'" :text="t('editor.toolbar.fadeCurve')">
              <span class="inline-flex">
                <Select v-model="curve">
                  <SelectTrigger size="sm" class="gap-1 px-2 border-transparent bg-transparent shadow-none dark:bg-transparent hover:bg-accent" :aria-label="t('editor.toolbar.fadeCurve')">
                    <SelectValue>
                      <FadeIcon :curve="curve" class="size-4 text-foreground" />
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem v-for="c in curves" :key="c.value" :value="c.value">
                      <FadeIcon :curve="c.value" class="size-4" /> {{ t(c.label) }}
                    </SelectItem>
                  </SelectContent>
                </Select>
              </span>
          </Hint>
          <Hint v-if="item === 'normalize'" :text="t('editor.toolbar.normalizeHint')">
              <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.normalize')" @click="emit('action', 'normalize')">
                <ArrowUpToLine />
              </Button>
          </Hint>
          <Hint v-if="item === 'gainDown'" :text="t('editor.toolbar.gainDownHint')">
              <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.gainDownHint')" @click="emit('action', 'gainDown')">
                <Volume1 />
              </Button>
          </Hint>
          <Hint v-if="item === 'gainUp'" :text="t('editor.toolbar.gainUpHint')">
              <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.gainUpHint')" @click="emit('action', 'gainUp')">
                <Volume2 />
              </Button>
          </Hint>
          <Hint v-if="item === 'reverse'" :text="t('editor.toolbar.reverseHint')">
              <Button variant="ghost" size="icon-sm" @click="emit('action', 'reverse')">
                <ArrowLeftRight />
              </Button>
          </Hint>
          <EditorProcessing v-if="item === 'stretch'" tool="stretch" />
          <EditorProcessing v-if="item === 'lufs'" tool="lufs" />
          <EditorProcessing v-if="item === 'dc'" tool="dc" />
          <Hint v-if="item === 'midi'" :text="t('basicPitch.action')">
              <Button variant="ghost" size="sm" class="px-2" :disabled="!canExportMidi" :aria-label="t('basicPitch.action')" @click="emit('action', 'midi')">
                <Music /> MIDI
              </Button>
          </Hint>
          <EditorSlicer v-if="item === 'slice'" v-model:preview="preview" :beat-frames="beatFrames" :downbeat="downbeat" :zero="zeroSnap" />
        </template>
      </ToolbarOverflow>
      <div class="shrink-0 flex items-center gap-1 pl-2 border-l border-border">
        <EditorSaveMenu />
        <Button v-if="props.embedded" variant="ghost" size="icon-sm" :aria-label="t(props.expanded ? 'editor.panel.collapse' : 'editor.panel.expand')" @click="emit('toggleExpand')">
          <Minimize2 v-if="props.expanded" /><Maximize2 v-else />
        </Button>
        <Button v-if="props.embedded" variant="ghost" size="icon-sm" :aria-label="t('common.actions.close')" @click="emit('close')"><X /></Button>
      </div>
    </div>

</template>
