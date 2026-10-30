// Opt-in browser check on /mock.html#editor; codec bytes are tested in Rust.
export async function checkAudioExport(page) {
  const check = (value, message) => { if (!value) throw new Error(message) }
  await page.goto('http://127.0.0.1:1433/mock.html#editor')
  await page.reload()
  await page.waitForFunction(() => document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia?._s.get('editor')?.state?.frames > 0)
  await page.evaluate(async () => {
    const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
    e.selection = { start: 10000, end: 90000 }
    await e.gain(-3)
    const original = window.__TAURI_INTERNALS__.invoke
    window.__audioExport = { original, saves: [], dialogs: [], cancelFile: false }
    window.__TAURI_INTERNALS__.invoke = (cmd, args, options) => {
      if (cmd === 'plugin:dialog|save') {
        window.__audioExport.dialogs.push(args)
        return Promise.resolve(window.__audioExport.cancelFile ? null : '/mock/Chosen.wav')
      }
      if (cmd === 'editor_save') window.__audioExport.saves.push(args)
      return original(cmd, args, options)
    }
  })
  await page.waitForFunction(() => !document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').processing)
  await page.evaluate(() => { window.__audioExport.promise = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').save() })
  const dialog = page.getByRole('dialog', { name: 'Save audio' })
  try {
    await dialog.waitFor()
    await dialog.locator('#audio-export-format').click()
    await page.getByRole('option', { name: 'MP3', exact: true }).click()
    await dialog.getByText('Loop points and slice markers are saved only in WAV.').waitFor()
    await dialog.locator('#audio-export-rate').click()
    check(await page.getByRole('option', { name: '96 kHz', exact: true }).count() === 0, 'MP3 offers an unsupported sample rate')
    await page.getByRole('option', { name: '48 kHz', exact: true }).click()
    await dialog.getByRole('button', { name: 'Continue', exact: true }).click()
    check(await page.evaluate(() => window.__audioExport.promise), 'first save failed')
    check(await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const request = window.__audioExport.saves[0]
      return request.mode === 'saveAs' && request.destination === '/mock/Chosen.mp3' && request.sampleRate === 48000 && e.sample.ext === 'mp3' && e.state.sampleRate === 44100 && e.state.exportSampleRate === 48000 && !e.state.dirty
    }), 'first Save failed to select format/path or update editor identity')
    await page.evaluate(() => document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').gain(-3))
    await page.waitForFunction(() => !document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').processing)
    check(await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      return await e.save() && e.sample.path === '/mock/Chosen.mp3' && window.__audioExport.dialogs.length === 1 && window.__audioExport.saves[1].mode === 'save'
    }), 'subsequent Save must reuse its MP3 target without asking for format')
    await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      window.__audioExport.before = JSON.stringify({ sample: e.sample, state: e.state })
      window.__audioExport.promise = e.save('selection')
    })
    await dialog.waitFor()
    await dialog.locator('#audio-export-format').click()
    await page.getByRole('option', { name: 'OGG Vorbis', exact: true }).click()
    await dialog.locator('#audio-export-rate').click()
    await page.getByRole('option', { name: '96 kHz', exact: true }).click()
    await dialog.getByRole('button', { name: 'Continue', exact: true }).click()
    check(await page.evaluate(() => window.__audioExport.promise), 'OGG selection save failed')
    check(await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const request = window.__audioExport.saves[2]
      return request.destination === '/mock/Chosen.ogg' && request.sampleRate === 96000 && request.range.start === 10000 && request.range.end === 90000 && JSON.stringify({ sample: e.sample, state: e.state }) === window.__audioExport.before
    }), 'selection export replaced the editor session or lost selected range')
    await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      window.__audioExport.promise = e.save('saveAs')
    })
    await dialog.waitFor()
    await dialog.getByRole('button', { name: 'Cancel', exact: true }).click()
    check(await page.evaluate(async () => !await window.__audioExport.promise && window.__audioExport.saves.length === 3), 'format cancellation wrote a file')
    await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      window.__audioExport.cancelFile = true
      window.__audioExport.promise = e.save('saveAs')
    })
    await dialog.waitFor()
    await dialog.getByRole('button', { name: 'Continue', exact: true }).click()
    check(await page.evaluate(async () => !await window.__audioExport.promise && window.__audioExport.saves.length === 3), 'file cancellation wrote a file')
    return { passed: ['format chooser on first save', 'MP3 extension/filter', 'MP3 sample rate restrictions', 'output rate leaves live audio unchanged', 'repeat save reuses target', 'OGG 96 kHz selection preserves source', 'format cancellation', 'file cancellation'] }
  } finally {
    await page.evaluate(() => { if (window.__audioExport) window.__TAURI_INTERNALS__.invoke = window.__audioExport.original })
  }
}
