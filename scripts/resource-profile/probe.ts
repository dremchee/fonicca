// Injected into a private copy of main.ts by profile-resources.mjs. Never shipped.
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { editorTimings } from './editor-performance'
import { i18n } from './i18n'
import config from './resource-profile-config.json'

// Numeric request IDs only: the observation must not retain DOM/callback payloads.
const pendingRaf = new Set<number>()
const originalRaf = window.requestAnimationFrame.bind(window)
const originalCancelRaf = window.cancelAnimationFrame.bind(window)
window.requestAnimationFrame = (callback) => {
  const id = originalRaf(time => { pendingRaf.delete(id); callback(time) })
  pendingRaf.add(id)
  return id
}
window.cancelAnimationFrame = id => { pendingRaf.delete(id); originalCancelRaf(id) }
const pending = () => Object.fromEntries(Object.entries((globalThis as any).__FONICCA_PENDING_EDITOR__ ?? {}).filter(([, count]) => Number(count) > 0))
const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms))
async function until(check: () => unknown, label: string, limit = 60000) {
  const start = performance.now()
  while (!check()) {
    if (performance.now() - start > limit) throw Error(`Timeout: ${label}`)
    await sleep(30)
  }
}
async function post(route: string, value: unknown) {
  const response = await fetch(`${config.controllerUrl}/${route}`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-fixture-token': config.token }, body: JSON.stringify(value) })
  if (!response.ok) throw Error(await response.text())
  return response.json()
}
const stores = () => (document.querySelector('#app') as any).__vue_app__.config.globalProperties.$pinia._s
const editor = () => stores().get('editor')
const resources = () => (globalThis as any).__FONICCA_RESOURCE_STATS__ ?? null
const callbacks = () => (window as any).__TAURI_INTERNALS__?.callbacks?.size ?? null
async function checkpoint(stage: string, extra = {}) {
  const win = getCurrentWindow()
  await win.show(); await win.setFocus()
  await until(() => document.visibilityState === 'visible', `visible checkpoint: ${stage}`, 15000)
  const native = await invoke('debug_snapshot')
  return post('checkpoint', { stage, window: getCurrentWindow().label, visibility: document.visibilityState,
    focused: document.hasFocus(), viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio },
    callbacks: callbacks(), pendingRaf: pendingRaf.size, pendingEditor: pending(), resources: resources() && { ...resources() },
    canvases: document.querySelectorAll('canvas').length,
    rows: document.querySelectorAll('.sample-list .row').length,
    library: stores().get('library')?.pageDiagnostics(), libraryTotal: stores().get('library')?.total,
    editor: { open: editor()?.open, frames: editor()?.state?.frames ?? 0, dirty: editor()?.state?.dirty },
    heapBytes: (performance as any).memory?.usedJSHeapSize ?? null, native, timings: editorTimings.snapshot(), ...extra })
}
function mode(value: 'waveform' | 'spectrogram') {
  const name = i18n.global.t(value === 'waveform' ? 'editor.spectrogram.waveform' : 'editor.spectrogram.title')
  const button = document.querySelector<HTMLButtonElement>(`button[aria-label=${JSON.stringify(name)}]`)
  if (!button) throw Error(`Missing ${value} control`)
  button.click()
  return button
}
async function detached() {
  await until(() => editor()?.state && !editor().loading, 'accepted standalone session')
  await sleep(2000)
  await checkpoint('detached-rendered')
  if (!await editor().detachWindow(undefined, true)) throw Error('Native return failed')
}
async function scenario() {
  const library = stores().get('library'), e = editor()
  await until(() => library.total === config.rows && document.querySelector('.sample-list .row'), 'seeded library')
  await sleep(2000)
  await checkpoint('baseline')
  const scroll = document.querySelector<HTMLElement>('.sample-list [tabindex="0"].overflow-y-auto')!
  for (let step = 1; step <= 20; step++) { scroll.scrollTop = (scroll.scrollHeight - scroll.clientHeight) * step / 20; await sleep(80) }
  scroll.scrollTop = 0
  await sleep(1500)
  await checkpoint('library-scrolled')
  for (let cycle = 1; cycle <= config.cycles; cycle++) {
    editorTimings.reset()
    await e.openSample({ id: 2, name: 'long synthetic', path: `${config.audio}/long.wav`, ext: 'wav', bpm: 120 })
    await until(() => e.state?.frames === config.seconds * 44100 && !e.loading, 'loaded long audio')
    await until(() => editorTimings.snapshot().some(x => x.name === 'editor_wave' && x.calls && !x.errors), 'rendered long wave')
    await sleep(1200)
    if (!config.smoke && !resources()?.created) throw Error('WebGL renderer was not exercised')
    await checkpoint(`cycle-${cycle}-opened`)
    for (let edit = 0; edit < 8; edit++) {
      e.selection = { start: 0, end: 44100 * 3 }
      e.fade('out')
      await until(() => !e.processing && e.state?.canUndo, 'fade')
      await e.undo()
      await until(() => !e.processing && !e.state?.dirty, 'undo')
    }
    const spectrum = mode('spectrogram')
    await until(() => spectrum.getAttribute('aria-pressed') === 'true' && spectrum.getAttribute('aria-busy') !== 'true'
      && editorTimings.snapshot().some(x => x.name === 'editor_spectrogram' && x.calls && !x.errors), 'spectrogram', 120000)
    await sleep(1200)
    await checkpoint(`cycle-${cycle}-spectrum`)
    mode('waveform')
    await sleep(500)
    if (cycle === config.cycles) {
      if (!await e.detachWindow()) throw Error('Native detach failed')
      await until(() => e.state && !e.loading, 'returned native session', 60000)
      await sleep(1500)
      await checkpoint(`cycle-${cycle}-returned`)
    }
    if (!await e.closeEmbedded()) throw Error('Clean editor did not close')
    await until(() => !document.querySelector('[data-editor-panel]'), 'editor unmounted')
    await sleep(2500)
    await checkpoint(`cycle-${cycle}-closed`)
  }
  // Close while STFT is running, independently of the already completed wave.
  editorTimings.reset()
  await e.openSample({ id: 2, name: 'long synthetic', path: `${config.audio}/long.wav`, ext: 'wav', bpm: 120 })
  await until(() => !e.loading && e.state, 'final open')
  await until(() => editorTimings.snapshot().some(x => x.name === 'editor_wave' && x.calls && !x.errors), 'final wave complete')
  await sleep(1500)
  const began = new Promise<void>(resolve => { (globalThis as any).__FONICCA_REQUEST_STARTED__ = (name: string) => { if (name === 'editor_spectrogram') resolve() } })
  mode('spectrogram')
  await began
  delete (globalThis as any).__FONICCA_REQUEST_STARTED__
  const closeStarted = performance.now()
  if (!await e.closeEmbedded()) throw Error('Cannot close pending spectrum')
  await until(() => !document.querySelector('[data-editor-panel]'), 'pending editor unmounted')
  await checkpoint('closed-with-pending-spectrum', { sinceCloseMs: performance.now() - closeStarted })
  await until(() => !Object.keys(pending()).length, 'pending requests drained', 120000)
  const drainedMs = performance.now() - closeStarted
  await checkpoint('pending-work-drained', { sinceCloseMs: drainedMs })
  await sleep(10000)
  await checkpoint('settled-after-close')
  await post('finished', { ok: true })
}
export async function profileStart(mount: () => Promise<void>) {
  try {
    const win = getCurrentWindow()
    await win.show(); await win.setFocus()
    if (win.label === 'main') {
      const info: any = await invoke('app_info')
      if (info.dbPath.replaceAll('\\', '/') !== `${config.data.replaceAll('\\', '/')}/library.db` || info.modelsDir.replaceAll('\\', '/') !== `${config.data.replaceAll('\\', '/')}/models`) throw Error('Unsafe profile app directories')
      await post('seed', { info })
    }
    await mount()
    if (win.label === 'main') await scenario()
    else await detached()
  } catch (error) {
    console.error(error)
    await post('finished', { ok: false, error: String(error), window: getCurrentWindow().label }).catch(() => {})
  }
}
