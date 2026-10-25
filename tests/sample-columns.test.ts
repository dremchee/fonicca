import assert from 'node:assert/strict'
import { test } from 'node:test'
import { boundedSampleColumnWidth, sampleTableLayout, SAMPLE_COLUMNS } from '../src/sample-columns.ts'

test('column resizing stays readable and rejects invalid stored sizes', () => {
  assert.equal(boundedSampleColumnWidth('name', 1), 160)
  assert.equal(boundedSampleColumnWidth('name', 10000), 2400)
  assert.equal(boundedSampleColumnWidth('category', -10), 64)
  assert.equal(boundedSampleColumnWidth('bpm', NaN), 56)
  assert.equal(boundedSampleColumnWidth('tags', Infinity), 120)
  assert.equal(boundedSampleColumnWidth('length', 88.4), 88)
})

test('resizing the name keeps its exact width while the trailing area fills free space', () => {
  const columns = SAMPLE_COLUMNS.filter(c => c.defaultVisible)
  const automatic = sampleTableLayout(columns, {})
  const manual = sampleTableLayout(columns, { name: 360, category: 140 })
  assert.match(automatic['--sample-columns'], /minmax\(220px, 1fr\)/)
  assert.equal(manual['--sample-columns'], '16px 360px 140px 56px 64px minmax(28px, 1fr)')
  assert.equal(manual.minWidth, '748px')
})

test('hidden columns reserve no space and recover their stored width when shown again', () => {
  const widths = { tags: 200 }
  const visible = sampleTableLayout(SAMPLE_COLUMNS, widths)
  const hidden = sampleTableLayout(SAMPLE_COLUMNS.filter(c => c.id !== 'tags'), widths)
  assert.equal(parseInt(visible.minWidth) - parseInt(hidden.minWidth), 212)
  assert.equal(sampleTableLayout(SAMPLE_COLUMNS, widths)['--sample-columns'], visible['--sample-columns'])
  assert.match(sampleTableLayout([], { name: NaN })['--sample-columns'], /minmax\(220px, 1fr\)/)
})

test('saved wide columns fit a narrower viewport without changing preferred widths', () => {
  const columns = SAMPLE_COLUMNS.filter(c => c.defaultVisible)
  const widths = { name: 620 }
  const fitted = sampleTableLayout(columns, widths, 875)
  assert.equal(fitted.minWidth, '875px')
  assert.equal(fitted['--sample-columns'], '16px 531px 96px 56px 64px minmax(28px, 1fr)')
  assert.equal(widths.name, 620)
  assert.match(sampleTableLayout(columns, widths, 1200)['--sample-columns'], /620px/)
})

test('oversized metadata columns shrink to their limits before horizontal scrolling', () => {
  const widths = { name: 800, category: 600, bpm: 400, length: 300 }
  const columns = SAMPLE_COLUMNS.filter(c => c.defaultVisible)
  const fitted = sampleTableLayout(columns, widths, 600)
  assert.equal(fitted.minWidth, '600px')
  assert.match(fitted['--sample-columns'], /^16px 160px /)
  const tiny = sampleTableLayout(columns, widths, 200)
  assert.equal(tiny.minWidth, '440px')
  assert.equal(tiny['--sample-columns'], '16px 160px 64px 44px 44px minmax(28px, 1fr)')
})
