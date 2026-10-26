/** Updater lifecycle, kept independent of the UI and native transport for verification. */
export type UpdatePhase = 'idle' | 'checking' | 'current' | 'available' | 'downloading' | 'ready' | 'installing' | 'installed'
export type UpdateEvent = { event: 'Started', data: { contentLength?: number } }
  | { event: 'Progress', data: { chunkLength: number } } | { event: 'Finished' }
export interface UpdatePackage {
  version: string
  body?: string
  download: (onEvent: (event: UpdateEvent) => void) => Promise<void>
  install: () => Promise<void>
  close: () => Promise<void>
}
export interface UpdateSnapshot {
  phase: UpdatePhase
  version: string
  notes: string
  received: number
  total: number
  error: string
}
export interface UpdateDependencies {
  check: () => Promise<UpdatePackage | null>
  beforeInstall: () => Promise<void>
  relaunch: () => Promise<void>
}

export function createUpdateController(deps: UpdateDependencies, changed: (state: UpdateSnapshot) => void) {
  let state: UpdateSnapshot = { phase: 'idle', version: '', notes: '', received: 0, total: 0, error: '' }
  let update: UpdatePackage | null = null
  let busy = false
  function set(patch: Partial<UpdateSnapshot>) { state = { ...state, ...patch }; changed(state) }
  async function check() {
    // Keep a downloaded package until installation; concurrent checks cannot replace it.
    if (busy || state.phase === 'ready' || state.phase === 'installed') return
    busy = true
    const previous = update
    set({ phase: 'checking', error: '' })
    try {
      const found = await deps.check()
      update = found
      if (previous) await previous.close().catch(console.error)
      set({ phase: found ? 'available' : 'current', version: found?.version ?? '', notes: found?.body ?? '', received: 0, total: 0 })
    }
    catch (error) { set({ phase: previous ? 'available' : 'idle', error: String(error) }) }
    finally { busy = false }
  }
  async function download() {
    if (busy || !update || state.phase !== 'available') return
    busy = true
    set({ phase: 'downloading', received: 0, total: 0, error: '' })
    try {
      await update.download((event) => {
        if (event.event === 'Started') set({ total: event.data.contentLength ?? 0 })
        if (event.event === 'Progress') set({ received: state.received + event.data.chunkLength })
      })
      // Finished is a network event; signature verification must also finish successfully.
      set({ phase: 'ready' })
    }
    catch (error) { set({ phase: 'available', received: 0, total: 0, error: String(error) }) }
    finally { busy = false }
  }
  async function install() {
    if (busy || !update || (state.phase !== 'ready' && state.phase !== 'installed')) return
    busy = true
    set({ error: '' })
    let installing = false
    try {
      await deps.beforeInstall()
      if (state.phase === 'ready') {
        installing = true
        set({ phase: 'installing' })
        await update.install()
        installing = false
        set({ phase: 'installed' })
        await update.close().catch(console.error)
      }
      // A failed relaunch can be retried without trying to install the same package twice.
      await deps.relaunch()
    }
    catch (error) { set({ phase: installing ? 'ready' : state.phase, error: String(error) }) }
    finally { busy = false }
  }
  return { check, download, install }
}
