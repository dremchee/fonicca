// Run against a private production build using the real main.ts with browser-only
// mockIPC installed before mount (see docs/refactoring.md). No native user data.
export async function checkStartupLoading(page, baseUrl) {
  const url = new URL(baseUrl)
  if (!['127.0.0.1', 'localhost'].includes(url.hostname) || url.pathname !== '/mock.html')
    throw new Error('Use a local private production mock, never the native application')
  const requests = [], errors = [], checks = []
  const requested = request => { if (/\/assets\/.*\.js(?:\?|$)/.test(request.url())) requests.push(new URL(request.url()).pathname) }
  const errored = error => errors.push(error.message)
  const consoleError = message => { if (message.type() === 'error') errors.push(message.text()) }
  const check = (value, message) => { if (!value) throw new Error(message); checks.push(message) }
  const loaded = name => requests.some(path => path.includes(`/assets/${name}-`))
  // The monitor preference intentionally persists across windows; start each
  // private test document with it closed rather than depending on earlier QA.
  await page.addInitScript(() => localStorage.setItem('fonicca.debug-monitor', 'false'))
  page.on('request', requested)
  page.on('pageerror', errored)
  page.on('console', consoleError)
  await page.route('**/favicon.ico', route => route.fulfill({ status: 204 }))
  let releaseSettings
  const held = new Promise(resolve => { releaseSettings = resolve })
  const holdSettings = async route => { await held; await route.continue() }
  try {
    url.search = '?size=100000'
    url.hash = '#library-large'
    await page.goto(url.href)
    await page.locator('.sample-list .row[data-index="0"]').waitFor()
    check(!['EditorWindow', 'EditorView', 'SettingsDialog', 'DuplicatesView', 'DebugMonitor'].some(loaded), 'library startup excludes unopened feature chunks')
    const libraryRequests = [...requests]

    await page.evaluate(() => {
      const ipc = window.__TAURI_INTERNALS__, original = ipc.invoke
      window.__startupCalls = []
      ipc.invoke = (command, ...args) => { window.__startupCalls.push(command); return original(command, ...args) }
    })
    await page.route('**/assets/SettingsDialog-*.js', holdSettings)
    const settingsRequested = page.waitForRequest(request => /\/assets\/SettingsDialog-.*\.js$/.test(request.url()))
    await page.getByRole('button', { name: 'Settings', exact: true }).click()
    await settingsRequested
    check(await page.getByRole('button', { name: 'Back to library', exact: true }).isVisible(), 'settings navigation opens while its component is loading')
    releaseSettings()
    await page.locator('.embedded-settings').waitFor()
    await page.waitForFunction(() => window.__startupCalls.includes('app_info') && window.__startupCalls.includes('list_hidden'))
    check(loaded('SettingsDialog'), 'first settings open loads its deferred chunk and metadata')
    await page.getByRole('button', { name: 'About', exact: true }).click()
    await page.locator('.embedded-settings').getByText('Version 0.1.0', { exact: true }).waitFor()
    check(true, 'metadata requested during an already-open lazy mount reaches the About screen')
    await page.evaluate(() => { window.__settingsElement = document.querySelector('.embedded-settings') })
    await page.getByRole('button', { name: 'Back to library', exact: true }).click()
    await page.locator('.embedded-settings').waitFor({ state: 'hidden' })
    await page.getByRole('button', { name: 'Settings', exact: true }).click()
    await page.locator('.embedded-settings').waitFor()
    check(await page.evaluate(() => window.__settingsElement === document.querySelector('.embedded-settings')), 'closing and reopening preserves the mounted settings component')
    await page.waitForFunction(() => window.__startupCalls.filter(command => command === 'app_info').length === 2)
    check(true, 'reopening refreshes application metadata')
    await page.getByRole('button', { name: 'Back to library', exact: true }).click()
    await page.locator('.sample-list .row[data-index="0"]').dblclick()
    await page.locator('[data-editor-panel]').waitFor()
    await page.waitForFunction(() => document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')?.state?.frames > 0)
    check(loaded('EditorView'), 'double-click loads the embedded editor and its audio state')
    await page.getByRole('button', { name: /^Duplicates \d/ }).click()
    await page.getByRole('heading', { name: 'Duplicates', exact: true }).waitFor()
    check(loaded('DuplicatesView'), 'duplicates remain available after deferred mounting')
    await page.getByRole('button', { name: 'Performance monitor', exact: true }).click()
    await page.getByText('Debug monitor', { exact: true }).waitFor()
    check(loaded('DebugMonitor'), 'monitor opens from its sidebar control')

    requests.length = 0
    url.search = '?editor=1'
    url.hash = '#editor'
    await page.goto(url.href)
    await page.locator('[data-editor-panel]').waitFor()
    await page.waitForFunction(() => document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')?.state?.frames > 0)
    check(loaded('EditorWindow') && loaded('EditorView') && !loaded('App') && !loaded('SettingsDialog'), `separate editor window loads without library or settings UI: ${requests.join(', ')}`)
    const editorRequests = [...requests]
    await page.evaluate(() => document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('settings').openDialog('waveform'))
    await page.getByRole('dialog').waitFor()
    check(await page.getByRole('tab', { name: 'Editor', exact: true }).getAttribute('data-state') === 'active', 'separate editor settings retain the editor waveform target')
    await page.keyboard.press('Escape')
    await page.getByRole('dialog').waitFor({ state: 'hidden' })
    check(errors.length === 0, `checked flows have no page or console errors: ${errors.join('; ')}`)
    return { checks, libraryRequests, editorRequests, browser: page.context().browser().version() }
  } finally {
    releaseSettings()
    await page.unroute('**/assets/SettingsDialog-*.js', holdSettings)
    await page.unroute('**/favicon.ico')
    page.off('request', requested)
    page.off('pageerror', errored)
    page.off('console', consoleError)
  }
}
