import assert from 'node:assert/strict'
import { test } from 'node:test'
import { clipEnd, emptyProject, moveProjectClip, parseProject, projectDuration, snapProjectTime } from '../src/project.ts'
import type { ProjectClip } from '../src/project.ts'
const clip = (id = 'clip'): ProjectClip => ({ id, name: 'Kick', path: '/sounds/kick.wav', sourceDuration: 2, start: 1, offset: 0.25, duration: 0.5, repeats: 4, fadeIn: 0.01, fadeOut: 0.02 })
function project() {
  const p = emptyProject('Loop')
  p.tracks = [
    { id: 'a', name: 'Drums', gainDb: -3, pan: 0, mute: false, solo: false, clips: [clip()] },
    { id: 'b', name: 'Layer', gainDb: 0, pan: 0.5, mute: true, solo: false, clips: [] },
  ]
  return p
}
test('project files round-trip trims, fades, repeats and mixer controls', () => {
  const p = project()
  assert.deepEqual(parseProject(JSON.parse(JSON.stringify(p))), p)
  assert.equal(clipEnd(p.tracks[0]!.clips[0]!), 3)
  assert.equal(projectDuration(p), 3)
  assert.equal(projectDuration(emptyProject('Empty')), 0)
})
test('project parser rejects invalid source ranges, IDs and non-finite values', () => {
  for (const mutate of [
    (p: ReturnType<typeof project>) => p.tracks[0]!.clips[0]!.offset = 2,
    (p: ReturnType<typeof project>) => p.tracks[0]!.clips[0]!.repeats = 1.5,
    (p: ReturnType<typeof project>) => p.tracks[0]!.clips[0]!.start = 600,
    (p: ReturnType<typeof project>) => p.tracks[0]!.clips[0]!.fadeIn = 1,
    (p: ReturnType<typeof project>) => p.tracks[1]!.id = 'a',
    (p: ReturnType<typeof project>) => p.masterDb = Number.NaN,
    (p: ReturnType<typeof project>) => p.bpm = 0,
  ]) {
    const p = project(); mutate(p); assert.equal(parseProject(p), null)
  }
  assert.equal(parseProject(null), null)
  assert.equal(parseProject({ version: 2 }), null)
})
test('moving between tracks preserves the source trim and leaves undo snapshots intact', () => {
  const p = project(), moved = moveProjectClip(p, 'clip', 'b', 5)
  assert.equal(p.tracks[0]!.clips.length, 1)
  assert.equal(moved.tracks[0]!.clips.length, 0)
  assert.deepEqual(moved.tracks[1]!.clips[0], { ...clip(), start: 5 })
  assert.ok(parseProject(moved))
  assert.equal(moveProjectClip(p, 'clip', 'b', 999).tracks[1]!.clips[0]!.start, 598)
  assert.equal(moveProjectClip(p, 'clip', 'a', -3).tracks[0]!.clips[0]!.start, 0)
})
test('grid snapping uses sixteenths and supports free movement', () => {
  assert.equal(snapProjectTime(0.19, 120, true), 0.25)
  assert.equal(snapProjectTime(0.19, 60, true), 0.25)
  assert.equal(snapProjectTime(0.19, 120, false), 0.19)
  assert.equal(snapProjectTime(-2, 120, true), 0)
  assert.equal(snapProjectTime(999, 120, false), 600)
})
