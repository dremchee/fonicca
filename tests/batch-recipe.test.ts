import assert from 'node:assert/strict'
import { test } from 'node:test'
import { batchDefaults, batchRecipe, restoreBatchForm, validBatchForm } from '../src/batch-recipe.ts'

test('legacy peak presets keep their level and do not enable new operations', () => {
  const old = restoreBatchForm({ normalize: true, peakDb: -6, trim: false, template: '{name}' })
  assert.equal(old.normalizeMode, 'peak')
  assert.equal(old.dcOffset, false)
  assert.equal(batchRecipe(old, false).normalize, -6)
  assert.equal(batchRecipe(old, false).lufs, null)
  assert.equal(old.targetLufs, -14)
})

test('LUFS and DC controls produce a single normalization mode', () => {
  const form = { ...batchDefaults, normalize: true, normalizeMode: 'lufs' as const, dcOffset: true, targetLufs: -18, ceilingDb: -1 }
  const recipe = batchRecipe(form, false)
  assert.equal(recipe.normalize, null)
  assert.equal(recipe.dcOffset, true)
  assert.deepEqual(recipe.lufs, { target: -18, ceiling: -1 })
  const restored = restoreBatchForm(JSON.parse(JSON.stringify(form)))
  assert.deepEqual(batchRecipe(restored, false), recipe)
  const disabled = batchRecipe({ ...form, normalize: false }, false)
  assert.equal(disabled.lufs, null)
  assert.equal(disabled.normalize, null)
})

test('invalid LUFS values disable running while disabled controls are ignored', () => {
  const form = { ...batchDefaults, normalize: true, normalizeMode: 'lufs' as const }
  assert.equal(validBatchForm(form), true)
  for (const targetLufs of [NaN, Infinity, -37, -4, '' as unknown as number])
    assert.equal(validBatchForm({ ...form, targetLufs }), false)
  for (const ceilingDb of [NaN, -13, 1, '' as unknown as number])
    assert.equal(validBatchForm({ ...form, ceilingDb }), false)
  assert.equal(validBatchForm({ ...form, targetLufs: NaN, normalize: false }), true)
})

test('new normalization controls preserve format, fades and in-place naming rules', () => {
  const form = { ...batchDefaults, normalize: true, normalizeMode: 'lufs' as const, format: true, channels: '1', sampleRate: '44100', bitDepth: '24', fades: true, rename: true }
  const recipe = batchRecipe(form, true)
  assert.equal(recipe.rename, null)
  assert.equal(recipe.channels, 1)
  assert.equal(recipe.sampleRate, 44100)
  assert.equal(recipe.bitDepth, 24)
  assert.equal(recipe.fadeOutMs, 10)
})
