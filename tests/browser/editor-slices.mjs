// Development mock integration: delay native analysis and resolve old responses
// inside the same microtask turn, before the 120ms debounce can issue a new read.
export async function checkEditorSlicePreview(page) {
  await page.waitForFunction(() => document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia?._s.get('editor')?.state?.frames > 0)
  await page.evaluate(() => {
    const ipc = window.__TAURI_INTERNALS__
    const original = ipc.invoke
    window.__sliceFixture = { original, requests: [], markerWrites: [] }
    ipc.invoke = (command, args, options) => {
      if (command === 'editor_slices') return new Promise((resolve, reject) => window.__sliceFixture.requests.push({ args, resolve, reject }))
      if (command === 'editor_apply' && args.op.op === 'setMarkers') window.__sliceFixture.markerWrites.push(args.op.markers)
      return original(command, args, options)
    }
  })
  const check = (value, message) => { if (!value) throw new Error(message) }
  const waitForReads = count => page.waitForFunction(count => window.__sliceFixture.requests.length >= count, count)
  const mark = () => page.getByRole('button', { name: /^Mark \d+ regions?$/ })
  try {
    await page.getByRole('button', { name: 'Slice', exact: true }).click()
    await waitForReads(1)
    const noStaleTarget = await page.evaluate(async () => {
      const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      editor.selection = { start: 10000, end: 20000 }
      window.__sliceFixture.requests[0].resolve([{ start: 1000, end: 2000 }])
      for (let i = 0; i < 24; i++) await Promise.resolve()
      const button = [...document.querySelectorAll('button')].find(button => /^Mark \d+ regions?$/.test(button.textContent.trim()))
      return button?.disabled === true && /Mark 0 regions?/.test(button.textContent)
    })
    check(noStaleTarget, 'old analysis appeared in the debounce gap after selection changed')
    await waitForReads(2)
    check(await page.evaluate(() => window.__sliceFixture.requests[1].args.start === 10000 && window.__sliceFixture.requests[1].args.end === 20000), 'new analysis did not capture the changed selection')
    await page.evaluate(() => window.__sliceFixture.requests[1].resolve([{ start: 11000, end: 18000 }]))
    await page.getByRole('button', { name: 'Mark 1 region', exact: true }).waitFor()

    await page.getByText('Grid', { exact: true }).click()
    check(await mark().isDisabled(), 'changing parameters left the old markers actionable')
    await waitForReads(3)
    check(await page.evaluate(() => window.__sliceFixture.requests[2].args.by.mode === 'grid'), 'new analysis retained the old mode')
    await page.keyboard.press('Escape')
    await page.evaluate(async () => {
      window.__sliceFixture.requests[2].resolve([{ start: 5000, end: 6000 }])
      for (let i = 0; i < 24; i++) await Promise.resolve()
    })
    await page.getByRole('button', { name: 'Slice', exact: true }).click()
    check(await mark().isDisabled(), 'closed analysis restored stale results on reopen')
    await waitForReads(4)
    await page.evaluate(() => window.__sliceFixture.requests[3].resolve([{ start: 12000, end: 19000 }]))
    await page.getByRole('button', { name: 'Mark 1 region', exact: true }).click()
    check(await page.evaluate(() => {
      const markers = window.__sliceFixture.markerWrites.at(-1)
      return markers?.some(marker => marker.frame === 12000 && !marker.skip) && !markers.some(marker => marker.frame === 5000 || marker.frame === 1000)
    }), 'placing regions used an outdated result')

    await page.getByRole('button', { name: 'Slice', exact: true }).click()
    await waitForReads(5)
    const noStaleRevision = await page.evaluate(async () => {
      const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      await editor.undo()
      window.__sliceFixture.requests[4].resolve([{ start: 13000, end: 14000 }])
      for (let i = 0; i < 24; i++) await Promise.resolve()
      const button = [...document.querySelectorAll('button')].find(button => /^Mark \d+ regions?$/.test(button.textContent.trim()))
      return button?.disabled === true
    })
    check(noStaleRevision, 'an older editor revision re-enabled marker placement')
    await waitForReads(6)

    // Unmount with another debounce pending. The old component must neither
    // issue a new analysis read nor revive its preview after disposal.
    await page.evaluate(async () => {
      const app = document.querySelector('#app').__vue_app__
      app.config.globalProperties.$pinia._s.get('editor').selection = { start: 20000, end: 30000 }
      app.unmount()
      window.__sliceFixture.requests[5].resolve([{ start: 15000, end: 16000 }])
      for (let i = 0; i < 24; i++) await Promise.resolve()
      window.__sliceFixture.readsAtUnmount = window.__sliceFixture.requests.length
    })
    // A negative lifecycle check needs a bounded window beyond the debounce.
    await page.waitForTimeout(250)
    check(await page.evaluate(() => window.__sliceFixture.requests.length === window.__sliceFixture.readsAtUnmount), 'disposed slicer started another analysis read')
    return { passed: ['selection debounce gap', 'captured selection', 'parameter invalidation', 'close and reopen', 'current markers only', 'editor revision', 'dispose during debounce'] }
  }
  finally {
    await page.evaluate(() => {
      window.__TAURI_INTERNALS__.invoke = window.__sliceFixture.original
      delete window.__sliceFixture
    })
  }
}
