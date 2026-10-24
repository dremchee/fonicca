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
