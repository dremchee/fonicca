// Browser-only regression scenario, run against the real production mock UI.
// Receives a Playwright Page; no additional test framework or app internals.
export async function checkLibraryHints(page) {
  const check = (value, message) => { if (!value) throw new Error(message) }
  const viewport = page.locator('.sample-list [aria-label="All samples"][tabindex="0"]')
  await viewport.evaluate(element => element.scrollTop = 0)
  const row = page.locator('.sample-list .row[data-index="0"]')
  await row.waitFor()
  const category = row.locator('span.truncate.text-xs')
  const bpm = row.locator('span.text-right').filter({ hasText: /^120$/ })
  const content = page.locator('[data-library-hint]:visible')
  const tooltip = page.getByRole('tooltip').and(content)
  const hidden = () => tooltip.waitFor({ state: 'hidden' })
  const shown = () => tooltip.waitFor({ state: 'visible' })
  await page.mouse.move(0, 0)
  await category.hover()
  await shown()
  check((await tooltip.textContent()).includes('100%'), 'category confidence hint changed')
  const lines = await content.locator('span.block').allTextContents()
  check(lines.length === 2 && lines[1].startsWith('('), 'the hint scope no longer has its own centered line')
  const id = await tooltip.getAttribute('id')
  check(id, 'visible hint has no ID')
  await page.waitForFunction(hintId => {
    const cell = document.querySelector('.sample-list .row[data-index="0"] span.truncate.text-xs')
    return cell?.getAttribute('aria-describedby')?.split(/\s+/).includes(hintId)
  }, id)

  // The pointer can cross onto the tooltip without it disappearing.
  const box = await content.boundingBox()
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps: 8 })
  await shown()
  await page.keyboard.press('Escape')
  await hidden()
  check(!await category.getAttribute('aria-describedby'), 'description retained after Escape')

  await bpm.hover()
  await shown()
  check((await tooltip.textContent()).includes('audio'), 'BPM provenance hint changed')
  await page.keyboard.press('Escape')
  await hidden()
  const bpmBox = await bpm.boundingBox()
  await page.mouse.move(bpmBox.x + bpmBox.width / 2 + 1, bpmBox.y + bpmBox.height / 2)
  check(await tooltip.count() === 0, 'Escape hint reopened on the same trigger')

  await row.locator('.star').hover()
  await shown()
  check((await content.innerText()).includes('F'), 'favorite shortcut is missing')
  await row.locator('.track-name').hover()
  await hidden()
  check(!await row.locator('.track-name').getAttribute('title'), 'track-name tooltip was restored')

  // Keyboard focus, rather than pointer-driven popup autofocus, opens the hint.
  await row.locator('.track-play').focus()
  await page.keyboard.press('Tab')
  // With default macOS WebKit preferences, Option-Tab includes buttons.
  const tab = await page.evaluate(() => document.activeElement === document.body) ? 'Alt+Tab' : 'Tab'
  if (tab === 'Alt+Tab') {
    await row.locator('.track-play').focus()
    await page.keyboard.press(tab)
  }
  check(await row.locator('.tag-add').evaluate(element => element === document.activeElement), 'tag picker is no longer keyboard reachable')
  await page.keyboard.press(tab)
  await tooltip.getByText(/favorites/).waitFor()
  check(await row.locator('.star').evaluate(element => element === document.activeElement), 'favorite is no longer keyboard reachable')
  check((await tooltip.textContent()).includes('favorites'), 'keyboard hint is wrong')
  await page.keyboard.press('Escape')
  await hidden()

  await category.hover()
  await shown()
  await viewport.evaluate(element => element.scrollTop = 90_000)
  await hidden()
  await viewport.evaluate(element => element.scrollTop = 0)
  await row.waitFor()
  await category.hover()
  await shown()
  await row.locator('.track-name').click({ button: 'right' })
  await hidden()

  // An analysis refresh can change hint data without removing the row.
  await page.mouse.move(0, 0)
  await category.hover()
  await shown()
  await category.evaluate(element => element.dataset.sampleHint = 'Updated analysis')
  await hidden()
  await page.mouse.move(0, 0)
  await category.hover()
  await shown()
  check((await tooltip.textContent()).includes('Updated analysis'), 'hint retained stale analysis')

  // Filtering out an anchored row must also dispose the tooltip and description.
  await page.getByRole('searchbox', { name: 'Search entire library' }).fill('no-such-fixture-sample')
  await row.waitFor({ state: 'detached' })
  await hidden()
  return { passed: ['category', 'scope line break', 'bpm', 'accessible description', 'hoverable content', 'Escape', 'shortcut', 'name stays tooltip-free', 'keyboard focus', 'scroll', 'context menu', 'analysis refresh', 'filter removal'] }
}
