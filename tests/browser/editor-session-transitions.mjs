// Real editor-store transitions with deferred save IPC; no user audio/files.
export async function checkEditorSessionTransitions(page) {
  const check = (value, message) => { if (!value) throw new Error(message) }
  await page.waitForFunction(() => {
    const pinia = document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia
    return pinia?._s.get('library')?.sources?.length && pinia._s.get('editor') && !pinia._s.get('editor').loading
  })
  await page.evaluate(async () => {
    const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
    await e.openSample({ id: 1, name: 'Original', path: '/mock/Original.wav', ext: 'wav', bpm: 120 })
    const original = window.__TAURI_INTERNALS__.invoke
    const test = window.__sessionTransitions = { original, opens: [], saves: [], events: [] }
    test.open = id => e.openSample({ id, name: `Track ${id}`, path: `/mock/Track ${id}.wav`, ext: 'wav', bpm: 120 })
    window.__TAURI_INTERNALS__.invoke = (cmd, args, options) => {
      if (cmd === 'editor_open') {
        test.opens.push(args)
        test.events.push(`open:${args.id}`)
      }
      if (cmd === 'editor_save') {
        test.events.push('save:start')
        return new Promise((resolve, reject) => {
          const request = { args, settled: false }
          request.resolve = async () => {
            request.settled = true
            const result = await original(cmd, args, options)
            test.events.push('save:complete')
            resolve(result)
          }
          request.reject = error => { request.settled = true; reject(error) }
          test.saves.push(request)
        })
      }
      return original(cmd, args, options)
    }
  })
  const dirty = async () => {
    await page.evaluate(() => document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').gain(-3))
    await page.waitForFunction(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      return e.state?.dirty && !e.processing
    })
    await page.evaluate(() => {
      // A pre-existing target makes the unsaved prompt's Save go straight to IPC.
      document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').state.hasSavedTarget = true
    })
  }
  const begin = async id => {
    await page.evaluate(id => { window.__sessionTransitions.transition = window.__sessionTransitions.open(id) }, id)
    await page.waitForFunction(() => document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').unsavedPrompt)
  }
  const choose = async choice => {
    await page.evaluate(choice => document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').resolveUnsaved(choice), choice)
  }
  try {
    await dirty()
    await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      e.cursor = 12345
      e.selection = { start: 10000, end: 90000 }
      window.__sessionTransitions.beforeCancel = JSON.stringify({ sample: e.sample, state: e.state, cursor: e.cursor, selection: e.selection })
    })
    await begin(2)
    await choose('cancel')
    check(await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor'), test = window.__sessionTransitions
      await test.transition
      return !e.loading && !e.saving && !e.unsavedPrompt && !test.opens.length && !test.saves.length
        && JSON.stringify({ sample: e.sample, state: e.state, cursor: e.cursor, selection: e.selection }) === test.beforeCancel
    }), 'Cancel changed the dirty session or opened the requested track')

    await begin(2)
    await choose('discard')
    check(await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor'), test = window.__sessionTransitions
      await test.transition
      return e.sample.id === 2 && !e.state.dirty && !e.loading && !e.saving && !test.saves.length
        && test.opens.length === 1 && test.opens[0].discardExisting === true
    }), 'Discard did not open a clean replacement with explicit native discard')

    await dirty()
    await begin(3)
    await choose('save')
    await page.waitForFunction(() => window.__sessionTransitions.saves.length === 1)
    check(await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor'), test = window.__sessionTransitions
      await test.open(99)
      const closed = await e.closeEmbedded()
      return e.loading && e.saving && e.sample.id === 2 && e.state.dirty && !closed
        && test.opens.length === 1 && test.saves[0].args.mode === 'save'
    }), 'Pending Save must run while loading and block replacement/close until it completes')
    await page.evaluate(() => window.__sessionTransitions.saves[0].resolve())
    check(await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor'), test = window.__sessionTransitions
      await test.transition
      return e.sample.id === 3 && !e.state.dirty && !e.loading && !e.saving && !e.unsavedPrompt
        && test.opens.length === 2 && test.opens[1].id === 3 && test.opens[1].discardExisting === false
        && test.events.indexOf('save:complete') < test.events.indexOf('open:3')
    }), 'Replacement opened before the dirty session finished saving')

    await dirty()
    await begin(4)
    await choose('save')
    await page.waitForFunction(() => window.__sessionTransitions.saves.length === 2)
    await page.evaluate(() => window.__sessionTransitions.saves[1].reject(new Error('Synthetic save failure')))
    check(await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor'), test = window.__sessionTransitions
      await test.transition
      return e.sample.id === 3 && e.state.dirty && !e.loading && !e.saving && !e.unsavedPrompt && test.opens.length === 2
    }), 'A failed save discarded edits or opened the replacement')

    await page.evaluate(() => {
      document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').state.hasSavedTarget = false
    })
    await begin(5)
    await choose('save')
    const exportDialog = page.getByRole('dialog', { name: 'Save audio' })
    await exportDialog.waitFor()
    await exportDialog.getByRole('button', { name: 'Cancel', exact: true }).click()
    check(await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor'), test = window.__sessionTransitions
      await test.transition
      return e.sample.id === 3 && e.state.dirty && !e.loading && !e.saving && !e.unsavedPrompt
        && test.opens.length === 2 && test.saves.length === 2
    }), 'Cancelling Save As opened the replacement or wrote a file')
    return { passed: ['dirty Cancel preserves session', 'Discard explicitly replaces dirty session', 'Save runs while transition loading', 'pending Save blocks competing open/close', 'replacement waits for save completion', 'failed Save preserves dirty session', 'cancelled Save As blocks replacement'] }
  } finally {
    await page.evaluate(async () => {
      // A just-cancelled dialog may still be visible during its exit animation.
      // Cleanup must not wait for a closing button to become actionable again.
      const { settleAudioExport } = await import('/src/composables/audioExport.ts')
      settleAudioExport(null)
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor'), test = window.__sessionTransitions
      e.resolveUnsaved('cancel')
      for (const request of test.saves) if (!request.settled) request.reject(new Error('Session fixture cleanup'))
      window.__TAURI_INTERNALS__.invoke = test.original
      delete window.__sessionTransitions
    })
  }
}
