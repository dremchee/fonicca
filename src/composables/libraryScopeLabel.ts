import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { folderName } from '@/format'
import { useLibraryStore } from '@/stores/library'

/** Shared labels for the header and its library search control. */
export function useLibraryScopeLabel() {
  const library = useLibraryStore()
  const { t } = useI18n()
  const title = computed(() => {
    const scope = library.scope
    switch (scope.kind) {
      case 'source': return folderName(scope.path ?? library.sources.find(s => s.id === scope.id)?.path ?? '') || t('sidebar.folders')
      case 'collection': return library.collections.find(c => c.id === scope.id)?.name ?? t('sidebar.collections')
      case 'tag': return library.tags.find(tag => tag.id === scope.id)?.name ?? t('sidebar.tags')
      case 'favorites': return t('sidebar.favorites')
      case 'similar': return t('list.similar.soundsLike')
      default: return t('sidebar.allSamples')
    }
  })
  const path = computed(() => {
    const scope = library.scope
    return scope.kind === 'source' ? scope.path ?? library.sources.find(s => s.id === scope.id)?.path : undefined
  })
  const searchLabel = computed(() => library.scope.kind === 'all'
    ? t('list.search.entireLibrary')
    : t('list.search.inScope', { name: title.value }))
  return { title, path, searchLabel }
}
