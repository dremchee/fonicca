// Run against mock.html?slow-pause#header-player with Playwright.
export async function checkMiniPlayer(page) {
  const require = (condition, message) => { if (!condition) throw Error(message) }
  await page.getByRole('button', { name: 'Play', exact: true }).first().click()
  const panel = page.getByRole('group', { name: 'Mini player' })
  const tooltip = page.locator('[role="tooltip"][data-open]')
  const next = panel.getByRole('button', { name: 'Next track' })
  await next.hover()
  const nextHint = tooltip.filter({ hasText: 'Next track' })
  await nextHint.waitFor()
  const bubble = await nextHint.boundingBox()
  const arrow = await nextHint.locator('[data-slot="tooltip-arrow"]').boundingBox()
  require(arrow.y + arrow.height > bubble.y + bubble.height, 'tooltip arrow stays inside the label')

  const info = panel.getByRole('button', { name: 'Track info' })
  await info.hover()
  const infoHint = tooltip.filter({ hasText: 'Track info' })
  await infoHint.waitFor()
  require(!await info.getAttribute('title'), 'track info still has a native tooltip')
  await info.click()
  const edit = page.getByRole('button', { name: 'Edit', exact: true })
  await edit.waitFor()
  await infoHint.waitFor({ state: 'hidden' })
  await page.keyboard.press('Escape')
  await edit.waitFor({ state: 'hidden' })

  const volume = panel.getByRole('slider', { name: 'Volume' })
  await volume.press('ArrowLeft')
  const previousVolume = await volume.inputValue()
  require(+previousVolume > 0 && +previousVolume < 100, 'volume slider did not change')
  const mute = panel.getByRole('button', { name: 'Mute', exact: true })
  await mute.click()
  const unmute = panel.getByRole('button', { name: 'Unmute', exact: true })
  await unmute.locator('svg.lucide-volume-x').waitFor()
  require(await volume.inputValue() === '0', 'mute did not set the volume to zero')
  await unmute.press('Space')
  await mute.locator('svg.lucide-volume-2').waitFor()
  require(await volume.inputValue() === previousVolume, 'unmute did not restore the previous volume')

  const spectrum = panel.locator('.spectrum-bars')
  await page.waitForFunction(() => [...document.querySelectorAll('.spectrum-bars rect')].some(bar => +bar.getAttribute('height') > 2))
  const layout = () => panel.evaluate(element => [
    element, ...element.querySelectorAll('.now-playing-cover, .playback-spectrum, .playback-progress, button'),
  ].map(node => {
    const { x, y, width, height } = node.getBoundingClientRect()
    return [x, y, width, height]
  }))
  const heightOfBars = () => spectrum.locator('rect').evaluateAll(bars => bars.map(bar => bar.getAttribute('height')).join(','))
  const beforePause = JSON.stringify(await layout())
  await panel.getByRole('button', { name: /^Pause/ }).click()
  await spectrum.and(panel.locator('[data-state="paused"]')).waitFor()
  const frozen = await heightOfBars()
  require(JSON.stringify(await layout()) === beforePause, 'the player layout shifted at pause')
  await panel.getByRole('button', { name: /^Play/ }).and(panel.locator('[aria-disabled="false"], :not([aria-disabled])')).waitFor()
  require(JSON.stringify(await layout()) === beforePause, 'the player layout shifted after pause completed')
  require(await heightOfBars() === frozen, 'the spectrum did not hold its paused frame')
  return { passed: ['tooltip arrow', 'track info tooltip and card', 'mute and unmute', 'pause layout and spectrum'] }
}
