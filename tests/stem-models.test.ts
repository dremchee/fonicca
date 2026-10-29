import assert from 'node:assert/strict'
import { test } from 'node:test'
import { modelsForMode, stemModelBytes, stemNames } from '../src/stem-models.ts'

test('two-track mode offers only specialized vocal models', () => {
  assert.deepEqual(modelsForMode('two'), ['melRoformer', 'bsRoformer', 'mdx23c'])
  assert.deepEqual(stemNames('two'), ['vocals', 'instrumental'])
  for (const model of modelsForMode('two')) assert.ok(stemModelBytes[model] > 0)
})
test('four-track mode excludes vocal-only models and retains existing outputs', () => {
  assert.deepEqual(modelsForMode('four'), ['scnetXlIhf', 'scnetLarge', 'htdemucs', 'spleeter'])
  assert.deepEqual(stemNames('four'), ['vocals', 'drums', 'bass', 'other'])
})
