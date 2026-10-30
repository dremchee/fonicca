// Remount a fresh UI while retaining the fake native backend in the same realm.
// This reproduces a renderer reload without relying on real user audio/files.
export async function checkEditorReloadReset(page) {
  const check = (value, message) => { if (!value) throw new Error(message) }
  await page.goto('http://127.0.0.1:1433/mock.html')
  await page.reload()
  await page.waitForFunction(() => document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia?._s.get('library')?.sources?.length > 0)
  await page.evaluate(async () => {
    const pinia = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia
    const editor = pinia._s.get('editor')
    await editor.openSample({ id: 1, name: 'Before reload', path: '/mock/Drum loop.wav', ext: 'wav', bpm: 120 })
    editor.selection = { start: 1000, end: 100000 }
    editor.gain(-3)
  })
  await page.waitForFunction(() => {
    const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
    return e.state?.dirty && !e.processing
  })
  await page.evaluate(async () => {
    const old = document.querySelector('#app').__vue_app__
    const pinia = old.config.globalProperties.$pinia
    const editor = pinia._s.get('editor')
    editor.playing = true
    editor.looping = true
    editor.expanded = true
    const original = window.__TAURI_INTERNALS__.invoke
    window.__reloadReset = { original, dirty: true, stops: 0, closes: 0, opens: 0, saves: 0 }
    window.__TAURI_INTERNALS__.invoke = (cmd, args, options) => {
      const s = window.__reloadReset
      if (cmd === 'stop') s.stops++
      if (cmd === 'editor_close') {
        if (s.dirty && !args.discard) throw Error('editor has unsaved changes')
        s.dirty = false; s.closes++
      }
      if (cmd === 'editor_open') {
        if (s.dirty && !args.discardExisting) throw Error('editor has unsaved changes')
        s.opens++
      }
      if (cmd === 'editor_save') s.saves++
      return original(cmd, args, options)
    }
    // Use the exact optimized module URLs already loaded by Vite: importing a
    // second copy without its version query creates different injection symbols.
    const resource = name => performance.getEntriesByType('resource').find(entry => entry.name.includes(`/deps/${name}.js?v=`)).name
    const { createApp } = await import(resource('vue'))
    const { createPinia } = await import(resource('pinia'))
    const { i18n } = await import('/src/i18n/index.ts')
    const { default: App } = await import('/src/App.vue')
    old.unmount()
    pinia._e.stop()
    createApp(App).use(createPinia()).use(i18n).mount('#app')
  })
  try {
    await page.waitForFunction(() => {
      const e = document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia?._s.get('editor')
      return e && window.__reloadReset.closes > 0 && !e.loading
    })
    check(await page.evaluate(() => {
      const pinia = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia
      const e = pinia._s.get('editor'), p = pinia._s.get('player'), s = window.__reloadReset
      return !e.open && !e.state && !e.playing && !e.looping && !e.expanded && p.playingId === null && s.stops > 0 && !s.dirty && s.saves === 0
    }), 'reload left a hidden dirty session or active playback')
    check(await page.getByRole('alertdialog').count() === 0, 'reload reset must not leave an unsaved prompt')
    await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      await e.openSample({ id: 2, name: 'After reload', path: '/mock/Another.wav', ext: 'wav', bpm: 120 })
    })
    check(await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      return e.open && e.sample.id === 2 && !e.state.dirty && window.__reloadReset.opens === 1
    }), 'a new track is still blocked after reset')
    return { passed: ['surviving native session discarded', 'sound stopped', 'fresh editor UI', 'no save/prompt', 'another track opens'] }
  } finally {
    await page.evaluate(() => { window.__TAURI_INTERNALS__.invoke = window.__reloadReset.original })
  }
}
