import assert from 'node:assert/strict'
import test from 'node:test'
import { tagColorStyle, TAG_COLORS } from '../src/tag-colors.ts'

function luminance(hex: string) {
  const rgb = [1, 3, 5].map(at => parseInt(hex.slice(at, at + 2), 16) / 255)
    .map(c => c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4)
  return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722
}

test('tag identity is stable across renders, letter case and Unicode normalization', () => {
  assert.deepEqual(tagColorStyle(' Drums '), tagColorStyle('drums'))
  assert.deepEqual(tagColorStyle('ＢＡＳＳ'), tagColorStyle('bass'))
  assert.deepEqual(tagColorStyle('cafe\u0301'), tagColorStyle('café'))
  assert.deepEqual(tagColorStyle('Вокал'), tagColorStyle('вокал'))
})

test('every automatic color remains readable in light and dark modes', () => {
  for (const [lightBg, lightInk, darkBg, darkInk] of TAG_COLORS) {
    for (const [background, ink] of [[lightBg, lightInk], [darkBg, darkInk]]) {
      const values = [luminance(background), luminance(ink)].sort((a, b) => a - b)
      assert.ok((values[1] + 0.05) / (values[0] + 0.05) >= 4.5, `${background} / ${ink}`)
    }
  }
})

test('custom colors preserve the chosen marker and readable badge contrast', () => {
  for (const color of ['#000000', '#ffffff', '#ff0000', '#00ff00', '#0000ff', '#ffff00', '#ff00ff', '#00ffff', '#8277dd']) {
    const style = tagColorStyle('Drums', color)
    assert.equal(style['--tag-marker'], color)
    for (const mode of ['light', 'dark'] as const) {
      const values = [luminance(style[`--tag-${mode}-bg`]), luminance(style[`--tag-${mode}-ink`])].sort((a, b) => a - b)
      assert.ok((values[1] + 0.05) / (values[0] + 0.05) >= 4.5, `${color} / ${mode}`)
    }
  }
  assert.deepEqual(tagColorStyle('Drums', 'invalid'), tagColorStyle('Drums'))
})
