// A fake batch run for the browser mocks: names from a few made-up samples,
// progress over a couple of seconds, one file failing.
import type { Recipe } from '@/composables/batch'
import { emit } from '@tauri-apps/api/event'

const fake = (id: number) => ({ name: `Kick ${id}`, category: 'kick', bpm: id % 2 ? 128 : null, key: null as string | null })

function render(template: string, id: number, n: number, total: number) {
  const s = fake(id)
  const width = Math.max(2, String(total).length)
  const out = template
    .replace(/\{name\}/g, s.name)
    .replace(/\{n\}/g, String(n).padStart(width, '0'))
    .replace(/\{category\}/g, s.category)
    .replace(/\{bpm\}/g, s.bpm ? String(s.bpm) : '')
    .replace(/\{\w+\}/g, '')
    .replace(/([_\- .])[_\- .]+/g, '$1')
    .replace(/^[_\- .]+|[_\- .]+$/g, '')
  return out || s.name
}

async function run(ids: number[], recipe: Recipe, destination: { mode: string, path?: string }) {
  for (let i = 0; i < ids.length; i++) {
    await new Promise(r => setTimeout(r, 150))
    await emit('batch:progress', { done: i + 1, total: ids.length, name: recipe.rename ? render(recipe.rename, ids[i], i + 1, ids.length) : fake(ids[i]).name })
  }
  await emit('batch:done', {
    written: ids.length - 1,
    failed: [{ name: fake(ids[0]).name, error: 'only WAV files can be replaced; save to a folder instead' }],
    cancelled: false,
    dir: destination.mode === 'folder' ? destination.path : null,
  })
}

/** Batch commands for the mock backend; `undefined` for anything else. */
export function batchCommand(cmd: string, args: any): unknown {
  switch (cmd) {
    case 'batch_preview': return args.ids.map((id: number, i: number) => render(args.template, id, i + 1, args.total))
    case 'batch_run': run(args.ids, args.recipe, args.destination); return null
    case 'batch_cancel': return null
    default: return undefined
  }
}
