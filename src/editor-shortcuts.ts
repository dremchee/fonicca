import type { Region } from './editor-regions.ts'

interface KeyboardEditor {
  selection: { start: number, end: number } | null
  cursor: number
  looping: boolean
  togglePlay: () => unknown
  undo: () => unknown
  redo: () => unknown
  save: (mode: 'save' | 'saveAs') => unknown
  remove: () => unknown
  toggleLoopPoints: () => unknown
}
interface Options {
  editor: KeyboardEditor
  frames: () => number
  regions: () => Region[]
  playRegion: (region: { start: number, end: number }) => unknown
  addMarkers: (frames: number[]) => unknown
  toggleSnap: () => void
  zoom: (scale: number, anchor?: number) => void
  fit: () => void
}

/** Keyboard mapping only; audio and selection state belong to the store. */
export function createEditorShortcuts(options: Options) {
  const { editor, frames, regions, playRegion, addMarkers, toggleSnap, zoom, fit } = options
  return function onKeyDown(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey
    const key = e.key.toLowerCase()
    const handled = () => e.preventDefault()
    if (e.key === ' ') {
      handled()
      editor.togglePlay()
    }
    // 1…9 play regions 1–9 like pads, ⇧1…⇧9 regions 10–18 (by key position, so any layout).
    else if (!mod && !e.altKey && /^Digit[1-9]$/.test(e.code)) {
      const n = Number(e.code.slice(5)) + (e.shiftKey ? 9 : 0)
      const region = regions()[n - 1]
      if (region) {
        handled()
        playRegion(region)
      }
    }
    else if (mod && key === 'z') {
      handled()
      e.shiftKey ? editor.redo() : editor.undo()
    }
    else if (mod && key === 's') {
      handled()
      editor.save(e.shiftKey ? 'saveAs' : 'save')
    }
    else if (mod && key === 'a') {
      handled()
      editor.selection = { start: 0, end: frames() }
    }
    else if (e.key === 'Delete' || e.key === 'Backspace') {
      handled()
      editor.remove()
    }
    else if (e.key === 'Escape') {
      editor.selection = null
    }
    else if (!mod && e.shiftKey && key === 'l') {
      editor.toggleLoopPoints()
    }
    else if (!mod && key === 'l') {
      editor.looping = !editor.looping
    }
    else if (!mod && (key === 'm' || key === 'ь')) {
      const sel = editor.selection
      addMarkers(sel ? [sel.start, sel.end] : [editor.cursor])
    }
    else if (!mod && key === 's') {
      toggleSnap()
    }
    else if (!mod && (e.key === '=' || e.key === '+')) {
      zoom(0.5, editor.selection ? (editor.selection.start + editor.selection.end) / 2 : editor.cursor)
    }
    else if (!mod && e.key === '-') {
      zoom(2)
    }
    else if (!mod && e.key === '0') {
      fit()
    }
  }

}
