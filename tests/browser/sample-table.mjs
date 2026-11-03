// Exercises column controls against the sparse, server-sorted 32k library fixture.
export async function verifySampleTable(page, baseUrl) {
  const url = new URL(baseUrl)
  if (!['localhost', '127.0.0.1'].includes(url.hostname) || url.pathname !== '/mock.html')
    throw new Error('Use the isolated local browser mock')
  url.searchParams.set('size', '32000')
  url.hash = 'library-large'
  await page.goto(url.href)
  await page.evaluate(() => {
    // Existing users' preferences must survive the switch to TanStack.
    localStorage.setItem('fonicca-list-columns-v1', JSON.stringify({
      directory: false, tags: false, category: true, kind: false,
      bpm: true, key: false, length: true, type: false,
    }))
    localStorage.setItem('fonicca-list-column-widths-v1', JSON.stringify({ name: 360, category: 140 }))
    localStorage.setItem('fonicca-sample-sort-v1', JSON.stringify({ by: 'length', direction: 'desc' }))
  })
  await page.reload()
  const waitForRows = () => page.waitForSelector('[data-index] .track-title')
  const read = () => page.evaluate(() => {
    const library = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('library')
    return {
      columns: [...document.querySelectorAll('.column-heading')].map(node => node.dataset.column),
      widths: JSON.parse(localStorage.getItem('fonicca-list-column-widths-v1')),
      sort: { ...library.sort }, filter: window.__sampleTableFilters?.at(-1), total: library.total,
      rows: document.querySelectorAll('[data-index]').length, pages: library.pages.size,
    }
  })
  const check = (condition, message) => { if (!condition) throw new Error(message) }
  const header = id => page.locator(`.column-heading[data-column="${id}"] button`)
  const columnsMenu = () => page.getByRole('button', { name: 'Columns', exact: true }).click()
  const choose = name => page.getByRole('menuitem', { name, exact: true }).click()
  await waitForRows()
  const migrated = await read()
  check(migrated.widths.name === 360 && migrated.widths.category === 140, 'Existing widths were lost')
  check(migrated.sort.by === 'length' && migrated.sort.direction === 'desc', 'Existing sort was lost')
  check(migrated.columns.join() === 'name,category,bpm,length', 'Existing visibility was lost')
  await page.evaluate(() => {
    const invoke = window.__TAURI_INTERNALS__.invoke
    window.__sampleTableFilters = []
    window.__TAURI_INTERNALS__.invoke = (command, args, ...rest) => {
      if (command === 'list_samples') window.__sampleTableFilters.push(args.filter)
      return invoke(command, args, ...rest)
    }
  })

  await columnsMenu()
  check(await page.getByRole('menuitemcheckbox', { name: 'Name always shown' }).isDisabled(), 'Name must remain visible')
  await page.getByRole('menuitemcheckbox', { name: 'Tags', exact: true }).click()
  await page.getByRole('menuitemcheckbox', { name: 'Category', exact: true }).click()
  check(await page.getByRole('menu').isVisible(), 'Column menu should stay open for multiple changes')
  await page.keyboard.press('Escape')
  check((await read()).columns.join() === 'name,tags,bpm,length', 'Visible columns did not update')

  await header('bpm').click()
  await choose('Descending')
  await page.waitForFunction(() => window.__sampleTableFilters?.some(filter => filter.sortBy === 'bpm' && filter.sortDirection === 'desc'))
  await header('bpm').click()
  await choose('Descending')
  const sorted = await read()
  check(sorted.filter.sortBy === 'bpm' && sorted.filter.sortDirection === 'desc', 'Menu sort did not reach the backend filter')
  await page.reload()
  await waitForRows()
  const restored = await read()
  check(restored.columns.join() === 'name,tags,bpm,length' && restored.sort.by === 'bpm' && restored.sort.direction === 'desc', 'Visibility/sorting did not survive reload')

  const handle = page.getByRole('separator', { name: 'Width of Name column', exact: true })
  const bounds = await handle.boundingBox()
  await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2)
  await page.mouse.down()
  await page.mouse.move(bounds.x + bounds.width / 2 + 50, bounds.y + bounds.height / 2, { steps: 5 })
  await page.mouse.up()
  await page.waitForFunction(() => JSON.parse(localStorage.getItem('fonicca-list-column-widths-v1')).name === 410)
  await page.reload()
  await waitForRows()
  check((await read()).widths.name === 410, 'Resized width did not survive reload')

  await header('bpm').click()
  await choose('Hide column')
  check(!await header('bpm').count(), 'Header hide action failed')
  await columnsMenu()
  check(await page.getByRole('menuitemcheckbox', { name: 'BPM', exact: true }).getAttribute('aria-checked') === 'false', 'Visibility menu is out of sync with the header')
  await page.getByRole('menuitemcheckbox', { name: 'BPM', exact: true }).click()
  await page.keyboard.press('Escape')
  check((await header('bpm').getAttribute('aria-label')).includes('Sorted descending'), 'Showing a hidden sorted column lost its indicator')

  await page.evaluate(() => {
    const library = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('library')
    library.findSimilar(library.sampleAt(0))
  })
  await header('bpm').click()
  check(!await page.getByRole('menuitem', { name: 'Ascending', exact: true }).count(), 'Similarity order must not offer column sorting')
  await page.keyboard.press('Escape')
  await page.evaluate(() => document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('library').exitSimilar())
  check((await header('bpm').getAttribute('aria-label')).includes('Sorted descending'), 'Leaving similarity lost the saved sorting')

  await columnsMenu()
  await choose('Reset column widths')
  check(Object.keys((await read()).widths).length === 0, 'Width reset failed')
  await columnsMenu()
  await choose('Reset to default')
  const reset = await read()
  check(reset.columns.join() === 'name,tags,category,bpm,length', 'Default view reset failed')
  check(reset.total === 32000 && reset.rows < 100 && reset.pages <= 12, 'Column controls materialized the large library')
  return { migrated, restored, reset, checks: ['migration', 'visibility', 'server sorting', 'reload', 'resize', 'hide/show', 'similarity', 'reset', 'bounded rows'] }
}
