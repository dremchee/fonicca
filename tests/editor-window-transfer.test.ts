import assert from 'node:assert/strict'
import { test } from 'node:test'
import { acceptEditorTransfer } from '../src/editor-window-transfer.ts'

test('return stages the destination before acceptance and never decodes or closes audio', async () => {
  const calls: string[] = []
  let finish!: () => void
  const pending = acceptEditorTransfer(7, {
    prepareTransfer: async token => { assert.equal(token, 7); calls.push('prepare'); return { frames: 100 } },
    acceptTransfer: token => { assert.equal(token, 7); calls.push('accept'); return new Promise(resolve => { finish = resolve }) },
    rejectTransfer: async () => { calls.push('reject') },
  }, { stage: opened => { assert.equal(opened.frames, 100); calls.push('stage') }, rollback: () => { calls.push('rollback') } })
  await new Promise(resolve => setImmediate(resolve))
  assert.deepEqual(calls, ['prepare', 'stage', 'accept'])
  finish()
  await pending
  assert.deepEqual(calls, ['prepare', 'stage', 'accept'])
})

test('preparation, UI staging and acceptance errors reject the offer and roll back', async () => {
  for (const failure of ['prepare', 'stage', 'accept']) {
    const calls: string[] = []
    const error = new Error(failure)
    const step = (name: string) => { calls.push(name); if (failure === name) throw error }
    await assert.rejects(acceptEditorTransfer(4, {
      prepareTransfer: async () => { step('prepare'); return 1 },
      acceptTransfer: async () => { step('accept') },
      rejectTransfer: async () => { calls.push('reject'); throw new Error('offer already expired') },
    }, { stage: () => step('stage'), rollback: () => { calls.push('rollback') } }), value => value === error)
    assert.deepEqual(calls.slice(-2), ['rollback', 'reject'])
    if (failure !== 'accept') assert.ok(!calls.includes('accept'))
  }
})

test('native acceptance waits for asynchronous UI readiness', async () => {
  let ready!: () => void
  let accepted = false
  const pending = acceptEditorTransfer(1, {
    prepareTransfer: async () => 1,
    acceptTransfer: async () => { accepted = true },
    rejectTransfer: async () => {},
  }, {
    stage: () => new Promise<void>(resolve => { ready = resolve }),
    rollback: () => {},
  })
  await new Promise(resolve => setImmediate(resolve))
  assert.equal(accepted, false)
  ready()
  await pending
  assert.equal(accepted, true)
})
