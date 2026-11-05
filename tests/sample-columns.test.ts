import assert from 'node:assert/strict'
import { test } from 'node:test'
import { boundedSampleColumnWidth, sampleColumnResizeMax, sampleTableLayout, SAMPLE_COLUMNS } from '../src/sample-columns.ts'

test('column resizing stays readable and rejects invalid stored sizes', () => {
  assert.equal(boundedSampleColumnWidth('name', 1), 160)
  assert.equal(boundedSampleColumnWidth('name', 10000), 2400)
  assert.equal(boundedSampleColumnWidth('category', -10), 64)
  assert.equal(boundedSampleColumnWidth('bpm', NaN), 56)
  assert.equal(boundedSampleColumnWidth('tags', Infinity), 120)
  assert.equal(boundedSampleColumnWidth('length', 88.4), 88)
})

test('the final column fills free space after the name is resized', () => {
  const columns = SAMPLE_COLUMNS.filter(c => c.defaultVisible)
  const automatic = sampleTableLayout(columns, {})
  const manual = sampleTableLayout(columns, { name: 360, category: 140 })
  assert.match(automatic['--sample-columns'], /minmax\(220px, 1fr\)/)
  assert.equal(manual['--sample-columns'], '16px 360px 120px 140px 56px minmax(64px, 1fr) 28px')
  assert.equal(manual.minWidth, '880px')
})

test('hidden columns reserve no space and recover their stored width when shown again', () => {
  const widths = { tags: 200 }
  const visible = sampleTableLayout(SAMPLE_COLUMNS, widths)
  const hidden = sampleTableLayout(SAMPLE_COLUMNS.filter(c => c.id !== 'tags'), widths)
  assert.equal(parseInt(visible.minWidth) - parseInt(hidden.minWidth), 212)
  assert.equal(sampleTableLayout(SAMPLE_COLUMNS, widths)['--sample-columns'], visible['--sample-columns'])
  assert.match(sampleTableLayout([], { name: NaN })['--sample-columns'], /minmax\(220px, 1fr\)/)
})

test('saved wide columns fit the window while reserving the final column', () => {
  const columns = SAMPLE_COLUMNS.filter(c => c.defaultVisible)
  const widths = { name: 620 }
  const fitted = sampleTableLayout(columns, widths, 875)
  assert.equal(fitted.minWidth, '875px')
  assert.equal(fitted['--sample-columns'], '16px 499px 64px 64px 44px minmax(64px, 1fr) 28px')
  assert.equal(widths.name, 620)
  assert.match(sampleTableLayout(columns, widths, 1200)['--sample-columns'], /620px/)
})

test('narrow windows keep the final column visible even with oversized preferences', () => {
  const widths = { name: 800, category: 600, bpm: 400, length: 300 }
  const columns = SAMPLE_COLUMNS.filter(c => c.defaultVisible)
  const fitted = sampleTableLayout(columns, widths, 600)
  assert.equal(fitted.minWidth, '600px')
  assert.equal(fitted.widths.length, 64)
  const tiny = sampleTableLayout(columns, {}, 420)
  assert.equal(tiny.minWidth, '420px')
  assert.equal(tiny['--sample-columns'], '16px minmax(80px, 1fr) 50px 50px 36px 64px 28px')
})

test('dragging reserves room for the final column and stops at the available width', () => {
  const columns = SAMPLE_COLUMNS.filter(c => c.defaultVisible)
  assert.equal(sampleColumnResizeMax('name', columns, 700), 324)
  const dragged = sampleTableLayout(columns, { name: 1000 }, 700, 'name')
  assert.equal(dragged.widths.name, 324)
  assert.equal(dragged.widths.length, 64)
  assert.equal(dragged.minWidth, '700px')
  assert.equal(sampleTableLayout(columns, dragged.widths, 700)['--sample-columns'], dragged['--sample-columns'])
})

test('the last visible column uses its automatic width after visibility changes', () => {
  const columns = SAMPLE_COLUMNS.filter(c => c.defaultVisible && c.id !== 'length')
  const layout = sampleTableLayout(columns, { name: 240, bpm: 400 }, 700)
  assert.equal(layout.widths.bpm, 56)
  assert.match(layout['--sample-columns'], /minmax\(56px, 1fr\) 28px$/)
})

test('many visible columns stay within the window and keep the last track intact', () => {
  const widths = Object.fromEntries(['name', ...SAMPLE_COLUMNS.map(c => c.id)].map(id => [id, 800]))
  for (const width of [320, 420, 520, 700, 1200]) {
    const layout = sampleTableLayout(SAMPLE_COLUMNS, widths, width)
    assert.ok(parseInt(layout.minWidth) <= width)
    assert.ok(layout.widths.name! >= 80)
    assert.equal(layout.widths.type, 44)
  }
})

test('saved sort preferences accept known columns and fall back safely', async () => {
  const { normalizeSampleSort } = await import('../src/sample-columns.ts')
  assert.deepEqual(normalizeSampleSort({ by: 'bpm', direction: 'desc' }), { by: 'bpm', direction: 'desc' })
  assert.deepEqual(normalizeSampleSort({ by: 'missing', direction: 'invalid' }), { by: 'name', direction: 'asc' })
  assert.deepEqual(normalizeSampleSort(null), { by: 'name', direction: 'asc' })
})

test('directory labels exclude the filename on POSIX and Windows paths', async () => {
  const { sampleDirectory } = await import('../src/sample-columns.ts')
  assert.equal(sampleDirectory('/Library/Drums/Kick.wav'), '/Library/Drums')
  assert.equal(sampleDirectory('/Kick.wav'), '/')
  assert.equal(sampleDirectory(String.raw`C:\Samples\Kick.wav`), String.raw`C:\Samples`)
  assert.equal(sampleDirectory('Kick.wav'), '')
})

test('directory column shows paths relative to the imported folder', async () => {
  const { relativeSampleDirectory } = await import('../src/sample-columns.ts')
  assert.equal(relativeSampleDirectory('/Music/VK/1970-е/Track.mp3', '/Music/VK'), '1970-е')
  assert.equal(relativeSampleDirectory('/Music/VK/Track.mp3', '/Music/VK/'), 'VK')
  assert.equal(relativeSampleDirectory('/Music/VK2/Albums/Track.mp3', '/Music/VK'), 'Albums')
  assert.equal(relativeSampleDirectory(String.raw`C:\Samples\Drums\Kick.wav`, String.raw`C:\Samples`), 'Drums')
})
