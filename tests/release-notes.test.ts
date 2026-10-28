import assert from 'node:assert/strict'
import { test } from 'node:test'
import { parseReleaseNotes } from '../src/release-notes.ts'

test('generated updater notes become a clean list without redundant heading or commit hashes', () => {
  assert.deepEqual(parseReleaseNotes('## Changes\r\n\r\n- Fix macOS signature (648fc4f)\r\n- Add release command (00d34b8)'), [
    { kind: 'list', items: ['Fix macOS signature', 'Add release command'] },
  ])
})

test('notes preserve section hierarchy, paragraphs and meaningful parentheses', () => {
  assert.deepEqual(parseReleaseNotes('A new update\nfor Fonicca.\n\n## Improvements\n- **macOS:** better startup (Apple Silicon)\n\n## Fixes\n1. See [details](https://example.test) and `Save`.'), [
    { kind: 'paragraph', text: 'A new update for Fonicca.' },
    { kind: 'heading', text: 'Improvements' },
    { kind: 'list', items: ['macOS: better startup (Apple Silicon)'] },
    { kind: 'heading', text: 'Fixes' },
    { kind: 'list', items: ['See details and Save.'] },
  ])
})

test('external HTML is kept as inert text, and empty notes have no blocks', () => {
  assert.deepEqual(parseReleaseNotes(''), [])
  assert.deepEqual(parseReleaseNotes('<img src=x onerror=alert(1)>'), [{ kind: 'paragraph', text: '<img src=x onerror=alert(1)>' }])
})
