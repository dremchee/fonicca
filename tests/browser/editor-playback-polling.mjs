// Deferred native replies exercise the editor's request-generation boundary.
export async function checkEditorPlaybackPolling(page) {
  const check = (value, message) => { if (!value) throw Error(message) }
  await page.waitForFunction(() => {
    const editor = document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia?._s.get('editor')
    return editor?.state && !editor.loading && !editor.processing
  })
  await page.evaluate(async () => {
    const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
    await editor.stopPlayback()
    const original = window.__TAURI_INTERNALS__.invoke
    const fixture = window.__editorPolling = {
      original, positions: [], updates: [], actions: [],
      originalCursor: editor.cursor,
      originalSelection: editor.selection && { ...editor.selection },
      originalLooping: editor.looping,
      step: Math.floor(editor.state.frames / 10),
      rate: editor.state.sampleRate,
    }
    if (fixture.step < 10 || fixture.rate <= 0) throw Error('Playback polling fixture requires a loaded audio track')
    fixture.afterFrames = () => new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(Error('Timed out waiting for animation frames')), 5000)
      requestAnimationFrame(() => requestAnimationFrame(() => { clearTimeout(timeout); resolve() }))
    })
    const defer = records => {
      const record = { settled: false }
      record.promise = new Promise((resolve, reject) => {
        record.resolve = value => { record.settled = true; resolve(value) }
        record.reject = error => { record.settled = true; reject(error) }
      })
      records.push(record)
      return record.promise
    }
    window.__TAURI_INTERNALS__.invoke = (command, args, options) => {
      if (command === 'playback_position') return defer(fixture.positions)
      if (command === 'editor_update_playback') return defer(fixture.updates)
      return original(command, args, options)
    }
    editor.selection = null
    editor.looping = false
    editor.cursor = fixture.step
    await editor.play()
  })
  try {
    await page.waitForFunction(() => window.__editorPolling.positions.length === 1, undefined, { timeout: 5000 })
    check(await page.evaluate(async () => {
      const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const fixture = window.__editorPolling
      await editor.stopPlayback()
      fixture.positions[0].resolve(1)
      await fixture.afterFrames()
      return !editor.playing && editor.playhead === null && fixture.positions.length === 1
    }), 'a late position reply restored playback or scheduled another poll after stop')

    await page.evaluate(async () => {
      const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      editor.cursor = window.__editorPolling.step * 2
      await editor.play()
    })
    await page.waitForFunction(() => window.__editorPolling.positions.length === 2, undefined, { timeout: 5000 })
    await page.evaluate(async () => {
      const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      editor.cursor = window.__editorPolling.step * 3
      await editor.play()
    })
    await page.waitForFunction(() => window.__editorPolling.positions.length === 3, undefined, { timeout: 5000 })
    await page.evaluate(() => {
      const fixture = window.__editorPolling
      fixture.currentFrame = fixture.step * 3 + 5
      fixture.positions[2].resolve(fixture.currentFrame / fixture.rate)
    })
    await page.waitForFunction(() => {
      const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      return editor.playhead === window.__editorPolling.currentFrame && window.__editorPolling.positions.length === 4
    }, undefined, { timeout: 5000 })
    check(await page.evaluate(async () => {
      const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const fixture = window.__editorPolling
      fixture.positions[1].resolve(fixture.step / fixture.rate)
      await fixture.afterFrames()
      return editor.playing && editor.playhead === fixture.currentFrame && fixture.positions.length === 4
    }), 'a previous playback poll overwrote the current playhead or started a competing polling loop')

    await page.evaluate(() => {
      const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const fixture = window.__editorPolling
      editor.selection = { start: fixture.step, end: fixture.step * 2 }
      fixture.rangeFalse = editor.updatePlaybackRange()
      fixture.actions.push(fixture.rangeFalse)
    })
    await page.waitForFunction(() => window.__editorPolling.updates.length === 1, undefined, { timeout: 5000 })
    await page.evaluate(async () => {
      const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const fixture = window.__editorPolling
      editor.selection = null
      editor.cursor = fixture.step * 4
      await editor.play()
    })
    await page.waitForFunction(() => window.__editorPolling.positions.length === 5, undefined, { timeout: 5000 })
    check(await page.evaluate(async () => {
      const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const fixture = window.__editorPolling
      fixture.updates[0].resolve(false)
      await fixture.rangeFalse
      return editor.playing && editor.playhead === fixture.step * 4
    }), 'a stale inactive-range reply stopped the new playback')

    await page.evaluate(() => {
      const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const fixture = window.__editorPolling
      editor.selection = { start: fixture.step * 2, end: fixture.step * 3 }
      fixture.rangeError = editor.updatePlaybackRange()
      fixture.actions.push(fixture.rangeError)
    })
    await page.waitForFunction(() => window.__editorPolling.updates.length === 2, undefined, { timeout: 5000 })
    await page.evaluate(async () => {
      const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const fixture = window.__editorPolling
      editor.selection = null
      editor.cursor = fixture.step * 5
      await editor.play()
    })
    await page.waitForFunction(() => window.__editorPolling.positions.length === 6, undefined, { timeout: 5000 })
    check(await page.evaluate(async () => {
      const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const fixture = window.__editorPolling
      fixture.updates[1].reject(Error('stale polling range fixture error'))
      await fixture.rangeError
      await fixture.afterFrames()
      return editor.playing && editor.playhead === fixture.step * 5
    }), 'a stale range failure changed the new playback state')
    check(await page.getByText('Error: stale polling range fixture error', { exact: true }).count() === 0,
      'a stale range failure produced an error toast for the new playback')
    return { passed: ['late position after stop stays stopped without repolling', 'old position cannot overwrite a newer playhead',
      'one current polling loop', 'stale range inactivity preserves new playback', 'stale range errors stay silent'] }
  } finally {
    await page.evaluate(async () => {
      const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const fixture = window.__editorPolling
      try {
        await editor.stopPlayback()
        for (const record of fixture.positions) if (!record.settled) record.resolve(null)
        for (const record of fixture.updates) if (!record.settled) record.resolve(false)
        await Promise.allSettled(fixture.actions)
        await fixture.afterFrames()
      } finally {
        window.__TAURI_INTERNALS__.invoke = fixture.original
        editor.cursor = fixture.originalCursor
        editor.selection = fixture.originalSelection
        editor.looping = fixture.originalLooping
        delete window.__editorPolling
      }
    })
  }
}
