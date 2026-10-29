/** What the sidebar has selected; combined with the search query into a filter. */
export type Scope
  = | { kind: 'all' }
    | { kind: 'favorites' }
    | { kind: 'source', id: number, path?: string }
    | { kind: 'tag', id: number }
    | { kind: 'collection', id: number }
    /** Samples that sound like `id`, most similar first. */
    | { kind: 'similar', id: number, name: string }
    /** Groups of identical files; shown by DuplicatesView instead of the list. */
    | { kind: 'duplicates' }
