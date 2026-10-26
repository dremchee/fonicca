import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createUpdateController, type UpdatePackage, type UpdateSnapshot } from '../src/update-controller.ts'

function fixture(overrides: Partial<UpdatePackage> = {}) {
  const calls: string[] = []
  let state: UpdateSnapshot
  const pkg: UpdatePackage = {
    version: '0.2.0', body: 'Changes', close: async () => { calls.push('close') },
    download: async (progress) => {
      calls.push('download')
      progress({ event: 'Started', data: { contentLength: 100 } })
      progress({ event: 'Progress', data: { chunkLength: 40 } })
      progress({ event: 'Progress', data: { chunkLength: 60 } })
      progress({ event: 'Finished' })
    },
    install: async () => { calls.push('install') }, ...overrides,
  }
  let blocker = ''
  let restartFails = false
  const controller = createUpdateController({
    check: async () => { calls.push('check'); return pkg },
    beforeInstall: async () => { calls.push('guard'); if (blocker) throw Error(blocker) },
    relaunch: async () => { calls.push('restart'); if (restartFails) throw Error('restart failed') },
  }, value => { state = value })
  return { controller, calls, state: () => state!, block: (value: string) => { blocker = value }, restartFails: () => { restartFails = true } }
}

test('download tracks chunks and does not install until explicitly requested', async () => {
  const f = fixture()
  await f.controller.check()
  assert.equal(f.state().phase, 'available')
  await f.controller.download()
  assert.deepEqual(f.calls, ['check', 'download'])
  assert.equal(f.state().phase, 'ready')
  assert.equal(f.state().received, 100)
  assert.equal(f.state().total, 100)
  await f.controller.check()
  assert.deepEqual(f.calls, ['check', 'download'])
  await f.controller.install()
  assert.deepEqual(f.calls, ['check', 'download', 'guard', 'install', 'close', 'restart'])
})

test('a failed signature check after Finished never allows installation and can retry', async () => {
  let fail = true
  const f = fixture({ download: async progress => {
    progress({ event: 'Finished' })
    if (fail) throw Error('invalid signature')
  } })
  await f.controller.check(); await f.controller.download(); await f.controller.install()
  assert.equal(f.state().phase, 'available')
  assert.match(f.state().error, /invalid signature/)
  assert.ok(!f.calls.includes('install'))
  fail = false
  await f.controller.download()
  assert.equal(f.state().phase, 'ready')
})

test('native installation guard is evaluated after download and keeps a blocked update ready', async () => {
  const f = fixture()
  await f.controller.check(); await f.controller.download()
  f.block('editor open')
  await f.controller.install()
  assert.equal(f.state().phase, 'ready')
  assert.match(f.state().error, /editor open/)
  assert.ok(!f.calls.includes('install'))
  f.block('')
  await f.controller.install()
  assert.equal(f.state().phase, 'installed')
})

test('relaunch failure retries only relaunch, never installs twice', async () => {
  const f = fixture()
  await f.controller.check(); await f.controller.download()
  f.restartFails()
  await f.controller.install(); await f.controller.install()
  assert.equal(f.state().phase, 'installed')
  assert.equal(f.calls.filter(c => c === 'install').length, 1)
  assert.equal(f.calls.filter(c => c === 'restart').length, 2)
})

test('concurrent checks/downloads cannot replace an update resource', async () => {
  let finish!: () => void
  const f = fixture({ download: () => new Promise<void>(resolve => { finish = resolve }) })
  await f.controller.check(); await f.controller.check()
  assert.ok(f.calls.includes('close'))
  const pending = f.controller.download()
  await f.controller.check(); await f.controller.download(); await f.controller.install()
  assert.equal(f.state().phase, 'downloading')
  assert.equal(f.calls.filter(c => c === 'check').length, 2)
  finish(); await pending
  assert.equal(f.state().phase, 'ready')
})

test('no update and network errors remain distinguishable', async () => {
  let state: UpdateSnapshot | undefined
  let fail = false
  const controller = createUpdateController({
    check: async () => { if (fail) throw Error('offline'); return null },
    beforeInstall: async () => {}, relaunch: async () => {},
  }, value => { state = value })
  await controller.check()
  assert.equal(state?.phase, 'current')
  fail = true
  await controller.check()
  assert.equal(state?.phase, 'idle')
  assert.match(state!.error, /offline/)
})
