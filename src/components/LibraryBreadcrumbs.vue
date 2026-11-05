<script setup lang="ts">
import { ChevronRight, Ellipsis } from '@lucide/vue'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { useLibraryScopeLabel } from '@/composables/libraryScopeLabel'
import { sourceBreadcrumbs, type LibraryBreadcrumb } from '@/library-breadcrumbs'
import { useLibraryStore } from '@/stores/library'

const { t } = useI18n()
const library = useLibraryStore()
const { title, path } = useLibraryScopeLabel()
const trail = computed<LibraryBreadcrumb[]>(() => {
  const root: LibraryBreadcrumb = { label: t('sidebar.allSamples'), scope: { kind: 'all' } }
  const scope = library.scope
  if (scope.kind === 'all') return [root]
  if (scope.kind === 'source') {
    const source = library.sources.find(source => source.id === scope.id)
    if (source) return [root, ...sourceBreadcrumbs(source, scope.path)]
  }
  return [root, { label: title.value, path: path.value, scope }]
})
const hidden = computed(() => trail.value.length > 4 ? trail.value.slice(1, -2) : [])
const visible = computed(() => hidden.value.length ? [trail.value[0], ...trail.value.slice(-2)] : trail.value)
</script>

<template>
  <nav class="library-breadcrumbs min-w-0" :aria-label="t('list.navigation.breadcrumbs')">
    <ol class="flex min-w-0 items-center gap-0.5 text-sm">
      <template v-for="(crumb, index) in visible" :key="`${index}:${crumb.path ?? crumb.label}`">
        <li v-if="index === 1 && hidden.length" class="flex shrink-0 items-center gap-0.5">
          <ChevronRight class="size-3 text-muted-foreground" aria-hidden="true" />
          <DropdownMenu>
            <DropdownMenuTrigger as-child>
              <Button variant="ghost" size="icon-xs" class="text-muted-foreground" :aria-label="t('list.navigation.parents')">
                <Ellipsis aria-hidden="true" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" class="max-w-80">
              <DropdownMenuItem v-for="parent in hidden" :key="parent.path ?? parent.label" :title="parent.path" @select="library.scope = parent.scope">
                <span class="truncate">{{ parent.label }}</span>
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </li>
        <li class="breadcrumb-item flex min-w-0 items-center gap-0.5" :class="index === visible.length - 1 ? 'breadcrumb-current flex-1' : 'breadcrumb-parent'">
          <ChevronRight v-if="index > 0" class="size-3 shrink-0 text-muted-foreground" aria-hidden="true" />
          <h1 v-if="index === visible.length - 1" aria-current="page" class="min-w-0 truncate px-1 font-medium text-foreground" :title="crumb.path ?? crumb.label">{{ crumb.label }}</h1>
          <button v-else type="button" class="breadcrumb-link min-w-0 truncate rounded-sm px-1 text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring" :title="crumb.path ?? crumb.label" @click="library.scope = crumb.scope">
            {{ crumb.label }}
          </button>
        </li>
      </template>
    </ol>
  </nav>
</template>

<style scoped>
.breadcrumb-parent { max-width: 28%; }
.breadcrumb-current { min-width: 3rem; }
.breadcrumb-link { height: 1.5rem; }
.library-breadcrumbs { margin-inline: -0.25rem; }
</style>
