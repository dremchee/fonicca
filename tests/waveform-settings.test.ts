import assert from 'node:assert/strict'
import test from 'node:test'
import { waveformLook, waveformPreset, waveformSettings, WAVEFORM_PRESETS } from '../src/waveform-settings.ts'
import { waveLevel } from '../src/waveform.ts'

test('old profiles retain their drawing choices and gain appearance defaults', () => {
  const options = waveformSettings({ waveStyle: 'bars', waveScale: 'db', waveChannels: 'mix' })
  assert.equal(options.waveStyle, 'bars')
  assert.equal(options.waveScale, 'db')
  assert.equal(options.waveDbRange, 60)
  assert.equal(options.waveDensity, 'off')
  assert.deepEqual(options.waveChannelColors, [null, null])
})

test('applying and editing a preset does not change another profile or preset', () => {
  const preview = waveformSettings(WAVEFORM_PRESETS.editing)
  const editor = waveformSettings(WAVEFORM_PRESETS.music)
  editor.waveChannelColors[0] = '#ff0000'
  assert.equal(waveformPreset(preview), 'editing')
  assert.equal(waveformPreset(editor), 'custom')
  assert.equal(preview.waveChannelColors[0], null)
  assert.equal(WAVEFORM_PRESETS.music.waveChannelColors[0], null)
  assert.equal(waveformPreset(waveformSettings(WAVEFORM_PRESETS.detail)), 'detail')
})

test('the dB range exposes quieter material without changing the audio level', () => {
  const level = 10 ** (-60 / 20)
  assert.ok(waveLevel(level, { scale: 'db', zoom: 1, dbRange: 40 }) === 0)
  assert.ok(Math.abs(waveLevel(level, { scale: 'db', zoom: 1, dbRange: 90 }) - 1 / 3) < 0.000001)
  assert.equal(waveLevel(1, { scale: 'db', zoom: 1, dbRange: 90 }), 1)
  assert.equal(waveLevel(level, { scale: 'linear', zoom: 1, dbRange: 90 }), level)
  const look = waveformLook(waveformSettings(WAVEFORM_PRESETS.detail))
  assert.equal(look.density, 'laplace')
  assert.equal(look.dbRange, 90)
})
