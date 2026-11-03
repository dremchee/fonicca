import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { save as chooseSavePath } from '@tauri-apps/plugin-dialog'
import { toast } from 'vue-sonner'
import { confirm } from '@/composables/confirm'
import { formatNumber } from '@/format'
import { t } from '@/i18n'
import { audioExportFormat, audioExportPath } from '@/audio-export'
import { chooseAudioExport } from '@/composables/audioExport'
import { editorApi } from './api'
import type { EditorStoreState } from './state'
import type { SaveMode } from './types'

interface Options {
  context: Pick<EditorStoreState, 'sample' | 'state' | 'selection' | 'processing' | 'saving' | 'canOverwrite' | 'regions'>
  stopLibraryPlayback: () => Promise<void>
  stopPlayback: () => Promise<void>
}

/** Saving and region export share the editor refs, without owning another store. */
export function createEditorSaving({ context, stopLibraryPlayback, stopPlayback }: Options) {
  const { sample, state, selection, processing, saving, canOverwrite, regions } = context

  async function save(mode: SaveMode = 'save') {
    if (!sample.value || processing.value || saving.value)
      return false
    if (mode === 'save' && !state.value?.dirty)
      return true
    const selected = selection.value ? { ...selection.value } : undefined
    if (mode === 'selection' && !selected)
      return false
    saving.value = true
    try {
      let destination: string | undefined
      let exportRate: number | null | undefined
      const actualMode = mode === 'save' && !state.value?.hasSavedTarget ? 'saveAs' : mode
      if (actualMode === 'saveAs' || actualMode === 'selection') {
        const options = await chooseAudioExport({ format: audioExportFormat(sample.value.path), sampleRate: state.value?.exportSampleRate ?? null }, state.value?.sampleRate ?? 44100)
        if (!options) return false
        const format = options.format
        exportRate = options.sampleRate
        const suffix = `${mode === 'selection' ? ' — loop' : ' — edited'}.${format}`
        const picked = await chooseSavePath({
          title: t(mode === 'selection' ? 'editor.save.selection' : 'editor.save.as'),
          defaultPath: sample.value.path.replace(/\.[^/\\.]+$/, '') + suffix,
          filters: [{ name: format === 'ogg' ? 'OGG Vorbis' : format.toUpperCase(), extensions: [format] }],
        })
        if (!picked) return false
        destination = audioExportPath(picked, format)
      }
      if (mode === 'replaceOriginal') {
        if (!canOverwrite.value || !await confirm({ title: t('editor.save.replace'),
          description: t('editor.save.replaceConfirm', { path: state.value?.originalPath ?? '' }),
          action: t('editor.save.replace'), destructive: true })) return false
      }
      await stopLibraryPlayback()
      await stopPlayback()
      const r = await editorApi.save(actualMode, destination, mode === 'selection' ? selected : undefined, exportRate)
      if (mode !== 'selection') {
        state.value = r.state
        sample.value = { id: r.state.sampleId, name: r.name, ext: r.ext, path: r.path, bpm: r.bpm ?? sample.value.bpm }
      }
      toast.success(t(mode === 'selection' ? 'editor.save.selectionSaved' : 'editor.save.saved', { name: r.name }), {
        description: r.backup ? t('editor.save.backupKept') : undefined,
      })
      if (r.warning) toast.warning(t('editor.save.indexWarning'), { description: r.warning })
      return true
    }
    catch (e) {
      toast.error(String(e))
      return false
    }
    finally { saving.value = false }
  }

  /** Saves each region not skipped as a new sample, into a folder next to the file. */
  async function saveRegions() {
    if (processing.value || saving.value)
      return
    const spans = regions.value.filter(r => !r.skip && r.end > r.start).map(({ start, end }) => ({ start, end }))
    if (!spans.length)
      return
    try {
      const r = await editorApi.exportSlices(spans)
      toast.success(t('editor.regions.saved', { n: formatNumber(r.count) }, r.count), {
        description: r.inLibrary ? undefined : t('editor.regions.notInLibrary'),
        action: { label: t('editor.regions.reveal'), onClick: () => revealItemInDir(r.dir) },
      })
    }
    catch (e) {
      toast.error(String(e))
    }
  }

  return { save, saveRegions }
}
