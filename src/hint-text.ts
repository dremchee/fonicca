/** Keep an action and its parenthesized scope on separate, centered lines. */
export function hintLines(text: string): string[] {
  const scope = text.match(/^(.*?)\s+(\([^()]+\))$/)
  return scope && text.length > 40
    ? [scope[1]!, scope[2]!]
    : text.split('\n')
}
