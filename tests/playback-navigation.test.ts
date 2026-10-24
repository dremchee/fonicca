import assert from 'node:assert/strict'
import { test } from 'node:test'
import { playbackSeekTarget } from '../src/playback-navigation.ts'

test('keyboard seeking is precise for one-shots and faster for long tracks', () => {
  assert.equal(playbackSeekTarget('ArrowRight', 0.25, 0.8), 0.26)
  assert.equal(playbackSeekTarget('ArrowLeft', 0.25, 0.8, true), 0.15)
  assert.equal(playbackSeekTarget('ArrowRight', 10, 180), 11)
  assert.equal(playbackSeekTarget('ArrowLeft', 10, 180, true), 5)
})

test('seeking clamps at both ends and supports fractional durations', () => {
  assert.equal(playbackSeekTarget('ArrowLeft', 0, 187.12345), 0)
  assert.equal(playbackSeekTarget('ArrowRight', 187, 187.12345), 187.12345)
  assert.equal(playbackSeekTarget('End', 10, 187.12345), 187.12345)
  assert.equal(playbackSeekTarget('Home', 10, 187.12345), 0)
  assert.equal(playbackSeekTarget('PageUp', 20, 180), 38)
  assert.equal(playbackSeekTarget('PageDown', 20, 180), 2)
})

test('unknown duration and unrelated keys do not trigger playback', () => {
  for (const duration of [0, -1, NaN, Infinity])
    assert.equal(playbackSeekTarget('ArrowRight', 0, duration), null)
  assert.equal(playbackSeekTarget('ArrowRight', NaN, 180), null)
  assert.equal(playbackSeekTarget('Tab', 10, 180), null)
})
