export type ReleaseNoteBlock =
  | { kind: 'heading' | 'paragraph'; text: string }
  | { kind: 'list'; items: string[] }

// Updater notes are external text. Render a small, readable subset as Vue text
// nodes, never as HTML. Full Markdown remains available on GitHub Releases.
function plainText(text: string) {
  return text
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/\*\*([^*]+)\*\*|__([^_]+)__/g, '$1$2')
    .replace(/`([^`]+)`/g, '$1')
    .replace(/\s+\([a-f\d]{7,40}\)\s*$/i, '')
    .trim()
}

export function parseReleaseNotes(notes: string): ReleaseNoteBlock[] {
  const blocks: ReleaseNoteBlock[] = []
  let paragraph: string[] = []
  const flush = () => {
    if (paragraph.length) blocks.push({ kind: 'paragraph', text: plainText(paragraph.join(' ')) })
    paragraph = []
  }
  for (const line of notes.replace(/\r\n?/g, '\n').split('\n')) {
    const heading = line.match(/^\s{0,3}#{1,6}\s+(.+?)\s*#*$/)
    const item = line.match(/^\s*(?:[-*+]\s+|\d+\.\s+)(.+)$/)
    if (!line.trim() || /^\s*[-*_]{3,}\s*$/.test(line)) { flush(); continue }
    if (heading) {
      flush()
      const text = plainText(heading[1]!)
      if (!/^(changes|changelog|release notes)$/i.test(text)) blocks.push({ kind: 'heading', text })
    } else if (item) {
      flush()
      const last = blocks.at(-1)
      const text = plainText(item[1]!)
      if (last?.kind === 'list') last.items.push(text)
      else blocks.push({ kind: 'list', items: [text] })
    } else paragraph.push(line.trim())
  }
  flush()
  return blocks
}
