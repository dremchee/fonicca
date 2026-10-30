import assert from 'node:assert/strict'
import { test } from 'node:test'
import { setImmediate } from 'node:timers/promises'
import { effectScope } from 'vue'
import { createJobPoller, createSharedJobPoller, type JobClock } from '../src/job-poller.ts'
import { useBackgroundJob } from '../src/composables/backgroundJob.ts'

function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (error: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
function scheduler() {
  const tasks = new Map<() => void, number>()
  const clock: JobClock = (delay, fn) => { tasks.set(fn, delay); return () => { tasks.delete(fn) } }
  const fire = () => { const fn = tasks.keys().next().value!; assert.ok(fn); tasks.delete(fn); fn() }
  return { clock, tasks, fire }
}
const status = (running = false, name = '') => ({ progress: { running, name }, modelReady: true, downloadBytes: 42 })

test('status reads serialize and coalesce refreshes without publishing the old model', async () => {
  const first = deferred<ReturnType<typeof status>>(), second = deferred<ReturnType<typeof status>>()
  let reads = 0
  const applied: string[] = [], errors: unknown[] = []
  const timer = scheduler()
  const poller = createJobPoller({ read: () => ++reads === 1 ? first.promise : second.promise,
    apply: state => applied.push(state.progress.name), running: state => state.progress.running,
    onError: error => errors.push(error), clock: timer.clock })
  const old = poller.refresh()
  await setImmediate()
  const newer = poller.refresh(), duplicate = poller.refresh()
  assert.equal(reads, 1)
  first.resolve(status(true, 'old model'))
  await setImmediate()
  assert.deepEqual(applied, [])
  assert.equal(reads, 2)
  second.resolve(status(false, 'new model'))
  await Promise.all([old, newer, duplicate])
  assert.deepEqual(applied, ['new model'])
  assert.deepEqual(errors, [])
  assert.equal(timer.tasks.size, 0)
  poller.dispose()
})

test('a transient status failure retries with bounded backoff and stops on idle', async () => {
  const timer = scheduler()
  let failures = 8, reads = 0, errors = 0, applied = 0
  const poller = createJobPoller({ read: async () => { reads++; if (failures-- > 0) throw Error('temporary'); return status() },
    apply: () => applied++, running: state => state.progress.running, onError: () => errors++, clock: timer.clock })
  await poller.refresh()
  for (const delay of [500, 1000, 2000, 4000, 5000, 5000, 5000, 5000]) {
    assert.deepEqual([...timer.tasks.values()], [delay])
    timer.fire()
    await setImmediate()
  }
  assert.equal(reads, 9)
  assert.equal(errors, 8)
  assert.equal(applied, 1)
  assert.equal(timer.tasks.size, 0)
  poller.dispose()
})

test('invalidation and disposal discard late errors/results and release waiting callers', async () => {
  const timer = scheduler(), first = deferred<ReturnType<typeof status>>()
  let applied = 0, errors = 0
  const poller = createJobPoller({ read: () => first.promise, apply: () => applied++,
    running: state => state.progress.running, onError: () => errors++, clock: timer.clock })
  const pending = poller.refresh()
  poller.invalidate()
  first.reject(Error('old read'))
  await pending
  assert.equal(errors, 0)
  assert.equal(timer.tasks.size, 0)
  const second = deferred<ReturnType<typeof status>>()
  const disposed = createJobPoller({ read: () => second.promise, apply: () => applied++,
    running: state => state.progress.running, onError: () => errors++, clock: timer.clock })
  const waiter = disposed.refresh()
  disposed.dispose()
  await waiter
  second.resolve(status(true))
  await setImmediate()
  assert.equal(applied, 0)
  assert.equal(timer.tasks.size, 0)
})

test('shared command state prevents duplicate start/cancel and awaits native cancellation', async () => {
  const owner = effectScope(), timer = scheduler()
  let running = false, starts = 0, cancels = 0
  const job = owner.run(() => useBackgroundJob({ initial: status().progress, bytes: 0, clock: timer.clock,
    read: async () => status(running), subscribe: async () => () => {} }))!
  try {
    await job.init()
    const start = deferred<void>()
    const command = async () => { starts++; await start.promise; running = true }
    const pending = job.start(command)
    await job.start(command)
    assert.equal(starts, 1)
    assert.equal(job.starting.value, true)
    start.resolve()
    await pending
    assert.equal(job.progress.value.running, true)
    assert.equal(job.starting.value, false)
    const stop = deferred<void>()
    const cancel = async () => { cancels++; await stop.promise; running = false }
    const cancelling = job.cancel(cancel)
    await job.cancel(cancel)
    assert.equal(cancels, 1)
    assert.equal(job.cancelling.value, true)
    stop.resolve()
    await cancelling
    assert.equal(job.cancelling.value, false)
    assert.equal(job.progress.value.running, false)
    assert.equal(timer.tasks.size, 0)
  } finally { owner.stop() }
})

test('cancel failure keeps its message while polling resumes; status errors clear on recovery', async () => {
  const owner = effectScope(), timer = scheduler()
  let fails = false
  const job = owner.run(() => useBackgroundJob({ initial: status().progress, bytes: 0, clock: timer.clock,
    read: async () => { if (fails) throw Error('status unavailable'); return status(true) },
    subscribe: async () => () => {} }))!
  try {
    await job.init()
    await job.cancel(async () => { throw Error('cannot cancel') })
    assert.match(job.error.value, /cannot cancel/)
    assert.equal(job.cancelling.value, false)
    assert.equal(timer.tasks.size, 1)
    fails = true
    await job.load()
    assert.match(job.error.value, /status unavailable/)
    fails = false
    await job.load()
    assert.equal(job.error.value, '')
  } finally { owner.stop() }
  assert.equal(timer.tasks.size, 0)
})

test('subscription initialization coalesces, can retry and releases a late listener after disposal', async () => {
  const owner = effectScope(), timer = scheduler(), listener = deferred<() => void>()
  let subscriptions = 0, released = 0, reads = 0
  const job = owner.run(() => useBackgroundJob({ initial: status().progress, bytes: 0, clock: timer.clock,
    read: async () => { reads++; return status() },
    subscribe: () => { subscriptions++; return listener.promise } }))!
  const a = job.init(), b = job.init()
  await setImmediate()
  assert.equal(subscriptions, 1)
  owner.stop()
  listener.resolve(() => released++)
  await Promise.all([a, b])
  assert.equal(released, 1)
  assert.equal(reads, 0)
  const retryOwner = effectScope()
  let fail = true
  const retry = retryOwner.run(() => useBackgroundJob({ initial: status().progress, bytes: 0, clock: timer.clock,
    read: async () => status(), subscribe: () => { if (fail) throw Error('listen failed'); return Promise.resolve(() => released++) } }))!
  await retry.init()
  assert.match(retry.error.value, /listen failed/)
  fail = false
  await retry.init()
  assert.equal(retry.error.value, '')
  retryOwner.stop()
  assert.equal(released, 2)
})

test('closing a save dialog does not launch or fetch a job', async () => {
  const owner = effectScope()
  let reads = 0
  const job = owner.run(() => useBackgroundJob({ initial: status().progress, bytes: 0,
    read: async () => { reads++; return status() }, subscribe: async () => () => {} }))!
  try {
    await job.start(async () => false)
    assert.equal(reads, 0)
    assert.equal(job.starting.value, false)
  } finally { owner.stop() }
})

test('init still coalesces after subscribing while the first status is pending', async () => {
  const owner = effectScope(), first = deferred<ReturnType<typeof status>>()
  const job = owner.run(() => useBackgroundJob({ initial: status().progress, bytes: 0,
    read: () => first.promise, subscribe: async () => () => {} }))!
  const initial = job.init()
  await setImmediate()
  assert.equal(job.init(), initial)
  owner.stop()
  await initial
  first.resolve(status())
})

test('starting a command invalidates a status read made before the command', async () => {
  const owner = effectScope(), timer = scheduler(), old = deferred<ReturnType<typeof status>>()
  let reads = 0
  const job = owner.run(() => useBackgroundJob({ initial: status().progress, bytes: 0, clock: timer.clock,
    read: () => ++reads === 1 ? old.promise : Promise.resolve(status(true, 'new job')),
    subscribe: async () => () => {} }))!
  try {
    const before = job.load()
    await setImmediate()
    const started = job.start(async () => {})
    old.resolve(status(false, 'old job'))
    await Promise.all([before, started])
    assert.equal(reads, 2)
    assert.equal(job.progress.value.name, 'new job')
    assert.equal(job.progress.value.running, true)
  } finally { owner.stop() }
})

test('mounted model views share one request loop and cannot receive a disposed loop result', async () => {
  const timer = scheduler(), first = deferred<ReturnType<typeof status>>(), second = deferred<ReturnType<typeof status>>()
  let reads = 0
  const applied: string[] = []
  const shared = createSharedJobPoller({ clock: timer.clock, read: () => ++reads === 1 ? first.promise : second.promise,
    apply: state => applied.push(state.progress.name), running: () => true, onError: () => assert.fail('unexpected read failure') })
  const badge = shared.acquire(), settings = shared.acquire()
  await setImmediate()
  assert.equal(reads, 1)
  badge.release(); badge.release()
  assert.equal(timer.tasks.size, 0)
  settings.release()
  const reopened = shared.acquire()
  await setImmediate()
  assert.equal(reads, 2)
  first.resolve(status(true, 'old window'))
  second.resolve(status(true, 'current window'))
  await setImmediate()
  assert.deepEqual(applied, ['current window'])
  assert.equal(timer.tasks.size, 1)
  reopened.release()
  assert.equal(timer.tasks.size, 0)
})
