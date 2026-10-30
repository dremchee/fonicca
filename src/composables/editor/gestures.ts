import type { Ref } from 'vue'
import type { Marker, Region } from '../../editor-regions.ts'
import type { Span } from '../../stores/editor.ts'
import { computed, onScopeDispose, ref, shallowRef, watch } from 'vue'

export interface GestureEditor {
  state: { loop: Span | null, markers: Marker[] } | null
  regions: Region[]
  selection: Span | null
  cursor: number
  playing: boolean
  loading?: boolean
  processing?: boolean
  saving?: boolean
  zeroSnap: (frames: number[], radius: number) => Promise<number[]>
  setMarkers: (markers: Marker[]) => unknown
  setLoopPoints: (range: Span) => Promise<boolean>
  stopPlayback: () => unknown
  play: () => unknown
  updatePlaybackRange: () => unknown
}
interface Options {
  editor: GestureEditor
  canvas: Ref<HTMLCanvasElement | null>
  frames: Ref<number>
  rate: Ref<number>
  viewStart: Ref<number>
  viewEnd: Ref<number>
  identity: Ref<string>
  zeroSnap: Ref<boolean>
  ruler: number
  x: (frame: number) => number
  snapped: (frame: number, event: MouseEvent) => number
  badgeAt: (event: MouseEvent) => Span | undefined
}

