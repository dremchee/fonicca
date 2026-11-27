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
  assert.equal(audioExportFormat('/tmp/Track.FLAC'), 'flac')
  assert.equal(audioExportFormat('/tmp/Track.aifc'), 'aiff')
  assert.equal(audioExportFormat('/tmp/Track.mp4'), 'm4a')
  assert.equal(audioExportFormat('/tmp/Track.oga'), 'ogg')
  assert.equal(audioExportFormat('/tmp/Track.caf'), 'caf')
  assert.equal(audioExportFormat('/tmp/Track'), 'wav')
  assert.equal(audioExportPath('/tmp/Track.FLAC', 'aiff'), '/tmp/Track.aiff')
  assert.equal(audioExportPath('/tmp/Track.aifc', 'm4a'), '/tmp/Track.m4a')
  assert.equal(audioExportPath('C:\\Audio.FLAC\\Track.WAVE', 'caf'), 'C:\\Audio.FLAC\\Track.caf')
  assert.equal(audioExportPath('C:\\Audio.FLAC\\Track', 'caf'), 'C:\\Audio.FLAC\\Track.caf')
})

test('MP3 rate selection cannot promise unsupported high sample rates', () => {
  assert.equal(supportedExportRate('wav', null, 96000), null)
  assert.equal(supportedExportRate('ogg', 96000, 44100), 96000)
  assert.equal(supportedExportRate('mp3', null, 96000), 48000)
  assert.equal(supportedExportRate('mp3', 88200, 44100), 48000)
  assert.equal(supportedExportRate('mp3', null, 44100), null)
  assert.equal(supportedExportRate('mp3', 22050, 48000), 22050)
})
