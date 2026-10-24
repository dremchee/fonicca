import type { Sample } from '@/api'
import type { ProjectClip, ProjectDocument, ProjectTrack } from '@/project'
import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { useLocalStorage } from '@vueuse/core'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { computed, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { confirm } from '@/composables/confirm'
import { t } from '@/i18n'
import { copyProject, emptyProject, moveProjectClip, parseProject, projectDuration } from '@/project'
import { usePlayerStore } from '@/stores/player'

export const useProjectStore = defineStore('project', () => {
  const draft = useLocalStorage<ProjectDocument>('project.draft.v1', emptyProject(t('project.untitled')))
  const document = ref(parseProject(draft.value) ?? emptyProject(t('project.untitled')))
  const path = ref('')
  const selectedId = ref<string | null>(null)
  const cursor = ref(0)
  const playing = ref(false)
  const busy = ref(false)
  const error = ref('')
  const past = ref<ProjectDocument[]>([])
  const future = ref<ProjectDocument[]>([])
  const duration = computed(() => projectDuration(document.value))
  const selected = computed(() => document.value.tracks.flatMap(t => t.clips).find(c => c.id === selectedId.value))
  const saved = ref(document.value.tracks.length ? '' : JSON.stringify(document.value))
  const dirty = computed(() => JSON.stringify(document.value) !== saved.value)
  let generation = 0
  let timer: ReturnType<typeof setTimeout> | undefined
  watch(document, p => draft.value = copyProject(p), { deep: true })

  function stop() {
    generation++
    playing.value = false
    busy.value = false
    clearTimeout(timer)
    void invoke('project_stop').catch(e => error.value = String(e))
  }
  function replace(next: ProjectDocument, remember = true) {
    if (!parseProject(next)) { error.value = t('project.invalid'); return false }
    stop()
    if (remember) { past.value = [...past.value.slice(-99), copyProject(document.value)]; future.value = [] }
    document.value = copyProject(next)
    error.value = ''
    return true
  }
  function edit(change: (p: ProjectDocument) => void) {
    const next = copyProject(document.value); change(next); return replace(next)
  }
  function undo() {
    const next = past.value.pop(); if (!next) return
    future.value.push(copyProject(document.value)); replace(next, false)
  }
  function redo() {
    const next = future.value.pop(); if (!next) return
    past.value.push(copyProject(document.value)); replace(next, false)
  }
  function track(p: ProjectDocument, name: string): ProjectTrack {
    const track: ProjectTrack = { id: crypto.randomUUID(), name, gainDb: 0, pan: 0, mute: false, solo: false, clips: [] }
    p.tracks.push(track); return track
  }
  function addTrack() { edit(p => track(p, `${t('project.track')} ${p.tracks.length + 1}`)) }
  async function addSources(command: 'project_samples' | 'project_sources', args: Record<string, unknown>, trackId?: string, start = cursor.value) {
    error.value = ''
    try {
      const samples = await invoke<Pick<Sample, 'id' | 'path' | 'name' | 'duration'>[]>(command, args)
      const next = copyProject(document.value)
      for (const s of samples) {
        if (!s.duration || s.duration <= 0) throw new Error(t('project.unknownDuration', { name: s.name }))
        const target = next.tracks.find(t => t.id === trackId) ?? track(next, s.name)
        const length = Math.min(s.duration, 600 - start)
        if (length <= 0) throw new Error(t('project.invalid'))
        target.clips.push({ id: crypto.randomUUID(), path: s.path, name: s.name, sourceDuration: Math.min(600, s.duration), start, offset: 0, duration: length, repeats: 1, fadeIn: 0, fadeOut: 0 })
      }
      replace(next)
    }
    catch (e) { error.value = String(e); toast.error(error.value) }
  }
  function addSamples(ids: number[], trackId?: string, start = cursor.value) { return addSources('project_samples', { ids }, trackId, start) }
  function addPaths(paths: string[], trackId?: string, start = cursor.value) { return addSources('project_sources', { paths }, trackId, start) }
  function updateTrack(id: string, values: Partial<ProjectTrack>) { edit(p => { const track = p.tracks.find(t => t.id === id); if (track) Object.assign(track, values) }) }
  function updateClip(id: string, values: Partial<ProjectClip>) {
    edit(p => {
      const clip = p.tracks.flatMap(t => t.clips).find(c => c.id === id)
      if (!clip) return
      Object.assign(clip, values)
      clip.duration = Math.min(clip.duration, clip.sourceDuration - clip.offset, (600 - clip.start) / clip.repeats)
      clip.fadeIn = Math.min(clip.fadeIn, clip.duration); clip.fadeOut = Math.min(clip.fadeOut, clip.duration)
    })
  }
  function moveClip(id: string, trackId: string, time: number) { replace(moveProjectClip(document.value, id, trackId, time)) }
  function deleteClip() { if (selectedId.value) edit(p => { for (const t of p.tracks) t.clips = t.clips.filter(c => c.id !== selectedId.value) }); selectedId.value = null }
  function duplicateClip() {
    const c = selected.value; if (!c) return
    edit(p => { const target = p.tracks.find(t => t.clips.some(x => x.id === c.id)); target?.clips.push({ ...c, id: crypto.randomUUID(), start: c.start + c.duration * c.repeats }) })
  }
  async function play() {
    if (playing.value || busy.value) { stop(); return }
    await usePlayerStore().stop()
    if (cursor.value >= duration.value) cursor.value = 0
    const version = ++generation
    busy.value = true; error.value = ''
    try {
      await invoke('project_play', { project: copyProject(document.value), start: cursor.value })
      if (version !== generation) return
      playing.value = true
      const tick = async () => {
        try {
          const pos = await invoke<number | null>('project_position')
          if (version !== generation) return
          if (pos === null) { playing.value = false; cursor.value = 0; return }
          cursor.value = pos
          timer = setTimeout(tick, 30)
        }
        catch (e) { if (version === generation) { error.value = String(e); stop() } }
      }
      void tick()
    }
    catch (e) { if (version === generation) error.value = String(e) }
    finally { if (version === generation) busy.value = false }
  }
  function seek(time: number) { stop(); cursor.value = Math.max(0, Math.min(duration.value, time)) }
  async function allowReplace() {
    return !dirty.value || await confirm({ title: t('project.replaceTitle'), description: t('project.replaceHint'), action: t('project.continue') })
  }
  async function newProject() {
    if (!await allowReplace()) return
    replace(emptyProject(t('project.untitled'))); past.value = []; future.value = []; path.value = ''; saved.value = JSON.stringify(document.value); cursor.value = 0; selectedId.value = null
  }
  async function saveProject() {
    const target = await save({ defaultPath: path.value || `${document.value.name}.fonicca.json`, filters: [{ name: 'Fonicca project', extensions: ['json'] }] })
    if (!target) return
    const snapshot = copyProject(document.value)
    try { await invoke('project_save', { project: snapshot, path: target }); path.value = target; saved.value = JSON.stringify(snapshot); toast.success(t('project.saved')) }
    catch (e) { error.value = String(e) }
  }
  async function openProject() {
    if (!await allowReplace()) return
    const target = await open({ filters: [{ name: 'Fonicca project', extensions: ['json'] }] })
    if (typeof target !== 'string') return
    try {
      const next = parseProject(await invoke('project_load', { path: target }))
      if (!next) throw new Error(t('project.invalid'))
      replace(next); past.value = []; future.value = []; path.value = target; saved.value = JSON.stringify(next); cursor.value = 0; selectedId.value = null
    }
    catch (e) { error.value = String(e) }
  }
  const exporting = ref(false)
  async function exportMix() {
    const target = await save({ defaultPath: `${document.value.name}.wav`, filters: [{ name: 'WAV · 48 kHz · 32-bit float', extensions: ['wav'] }] })
    if (!target) return
    exporting.value = true; error.value = ''
    try {
      const report = await invoke<{ peakDb: number }>('project_export', { project: copyProject(document.value), path: target })
      toast.success(report.peakDb > 0 ? t('project.exportClipping', { peak: report.peakDb.toFixed(1) }) : t('project.exported'))
    }
    catch (e) { error.value = String(e) }
    finally { exporting.value = false }
  }
  return { document, path, selectedId, selected, cursor, playing, busy, exporting, error, past, future, duration, dirty,
    edit, undo, redo, addTrack, addSamples, addPaths, updateTrack, updateClip, moveClip, deleteClip, duplicateClip, seek, play, stop, newProject, saveProject, openProject, exportMix }
})

if (import.meta.hot)
  import.meta.hot.accept(acceptHMRUpdate(useProjectStore, import.meta.hot))
