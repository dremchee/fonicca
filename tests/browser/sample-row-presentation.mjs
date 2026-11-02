// Deterministic grouped/playing/paused/pinned rows, isolated metadata and SVGs.
export async function captureSampleRowPresentation(page, baseUrl, outputPrefix) {
  const url = new URL(baseUrl)
  if (!['localhost', '127.0.0.1'].includes(url.hostname) || url.pathname !== '/mock.html') throw new Error('Use the local browser mock')
  url.searchParams.set('size', '32000')
  url.hash = 'library-large'
  await page.goto(url.href)
  await page.reload()
  await page.waitForFunction(() => document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia?._s.get('library')?.sampleAt(0))
  await page.evaluate(async () => {
    const p = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia
    const l = p._s.get('library'), player = p._s.get('player')
    const rows = l.pages.get(0).slice()
    rows[0] = { ...rows[0], name: 'Parent track', stemCount: 2, favorite: true }
    rows[1] = { ...rows[1], name: 'Voice stem', stemParentId: rows[0].id, stemKind: 'vocals', category: 'vocal' }
    rows[2] = { ...rows[2], name: 'Instrumental stem', stemParentId: rows[0].id, stemKind: 'instrumental' }
    l.pages.set(0, rows)
    l.expandedStems.add(rows[0].id)
    l.selectedIds = new Set([rows[0].id])
    l.focusIndex = 0
    player.playingId = null
    player.paused = false
    const { useWaveformBackgrounds } = await import('/src/composables/waveformBackgrounds.ts')
    const values = new Float32Array(256 * 6)
    for (let i = 0; i < 256; i++) { const a = .15 + .7 * Math.abs(Math.sin(i / 13)); values[i * 6] = -a; values[i * 6 + 1] = a }
    for (const sample of rows.slice(0, 3)) useWaveformBackgrounds().remember(sample, { count: 256, lanes: 1, samples: false, stride: 6, values })
  })
  await page.mouse.move(1279, 899)
  const snapshot = async name => {
    await page.waitForTimeout(220)
    const state = await page.evaluate(() => {
      const list = document.querySelector('.sample-list'), top = list.getBoundingClientRect().top
      return [...list.querySelectorAll('.row[data-index]')].filter(row => row.getClientRects().length && (Number(row.dataset.index) < 3 || row.dataset.pinned)).map(row => {
        const r = row.getBoundingClientRect(), s = getComputedStyle(row)
        const title = row.querySelector('.track-title'), icon = row.querySelector('.track-icon')
        const tr = title.getBoundingClientRect(), ir = icon.getBoundingClientRect()
        return { index: Number(row.dataset.index), classes: [...row.classList].sort(), pinned: !!row.dataset.pinned,
          rect: [r.x, r.y - top, r.width, r.height].map(n => Math.round(n * 100) / 100),
          title: title.textContent, titleX: tr.x, iconX: ir.x,
          background: s.backgroundColor, color: s.color, radius: s.borderRadius, grid: s.gridTemplateColumns,
          stemBadges: row.querySelectorAll('.stem-toggle').length,
          starFill: getComputedStyle(row.querySelector('.star svg')).fill }
      })
    })
    if (outputPrefix) await page.locator('.sample-list').screenshot({ path: `${outputPrefix}-${name}.png` })
    return { name, rows: state }
  }
  const results = [await snapshot('group')]
  await page.evaluate(() => {
    const p = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia, l = p._s.get('library'), player = p._s.get('player')
    const sample = l.sampleAt(1)
    l.selectedIds = new Set([sample.id]); l.focusIndex = 1
    player.playingId = sample.id; player.paused = false; player.position = .2
  })
  results.push(await snapshot('playing'))
  await page.evaluate(() => { document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('player').paused = true })
  results.push(await snapshot('paused'))
  await page.evaluate(() => {
    const p = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia, l = p._s.get('library'), player = p._s.get('player')
    l.selectedIds = new Set([l.sampleAt(0).id]); l.focusIndex = 0
    player.playingId = l.sampleAt(0).id; player.paused = false
  })
  await page.waitForTimeout(30)
  await page.evaluate(() => { document.querySelector('.sample-list div[tabindex="0"].overflow-y-auto').scrollTop = 200 })
  await page.waitForSelector('[data-pinned="true"]')
  results.push(await snapshot('pinned'))
  if (!results.at(-1).rows.some(row => row.pinned && row.stemBadges === 0)) throw new Error('Pinned row must hide stem badges')
  return results
}
