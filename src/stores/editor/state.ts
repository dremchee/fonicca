import { computed, ref } from 'vue'
import { regionsOf } from '@/editor-regions'
import type { EditorWindowView } from '@/editor-window-state'
import type { ClipInfo, Curve, EditedFile, EditorState, Span } from './types'

/** One set of refs per Pinia editor instance; action modules share these refs. */
export function createEditorState() {
  const sample = ref<EditedFile | null>(null)
  const state = ref<EditorState | null>(null)
  const open = computed(() => sample.value !== null)
  const loading = ref(false)
  const processing = ref(false)
  const saving = ref(false)
  const sessionVersion = ref(0)
  const expanded = ref(false)
  const windowView = ref<EditorWindowView | null>(null)
  const unsavedPrompt = ref(false)
  /** Bumped after every change of the audio, so the view re-reads peaks. */
  const revision = ref(0)
  // Selection and cursor, in frames.
  const cursor = ref(0)
  const selection = ref<Span | null>(null)
  const looping = ref(false)
  const playing = ref(false)
  const playhead = ref<number | null>(null)
  const curve = ref<Curve>('smooth')
  /** The clipboard shared by all editor windows; re-read when this one gets focus. */
  const clipboard = ref<ClipInfo | null>(null)
  const error = ref<string | null>(null)
  const canOverwrite = computed(() => !!state.value?.canReplaceOriginal)
  const regions = computed(() => regionsOf(state.value?.markers ?? [], state.value?.frames ?? 0))

  return {
    sample, state, open, loading, processing, saving, sessionVersion,
    expanded, windowView, unsavedPrompt, revision, cursor, selection,
    looping, playing, playhead, curve, clipboard, error, canOverwrite, regions,
  }
}

export type EditorStoreState = ReturnType<typeof createEditorState>
