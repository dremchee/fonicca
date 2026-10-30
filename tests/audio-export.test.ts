import assert from 'node:assert/strict'
import { test } from 'node:test'
import { audioExportFormat, audioExportPath, supportedExportRate } from '../src/audio-export.ts'

test('export uses the selected codec extension, never WAV data disguised as MP3', () => {
  assert.equal(audioExportPath('/tmp/Track.wav', 'mp3'), '/tmp/Track.mp3')
  assert.equal(audioExportPath('/tmp/Track.MP3', 'ogg'), '/tmp/Track.ogg')
  assert.equal(audioExportPath('/tmp/Track', 'wav'), '/tmp/Track.wav')
  assert.equal(audioExportPath('/tmp/Track.v2', 'mp3'), '/tmp/Track.v2.mp3')
  assert.equal(audioExportPath('/tmp/ogg.folder/Track.ogg', 'wav'), '/tmp/ogg.folder/Track.wav')
  assert.equal(audioExportFormat('/tmp/Track.MP3'), 'mp3')
  assert.equal(audioExportFormat('/tmp/Track.flac'), 'wav')
})

test('MP3 rate selection cannot promise unsupported high sample rates', () => {
  assert.equal(supportedExportRate('wav', null, 96000), null)
  assert.equal(supportedExportRate('ogg', 96000, 44100), 96000)
  assert.equal(supportedExportRate('mp3', null, 96000), 48000)
  assert.equal(supportedExportRate('mp3', 88200, 44100), 48000)
  assert.equal(supportedExportRate('mp3', null, 44100), null)
  assert.equal(supportedExportRate('mp3', 22050, 48000), 22050)
})
