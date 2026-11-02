// A renderer can reload after native acceptance but before its URL token is removed.
export async function checkEditorTransferReload(page) {
  // Exercise the reload branch regardless of how the runner first navigated here.
  await page.reload()
  const check = (value, message) => { if (!value) throw new Error(message) }
  await page.waitForFunction(() => document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia?._s.get('editor')?.state)
  const result = await page.evaluate(async () => {
    const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
    await e.reset()
    const original = window.__TAURI_INTERNALS__.invoke
    const originalUrl = location.href
    const test = { calls: [], sourceOwns: false }
    window.__TAURI_INTERNALS__.invoke = (cmd, args, options) => {
      test.calls.push({ cmd, args })
      if (cmd === 'editor_prepare_transfer') throw Error('Editor transfer expired')
      if (cmd === 'editor_open' && args.reuse && test.sourceOwns) throw Error('Source still owns the session')
      return original(cmd, args, options)
    }
    const setUrl = token => {
      const url = new URL(location.href)
      url.searchParams.set('editor', '1')
      url.searchParams.set('transfer', String(token))
      url.searchParams.set('detached', '1')
      url.searchParams.set('view', JSON.stringify({ cursor: 100, selection: null, looping: false, curve: 'linear' }))
      history.replaceState(null, '', url)
    }
    try {
      setUrl(99)
      await e.init(1)
      const owned = {
        ready: !!e.sample && !!e.state && !e.state.dirty && !e.error,
        tokenRemoved: !new URLSearchParams(location.search).has('transfer'),
        calls: test.calls.slice(),
      }
      await e.reset()
      test.calls.length = 0
      test.sourceOwns = true
      setUrl(100)
      await e.init(1)
      const unowned = {
        empty: !e.sample && !e.state && !e.loading,
        error: e.error,
        calls: test.calls.slice(),
      }
      return { owned, unowned }
    } finally {
      window.__TAURI_INTERNALS__.invoke = original
      history.replaceState(null, '', originalUrl)
    }
  })
  check(result.owned.ready && result.owned.tokenRemoved, 'committed transfer reload left a hidden session or stale token')
  const ownedOpens = result.owned.calls.filter(c => c.cmd === 'editor_open')
  check(ownedOpens.length === 2 && ownedOpens[0].args.reuse && !ownedOpens[1].args.reuse,
    'recovery did not verify ownership before resetting and decoding')
  const ownedClose = result.owned.calls.find(c => c.cmd === 'editor_close')
  check(ownedClose?.args.discard, 'committed reload did not use the explicit reset policy')
  check(result.unowned.empty && result.unowned.error && !result.unowned.calls.some(c => c.cmd === 'editor_close'),
    'failed ownership probe discarded a session at its source')
  check(result.unowned.calls.filter(c => c.cmd === 'editor_open').every(c => c.args.reuse),
    'unowned target decoded a replacement instead of retaining the source')
  return { passed: ['committed reload recovers from an unconsumed URL token', 'ownership probe precedes explicit reset', 'unowned source is never discarded or decoded'] }
}
