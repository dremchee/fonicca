import { onScopeDispose, shallowRef } from 'vue'
import { createJobPoller, type JobClock } from '../job-poller.ts'

interface JobProgress { running: boolean }
export interface ModelJobStatus<Progress extends JobProgress> {
  progress: Progress
  modelReady: boolean
  downloadBytes: number
}
interface Options<Progress extends JobProgress> {
  initial: Progress
  bytes: number
  read: () => Promise<ModelJobStatus<Progress>>
  subscribe: (reload: () => void) => Promise<() => void>
  onStatus?: (status: ModelJobStatus<Progress>) => void
  clock?: JobClock
}

/** Shared status/command lifetime; each store still owns its source and DSP options. */
export function useBackgroundJob<Progress extends JobProgress>(options: Options<Progress>) {
  const progress = shallowRef<Progress>(options.initial)
  const ready = shallowRef(false), bytes = shallowRef(options.bytes), error = shallowRef('')
  const starting = shallowRef(false), cancelling = shallowRef(false)
  let disposed = false, statusError: string | undefined
  let unlisten: (() => void) | undefined
  let initializing: Promise<void> | undefined
  function clearError() { error.value = ''; statusError = undefined }
  function commandError(cause: unknown) { error.value = String(cause); statusError = undefined }
  const poller = createJobPoller({
    read: options.read, clock: options.clock,
    running: status => status.progress.running,
    apply(status) {
      options.onStatus?.(status)
      progress.value = status.progress; ready.value = status.modelReady; bytes.value = status.downloadBytes
      if (!status.progress.running) cancelling.value = false
      if (statusError !== undefined && error.value === statusError) error.value = ''
      statusError = undefined
    },
    onError(cause) { statusError = String(cause); error.value = statusError },
  })
  function init(): Promise<void> {
    if (disposed) return Promise.resolve()
    if (initializing) return initializing
    if (unlisten) return Promise.resolve()
    clearError()
    initializing = Promise.resolve().then(async () => {
      try {
        const release = await options.subscribe(() => { void poller.refresh() })
        if (disposed) { release(); return }
        unlisten = release
        await poller.refresh()
      }
      catch (cause) { if (!disposed) commandError(cause) }
      finally { initializing = undefined }
    })
    return initializing
  }
  async function start(command: () => Promise<void | false>) {
    if (disposed || starting.value || progress.value.running) return
    starting.value = true; clearError(); poller.invalidate()
    try {
      const started = await command()
      if (!disposed && started !== false) await poller.refresh()
    }
    catch (cause) { if (!disposed) { commandError(cause); await poller.refresh() } }
    finally { if (!disposed) starting.value = false }
  }
  async function cancel(command: () => Promise<void>) {
    if (disposed || cancelling.value) return
    cancelling.value = true; poller.invalidate()
    try { await command(); if (!disposed) await poller.refresh() }
    catch (cause) {
      if (!disposed) { cancelling.value = false; commandError(cause); await poller.refresh() }
    }
  }
  onScopeDispose(() => {
    disposed = true; poller.dispose(); unlisten?.(); unlisten = undefined
  })
  return { progress, ready, bytes, error, starting, cancelling, init, load: poller.refresh, clearError, start, cancel }
}
