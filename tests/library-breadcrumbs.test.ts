import assert from 'node:assert/strict'
import { test } from 'node:test'
import { sourceBreadcrumbs } from '../src/library-breadcrumbs.ts'

test('folder breadcrumbs stay inside the imported source and navigate each parent', () => {
  const trail = sourceBreadcrumbs({ id: 7, path: '/Users/music/VK Download/' }, '/Users/music/VK Download/1970-е/Soul')
  assert.deepEqual(trail.map(crumb => crumb.label), ['VK Download', '1970-е', 'Soul'])
  assert.deepEqual(trail.map(crumb => crumb.scope), [
    { kind: 'source', id: 7 },
    { kind: 'source', id: 7, path: '/Users/music/VK Download/1970-е' },
    { kind: 'source', id: 7, path: '/Users/music/VK Download/1970-е/Soul' },
  ])
})

test('source roots are not repeated and sibling paths cannot create parent links', () => {
  const source = { id: 1, path: '/samples/Pack' }
  for (const path of [undefined, '/samples/Pack/', '/samples/Pack extra/Kicks', '/elsewhere', '/samples/Pack/../Other']) {
    assert.equal(sourceBreadcrumbs(source, path).length, 1)
  }
})

test('Windows and filesystem root paths preserve valid navigation targets', () => {
  const windows = sourceBreadcrumbs({ id: 2, path: 'C:\\Samples\\' }, 'C:\\Samples\\Drums\\Kicks')
  assert.deepEqual(windows.at(-1)?.scope, { kind: 'source', id: 2, path: 'C:\\Samples\\Drums\\Kicks' })
  assert.equal(windows[0].label, 'Samples')
  const root = sourceBreadcrumbs({ id: 3, path: '/' }, '/Drums/Kicks/')
  assert.deepEqual(root.map(crumb => crumb.path), ['/', '/Drums', '/Drums/Kicks'])
})
