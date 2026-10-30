export type JobClock = (delayMs: number, callback: () => void) => () => void
interface Options<Status> {
  read: () => Promise<Status>
  apply: (status: Status) => void
  running: (status: Status) => boolean
  onError: (error: unknown) => void
  intervalMs?: number
  clock?: JobClock
}

/** One status read at a time. Refreshes coalesce, but still invalidate old reads. */
export function createJobPoller<Status>(options: Options<Status>) {
  const interval = options.intervalMs ?? 500
  const clock: JobClock = options.clock ?? ((delay, callback) => {
    const timer = setTimeout(callback, delay)
    return () => clearTimeout(timer)
  })
  let version = 0, busy = false, dirty = false, disposed = false, failures = 0
  let cancelTimer: (() => void) | undefined
  let waiters: (() => void)[] = []
  function stopTimer() { cancelTimer?.(); cancelTimer = undefined }
  function later(delay: number) {
    stopTimer()
    cancelTimer = clock(delay, () => { cancelTimer = undefined; void refresh() })
  }
  function settle() {
    const ready = waiters
    waiters = []
    for (const resolve of ready) resolve()
  }
  function pump() {
    if (busy || !dirty || disposed) return
    dirty = false; busy = true
    const current = version
    void Promise.resolve().then(options.read).then(status => {
      if (disposed || current !== version) return
      failures = 0
      options.apply(status)
      if (current === version && options.running(status)) later(interval)
    }).catch(error => {
      if (disposed || current !== version) return
      options.onError(error)
      later(Math.min(5000, interval * 2 ** Math.min(failures++, 4)))
    }).finally(() => {
      busy = false
      if (dirty && !disposed) pump()
      else settle()
    })
  }
  function refresh(): Promise<void> {
    if (disposed) return Promise.resolve()
    stopTimer(); version++; dirty = true
    const done = new Promise<void>(resolve => waiters.push(resolve))
    pump()
    return done
  }
  /** Invalidate a pre-command status without cancelling the native job. */
  function invalidate() { version++; dirty = false; stopTimer() }
  function dispose() { disposed = true; invalidate(); settle() }
  return { refresh, invalidate, dispose }
}

/** Mounted consumers share one poller; the last release ends its lifetime. */
export function createSharedJobPoller<Status>(options: Options<Status>) {
  let poller: ReturnType<typeof createJobPoller<Status>> | undefined
  let consumers = 0
  return {
    acquire() {
      poller ??= createJobPoller(options)
      const current = poller
      if (++consumers === 1) void current.refresh()
      let released = false
      return {
        refresh: () => released ? Promise.resolve() : current.refresh(),
        release() {
          if (released) return
          released = true
          if (--consumers === 0) { current.dispose(); poller = undefined }
        },
      }
    },
  }
}