/** Owns transient selection/marker gestures; persisted audio state stays in the store. */
export function useEditorGestures(options: Options) {
  const { editor, canvas, frames, rate, viewStart, viewEnd, zeroSnap, x, snapped, badgeAt } = options
  const RULER = options.ruler
  const draggedMarkers = shallowRef<Marker[] | null>(null)
  const draggedLoop = shallowRef<Span | null>(null)
  let mouseGesture = 0
  let releaseListeners = () => {}
  function cancelGesture() {
    mouseGesture++
    releaseListeners()
    draggedMarkers.value = null
    draggedLoop.value = null
  }
  function listen(move: (event: MouseEvent) => void, up: (event: MouseEvent) => void) {
    window.addEventListener('mousemove', move)
    window.addEventListener('mouseup', up)
    releaseListeners = () => {
      window.removeEventListener('mousemove', move)
      window.removeEventListener('mouseup', up)
      releaseListeners = () => {}
    }
  }
  function playRegion(badge: { start: number, end: number }) {
    editor.stopPlayback()
    editor.selection = { start: badge.start, end: badge.end }
    editor.cursor = badge.start
    editor.play()
  }

  // --- Mouse ------------------------------------------------------------------

  function frameAt(clientX: number) {
    const rect = canvas.value!.getBoundingClientRect()
    const f = viewStart.value + ((clientX - rect.left) / rect.width) * (viewEnd.value - viewStart.value)
    return Math.round(Math.min(frames.value, Math.max(0, f)))
  }

  // Edges move to the nearest zero crossing, so cuts and loops don't click. Within
  // 10 ms (a 50 Hz period); ⌥ inverts it, like grid snapping.
  async function toZeroCrossings(e: MouseEvent) {
    if (zeroSnap.value === e.altKey)
      return
    const sel = editor.selection
    const cursor = editor.cursor
    const gesture = mouseGesture
    try {
      const radius = Math.round(rate.value * 0.01)
      const [a, b] = await editor.zeroSnap(sel ? [sel.start, sel.end] : [cursor], radius)
      // Unless the user has moved on in the meantime.
      if (gesture !== mouseGesture || editor.selection !== sel || editor.cursor !== cursor)
        return
      if (sel && b > a)
        editor.selection = { start: a, end: b }
      editor.cursor = a
    }
    catch (err) {
      console.error(err)
    }
  }

  function selectLoop() {
    const lp = editor.state?.loop
    if (lp) {
      editor.selection = { ...lp }
      editor.cursor = lp.start
    }
  }

  // A selection edge under the pointer (within a few pixels, in the wave area): dragging it resizes the selection.
  const EDGE_PX = 5
  const hoverEdge = ref<'start' | 'end' | null>(null)

  function edgeAt(e: MouseEvent): 'start' | 'end' | null {
    const sel = editor.selection
    const rect = canvas.value?.getBoundingClientRect()
    if (!sel || !rect || e.clientY - rect.top < RULER)
      return null
    const px = e.clientX - rect.left
    const toStart = Math.abs(px - x(sel.start))
    const toEnd = Math.abs(px - x(sel.end))
    if (Math.min(toStart, toEnd) > EDGE_PX)
      return null
    return toStart < toEnd ? 'start' : 'end'
  }

  function loopEdgeAt(e: MouseEvent): 'start' | 'end' | null {
    const lp = draggedLoop.value ?? editor.state?.loop
    const rect = canvas.value?.getBoundingClientRect()
    if (!lp || !rect || editor.loading || editor.processing || editor.saving) return null
    // A distinct selection keeps its own resize handles in the wave area.
    const sel = editor.selection
    if (edgeAt(e) && sel && (sel.start !== lp.start || sel.end !== lp.end)) return null
    const px = e.clientX - rect.left
    const start = Math.abs(px - x(lp.start))
    const end = Math.abs(px - x(lp.end))
    return Math.min(start, end) <= EDGE_PX ? (start < end ? 'start' : 'end') : null
  }

  /** Preview each move; commit saved loop metadata once, as one undo step. */
  function dragLoop(e: MouseEvent, edge: 'start' | 'end') {
    const original = editor.state?.loop
    if (!original) return
    const gesture = mouseGesture
    const previousSelection = editor.selection
    const previousCursor = editor.cursor
    const startX = e.clientX
    let moved = false
    const clamp = (frame: number) => edge === 'start'
      ? Math.max(0, Math.min(original.end - 1, frame))
      : Math.min(frames.value, Math.max(original.start + 1, frame))
    const preview = (frame: number) => {
      const range = { ...original, [edge]: clamp(frame) }
      draggedLoop.value = range
      editor.selection = { ...range }
      if (editor.playing) void editor.updatePlaybackRange()
      return range
    }
    const move = (ev: MouseEvent) => {
      if (!moved && Math.abs(ev.clientX - startX) < 3) return
      moved = true
      preview(snapped(frameAt(ev.clientX), ev))
    }
    const up = async (ev: MouseEvent) => {
      releaseListeners()
      if (!moved) { selectLoop(); return }
      move(ev)
      const range = draggedLoop.value!
      const [at] = await zeroSnapped([range[edge]], ev)
      if (gesture !== mouseGesture) return
      if (editor.state?.loop !== original) { draggedLoop.value = null; return }
      const final = preview(at)
      editor.cursor = final.start
      const saved = await editor.setLoopPoints(final)
      if (gesture !== mouseGesture) return
      if (!saved) {
        editor.selection = previousSelection
        editor.cursor = previousCursor
        if (editor.playing) void editor.updatePlaybackRange()
      }
      draggedLoop.value = null
    }
    listen(move, up)
  }

  // --- Markers ----------------------------------------------------------------

  const hoverMarker = ref(false)

  /** The marker whose flag is under the pointer (on the ruler, within a few pixels), or -1. */
  function markerAt(e: MouseEvent) {
    const rect = canvas.value?.getBoundingClientRect()
    const markers = editor.state?.markers ?? []
    if (!rect || e.clientY - rect.top >= RULER)
      return -1
    const px = e.clientX - rect.left
    let best = -1
    let bestDistance = EDGE_PX + 1
    markers.forEach((m, i) => {
      const d = Math.abs(px - x(m.frame))
      if (d < bestDistance) {
        best = i
        bestDistance = d
      }
    })
    return best
  }

  const onRuler = (e: MouseEvent) => e.clientY - canvas.value!.getBoundingClientRect().top < RULER

  /** Moves frames to zero crossings when that snapping is on (⌥ inverts it). */
  async function zeroSnapped(at: number[], e: MouseEvent | KeyboardEvent) {
    if (zeroSnap.value === e.altKey)
      return at
    try {
      return await editor.zeroSnap(at, Math.round(rate.value * 0.01))
    }
    catch {
      return at
    }
  }

  /** New markers split regions; each half keeps the region's skip. */
  function addMarkers(at: number[]) {
    const markers = editor.state?.markers ?? []
    const added = at
      .filter(f => f >= 0 && f < frames.value && !markers.some(m => m.frame === f))
      .map(frame => ({ frame, skip: editor.regions.find(r => frame >= r.start && frame < r.end)?.skip ?? false }))
    if (added.length)
      editor.setMarkers([...markers, ...added])
  }

  function removeMarker(i: number) {
    editor.setMarkers((editor.state?.markers ?? []).filter((_, j) => j !== i))
  }

  function toggleSkip(region: { start: number, skip: boolean }) {
    const markers = editor.state?.markers ?? []
    editor.setMarkers(markers.map(m => (m.frame === region.start ? { ...m, skip: !m.skip } : m)))
  }

  /** Drags marker `i` between its neighbours, snapping like the selection. */
  function dragMarker(e: MouseEvent, i: number) {
    cancelGesture()
    const gesture = mouseGesture
    const markers = editor.state?.markers ?? []
    const lo = i > 0 ? markers[i - 1].frame + 1 : 0
    const hi = (markers[i + 1]?.frame ?? frames.value) - 1
    const startX = e.clientX
    let moved = false
    let at = markers[i].frame
    const place = (f: number) => markers.map((m, j) => (j === i ? { ...m, frame: f } : m))
    const move = (ev: MouseEvent) => {
      if (!moved && Math.abs(ev.clientX - startX) < 3)
        return
      moved = true
      at = Math.min(hi, Math.max(lo, snapped(frameAt(ev.clientX), ev)))
      draggedMarkers.value = place(at)
    }
    const up = async (ev: MouseEvent) => {
      releaseListeners()
      if (moved) {
        const [f] = await zeroSnapped([at], ev)
        if (gesture !== mouseGesture) return
        await editor.setMarkers(place(Math.min(hi, Math.max(lo, f))))
      }
      if (gesture === mouseGesture) draggedMarkers.value = null
    }
    listen(move, up)
  }

  async function onDoubleClick(e: MouseEvent) {
    if (!onRuler(e) || markerAt(e) >= 0) return
    const gesture = mouseGesture
    const at = await zeroSnapped([snapped(frameAt(e.clientX), e)], e)
    if (gesture === mouseGesture) addMarkers(at)
  }

  // --- Context menu -----------------------------------------------------------

  // What was right-clicked: a marker's flag, the selection, or any other point,
  // which moves the cursor there like a click.
  const menuOnRuler = ref(false)
  const menu = shallowRef<{ kind: 'marker', index: number } | { kind: 'selection' } | { kind: 'point', frame: number } | null>(null)
  // The region starting at the right-clicked marker, for "skip / include".
  const menuRegion = computed(() => {
    const m = menu.value
    const marker = m?.kind === 'marker' ? editor.state?.markers[m.index] : undefined
    return marker ? editor.regions.find(r => r.start === marker.frame) : undefined
  })

  function onContextMenu(e: MouseEvent) {
    menuOnRuler.value = onRuler(e)
    const i = markerAt(e)
    if (i >= 0) {
      menu.value = { kind: 'marker', index: i }
      return
    }
    const f = frameAt(e.clientX)
    const s = editor.selection
    if (s && f >= s.start && f <= s.end) {
      menu.value = { kind: 'selection' }
      return
    }
    editor.selection = null
    editor.cursor = f
    menu.value = { kind: 'point', frame: f }
  }


  function onMouseDown(e: MouseEvent) {
    if (e.button !== 0)
      return
    cancelGesture()
    const gesture = mouseGesture
    const badge = badgeAt(e)
    if (badge) {
      playRegion(badge)
      return
    }
    // A marker's flag on the ruler drags it.
    const marker = markerAt(e)
    const loopEdge = e.shiftKey ? null : loopEdgeAt(e)
    const y = e.clientY - canvas.value!.getBoundingClientRect().top
    if (marker >= 0 && (!loopEdge || y < RULER - 4)) {
      dragMarker(e, marker)
      return
    }
    if (loopEdge) {
      dragLoop(e, loopEdge)
      return
    }
    // The loop band on the ruler selects the loop.
    const lp = editor.state?.loop
    if (lp && e.clientY - canvas.value!.getBoundingClientRect().top < RULER) {
      const f = frameAt(e.clientX)
      if (f >= lp.start && f <= lp.end) {
        selectLoop()
        return
      }
    }
    // A region on the ruler: a click selects it, ⌥-click skips it or takes it back.
    if (editor.regions.length && onRuler(e)) {
      const f = frameAt(e.clientX)
      const region = editor.regions.find(r => f >= r.start && f < r.end)
      if (region) {
        if (e.altKey) {
          toggleSkip(region)
        }
        else {
          editor.selection = { start: region.start, end: region.end }
          editor.cursor = region.start
        }
        return
      }
    }
    // Grabbing an edge keeps the other one as the anchor; otherwise a new selection starts here.
    const edge = e.shiftKey ? null : edgeAt(e)
    const sel = editor.selection
    const anchor = edge && sel
      ? (edge === 'start' ? sel.end : sel.start)
      : e.shiftKey ? editor.cursor : snapped(frameAt(e.clientX), e)
    const startX = e.clientX
    let dragged = false
    const move = (ev: MouseEvent) => {
      if (Math.abs(ev.clientX - startX) < 3 && !dragged)
        return
      dragged = true
      const f = snapped(frameAt(ev.clientX), ev)
      editor.selection = f === anchor ? null : { start: Math.min(f, anchor), end: Math.max(f, anchor) }
    }
    const up = async (ev: MouseEvent) => {
      releaseListeners()
      // The last mousemove may lag behind the release point.
      if (dragged)
        move(ev)
      // A click on an edge without moving leaves the selection as it is.
      else if (edge)
        return
      if (!dragged) {
        const f = snapped(frameAt(ev.clientX), ev)
        if (e.shiftKey && f !== anchor)
          editor.selection = { start: Math.min(f, anchor), end: Math.max(f, anchor) }
        else {
          editor.selection = null
          editor.cursor = f
        }
      }
      else if (editor.selection) {
        editor.cursor = editor.selection.start
      }
      await toZeroCrossings(ev)
      if (gesture !== mouseGesture)
        return
      const selected = editor.selection
      const changedRange = selected && (!sel || selected.start !== sel.start || selected.end !== sel.end)
      // An active audition follows the edited selection, including its new end and loop bounds.
      if (editor.playing && changedRange)
        void editor.updatePlaybackRange()
      // Creating a selection while stopped remains silent; a plain click auditions the cursor.
      else if (!dragged && !e.shiftKey && e.detail < 2 && !selected && editor.cursor < frames.value)
        void editor.play()
    }
    listen(move, up)
  }


  function updateHover(event: MouseEvent) {
    hoverEdge.value = loopEdgeAt(event) ?? edgeAt(event)
    hoverMarker.value = markerAt(event) >= 0
  }
  watch(options.identity, cancelGesture, { flush: 'sync' })
  watch(() => editor.state?.loop, () => { if (draggedLoop.value) cancelGesture() }, { flush: 'sync' })
  onScopeDispose(cancelGesture)
  return { draggedMarkers, draggedLoop, hoverEdge, hoverMarker, menuOnRuler, menu, menuRegion,
    frameAt, selectLoop, playRegion, addMarkers, removeMarker, toggleSkip,
    onDoubleClick, onContextMenu, onMouseDown, updateHover }
}
