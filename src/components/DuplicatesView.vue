<script setup lang="ts">
import type { DuplicateCopy, DuplicateGroup } from '@/api'
import { CircleCheck, EyeOff, FolderOpen, Pin, Play, Square, Trash2, X } from '@lucide/vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { onKeyStroke } from '@vueuse/core'
import { storeToRefs } from 'pinia'
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Spinner } from '@/components/ui/spinner'
import { confirm } from '@/composables/confirm'
import { folderName, formatDuration, formatSize } from '@/format'
import { useDuplicatesStore } from '@/stores/duplicates'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'

const { t } = useI18n()
const dup = useDuplicatesStore()
const { groups, loading, falseMatches, rule, preferred, selected, focusedId, verifying, extras, trashable } = storeToRefs(dup)
const library = useLibraryStore()
const player = usePlayerStore()

onMounted(() => dup.load())

const GROUP_HEIGHT = 36
const COPY_HEIGHT = 32

type Row = { kind: 'group', g: DuplicateGroup } | { kind: 'copy', g: DuplicateGroup, c: DuplicateCopy }
const rows = computed<Row[]>(() => groups.value.flatMap(g => [
  { kind: 'group' as const, g },
  ...g.copies.map(c => ({ kind: 'copy' as const, g, c })),
]))

const scroller = ref<HTMLElement | null>(null)
const virtualizer = useVirtualizer(computed(() => ({
  count: rows.value.length,
  getScrollElement: () => scroller.value,
  estimateSize: (i: number) => rows.value[i]?.kind === 'group' ? GROUP_HEIGHT : COPY_HEIGHT,
  overscan: 20,
})))
// Row heights depend on the kind, so remeasure when the rows change.
watch(rows, () => virtualizer.value.measure())
const visible = computed(() => virtualizer.value.getVirtualItems().map(item => ({ item, row: rows.value[item.index]! })))

const totals = computed(() => ({
  copies: groups.value.reduce((n, g) => n + g.copies.length - 1, 0),
  bytes: groups.value.reduce((n, g) => n + g.size * (g.copies.length - 1), 0),
}))
const bytesOf = (copies: DuplicateCopy[]) => copies.reduce((n, c) => n + c.size, 0)

const allSelected = computed(() => groups.value.length > 0 && selected.value.size === groups.value.length)
function toggleAll(v: boolean | 'indeterminate') {
  selected.value.clear()
  if (v === true)
    groups.value.forEach(g => selected.value.add(g.key))
}

function sourceRoot(c: DuplicateCopy) {
  return library.sources.find(s => s.id === c.sourceId)?.path
}

/** Path below the source folder, split into segments; those not shared by every copy are marked. */
function segments(g: DuplicateGroup, c: DuplicateCopy) {
  const rel = (copy: DuplicateCopy) => {
    const root = sourceRoot(copy)
    return root && copy.path.startsWith(`${root}/`) ? copy.path.slice(root.length + 1) : copy.path
  }
  const sets = g.copies.map(x => new Set(rel(x).split('/')))
  return rel(c).split('/').map(s => ({ s, differs: !sets.every(set => set.has(s)) }))
}

/** The folder down to the first segment that differs between copies: what "always keep" pins. */
function packFolder(g: DuplicateGroup, c: DuplicateCopy) {
  const parts = segments(g, c)
  const n = Math.max(parts.findIndex(p => p.differs), 0) + 1
  const rel = parts.slice(0, Math.min(n, parts.length - 1)).map(p => p.s).join('/')
  const root = sourceRoot(c)
  return root ? (rel ? `${root}/${rel}` : root) : c.path.slice(0, c.path.lastIndexOf('/'))
}

function fileName(g: DuplicateGroup) {
  return folderName(dup.keepOf(g).path)
}

function formatDate(mtime: number) {
  return new Date(mtime * 1000).toLocaleDateString()
}

async function reveal(path: string) {
  try {
    await revealItemInDir(path)
  }
  catch (e) {
    toast.error(String(e))
  }
}

function filesLabel(n: number) {
  return t('common.count.files', { n: n.toLocaleString() }, n)
}

function reportSkipped(skipped: string[]) {
  if (skipped.length)
    toast.warning(t('duplicates.skipped', { files: filesLabel(skipped.length) }), { description: skipped.slice(0, 3).join('\n') })
}

async function trashExtras() {
  const list = trashable.value
  const waiting = extras.value.length - list.length
  const ok = await confirm({
    title: t('duplicates.trashAll.title', { files: filesLabel(list.length), size: formatSize(bytesOf(list)) }),
    description: `${t('duplicates.trashAll.description')}${
      waiting ? ` ${t('duplicates.trashAll.waiting', { files: filesLabel(waiting) })}` : ''}`,
    action: t('duplicates.moveToTrash'),
    destructive: true,
  })
  if (!ok)
    return
  try {
    const r = await dup.resolve('trash')
    toast.success(t('duplicates.trashed', { files: filesLabel(r.removed), size: formatSize(r.bytes) }))
    reportSkipped(r.skipped)
  }
  catch (e) {
    toast.error(String(e))
  }
}

async function hideExtras(only?: { group: DuplicateGroup, copy: DuplicateCopy }) {
  const paths = only ? [only.copy.path] : extras.value.map(c => c.path)
  try {
    const r = await dup.resolve('hide', only)
    toast(t('duplicates.hid', { files: filesLabel(r.removed) }), {
      description: t('duplicates.hidDescription'),
      action: { label: t('common.actions.undo'), onClick: () => dup.unhide(paths) },
    })
    reportSkipped(r.skipped)
  }
  catch (e) {
    toast.error(String(e))
  }
}

async function trashOne(group: DuplicateGroup, copy: DuplicateCopy) {
  const ok = await confirm({
    title: t('duplicates.trashOne.title', { name: folderName(copy.path) }),
    description: t('duplicates.trashOne.description'),
    action: t('duplicates.moveToTrash'),
    destructive: true,
  })
  if (!ok)
    return
  try {
    const r = await dup.resolve('trash', { group, copy })
    reportSkipped(r.skipped)
  }
  catch (e) {
    toast.error(String(e))
  }
}

async function prefer(folder: string, on = true) {
  await dup.prefer(folder, on)
  if (on)
    toast(t('duplicates.preferred', { folder: folderName(folder) }))
}

// One context menu for all rows; the row under the pointer is remembered.
const menuTarget = ref<{ g: DuplicateGroup, c: DuplicateCopy } | null>(null)
function onContextMenu(e: MouseEvent) {
  const el = (e.target as HTMLElement).closest<HTMLElement>('[data-row]')
  const row = el ? rows.value[Number(el.dataset.row)] : undefined
  if (row?.kind !== 'copy') {
    e.preventDefault()
    menuTarget.value = null
    return
  }
  menuTarget.value = { g: row.g, c: row.c }
  focusedId.value = row.c.id
}

// --- Keyboard: the global list shortcuts stand aside while this view is open.
const copyRows = computed(() => rows.value.filter(r => r.kind === 'copy'))

function move(delta: number) {
  const list = copyRows.value
  if (!list.length)
    return
  const i = list.findIndex(r => r.c.id === focusedId.value)
  const next = list[Math.min(list.length - 1, Math.max(0, i < 0 ? 0 : i + delta))]!
  focusedId.value = next.c.id
  virtualizer.value.scrollToIndex(rows.value.indexOf(next), { align: 'auto' })
  if (player.autoplay)
    player.play(next.c)
}

function isTyping(e: KeyboardEvent) {
  return e.target instanceof Element && (e.target.closest('[role="menu"], [role="dialog"], [role="alertdialog"], [role="listbox"]') !== null
    || (e.target instanceof HTMLInputElement && e.target.type !== 'checkbox'))
}
onKeyStroke('ArrowDown', (e) => {
  if (!isTyping(e)) {
    e.preventDefault()
    move(1)
  }
})
onKeyStroke('ArrowUp', (e) => {
  if (!isTyping(e)) {
    e.preventDefault()
    move(-1)
  }
})
onKeyStroke(' ', (e) => {
  const row = copyRows.value.find(r => r.c.id === focusedId.value)
  if (!isTyping(e) && row) {
    e.preventDefault()
    player.toggle(row.c)
  }
})
</script>

<template>
  <div class="flex-1 min-h-0 flex flex-col">
    <header class="h-11 shrink-0 px-4 flex items-center gap-3 border-b border-border" data-tauri-drag-region>
      <h1 class="font-semibold" data-tauri-drag-region>
        {{ t('duplicates.title') }}
      </h1>
      <span v-if="groups.length" class="text-xs text-muted-foreground tabular" data-tauri-drag-region>
        {{ t('duplicates.summary', {
          groups: t('common.count.groups', { n: groups.length.toLocaleString() }, groups.length),
          extras: t('duplicates.extraCopies', { n: totals.copies.toLocaleString() }, totals.copies),
          size: formatSize(totals.bytes),
        }) }}
      </span>
      <span v-if="verifying" class="text-xs text-muted-foreground flex items-center gap-1.5">
        <Spinner class="size-3 text-primary" /> {{ t('duplicates.verifying', { n: verifying.toLocaleString() }, verifying) }}
      </span>
      <span v-else-if="falseMatches" class="text-xs text-muted-foreground">
        {{ t('duplicates.falseMatches', falseMatches) }}
      </span>
    </header>

    <div v-if="groups.length" class="shrink-0 px-4 py-2 flex flex-wrap items-center gap-2 border-b border-border">
      <label class="flex items-center gap-2 mr-2">
        <Checkbox :model-value="allSelected ? true : selected.size ? 'indeterminate' : false" :aria-label="t('duplicates.selectAll')" @update:model-value="toggleAll" />
        <span class="text-muted-foreground text-xs tabular">{{ selected.size ? t('duplicates.selectedCount', { n: selected.size.toLocaleString() }) : t('duplicates.allGroups') }}</span>
      </label>
      <span class="text-xs text-muted-foreground">{{ t('duplicates.keep') }}</span>
      <Select v-model="rule">
        <SelectTrigger size="sm" class="min-w-40">
          <SelectValue>{{ t(`duplicates.rules.${rule}`) }}</SelectValue>
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="shortest">
            {{ t('duplicates.rules.shortest') }}
          </SelectItem>
          <SelectItem value="oldest">
            {{ t('duplicates.rules.oldest') }}
          </SelectItem>
          <SelectItem value="newest">
            {{ t('duplicates.rules.newest') }}
          </SelectItem>
        </SelectContent>
      </Select>
      <span
        v-for="f in preferred"
        :key="f"
        class="h-6 pl-2 pr-1 rounded-md bg-primary-soft text-primary text-xs flex items-center gap-1"
        :title="f"
      >
        <Pin class="size-3" /> {{ folderName(f) }}
        <button class="size-4 grid place-items-center rounded hover:bg-primary/15" :aria-label="t('duplicates.unprefer')" @click="prefer(f, false)">
          <X class="size-3" />
        </button>
      </span>
      <div class="ml-auto flex items-center gap-2">
        <Button variant="outline" size="sm" :disabled="!extras.length" @click="hideExtras()">
          <EyeOff /> {{ t('duplicates.hideExtras', { n: extras.length.toLocaleString() }) }}
        </Button>
        <Button variant="outline" size="sm" class="text-destructive hover:text-destructive" :disabled="!trashable.length" @click="trashExtras">
          <Trash2 /> {{ t('duplicates.trashExtras', { n: trashable.length.toLocaleString() }) }}
        </Button>
      </div>
    </div>

    <ContextMenu>
      <ContextMenuTrigger as-child>
        <div ref="scroller" class="flex-1 overflow-y-auto outline-none" tabindex="-1" @contextmenu="onContextMenu">
          <div v-if="!groups.length" class="h-full grid place-items-center text-muted-foreground">
            <div v-if="loading" class="flex items-center gap-2">
              <Spinner /> {{ t('duplicates.loading') }}
            </div>
            <div v-else class="flex flex-col items-center gap-2">
              <CircleCheck class="size-6 text-primary" />
              {{ t('duplicates.empty') }}
            </div>
          </div>
          <div v-else class="relative w-full" :style="{ height: `${virtualizer.getTotalSize()}px` }">
            <template v-for="{ item, row } in visible" :key="item.index">
              <div
                v-if="row.kind === 'group'"
                :data-row="item.index"
                class="absolute inset-x-0 px-4 flex items-center gap-3 bg-muted/40 border-t border-border"
                :style="{ height: `${item.size}px`, transform: `translateY(${item.start}px)` }"
              >
                <Checkbox
                  :model-value="selected.has(row.g.key)"
                  :aria-label="t('duplicates.selectGroup', { name: fileName(row.g) })"
                  @update:model-value="$event ? selected.add(row.g.key) : selected.delete(row.g.key)"
                />
                <span class="font-medium truncate">{{ fileName(row.g) }}</span>
                <span class="text-xs text-muted-foreground tabular shrink-0">
                  {{ t('duplicates.groupInfo', { copies: t('common.count.copies', row.g.copies.length), size: formatSize(row.g.size), duration: formatDuration(row.g.copies[0]!.duration) }) }}
                </span>
                <span v-if="!row.g.verified" class="text-[11px] text-muted-foreground flex items-center gap-1">
                  <Spinner class="size-3" /> {{ t('duplicates.groupVerifying') }}
                </span>
              </div>
              <div
                v-else
                :data-row="item.index"
                class="copy-row group absolute inset-x-0"
                :class="{
                  focused: focusedId === row.c.id,
                  extra: dup.keepOf(row.g).id !== row.c.id,
                  playing: player.playingId === row.c.id,
                }"
                :style="{ height: `${item.size}px`, transform: `translateY(${item.start}px)` }"
                @click="focusedId = row.c.id"
              >
                <button
                  class="keep"
                  :class="{ on: dup.keepOf(row.g).id === row.c.id }"
                  :aria-label="t('duplicates.keepPath', { path: row.c.path })"
                  @click.stop="dup.setKeep(row.g, row.c.id)"
                >
                  {{ dup.keepOf(row.g).id === row.c.id ? t('duplicates.badge.keep') : t('duplicates.badge.extra') }}
                </button>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  class="rounded-full shrink-0"
                  :aria-label="player.playingId === row.c.id ? t('common.actions.stop') : t('common.actions.play')"
                  @click.stop="focusedId = row.c.id; player.toggle(row.c)"
                >
                  <component :is="player.playingId === row.c.id ? Square : Play" class="fill-current" />
                </Button>
                <span class="flex-1 min-w-0 truncate" :title="row.c.path">
                  <template v-for="(p, i) in segments(row.g, row.c)" :key="i">
                    <span v-if="i" class="text-muted-foreground/60">/</span>
                    <span :class="p.differs ? 'text-foreground font-medium' : 'text-muted-foreground'">{{ p.s }}</span>
                  </template>
                </span>
                <span class="text-[11px] text-muted-foreground tabular shrink-0">{{ formatDate(row.c.mtime) }}</span>
                <Hint :text="t('common.actions.showInFinder')">
                  <Button variant="ghost" size="icon-xs" class="text-muted-foreground opacity-0 group-hover:opacity-100" :aria-label="t('common.actions.showInFinder')" @click.stop="reveal(row.c.path)">
                    <FolderOpen />
                  </Button>
                </Hint>
              </div>
            </template>
          </div>
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent v-if="menuTarget">
        <ContextMenuItem @select="dup.setKeep(menuTarget.g, menuTarget.c.id)">
          {{ t('duplicates.menu.keepThis') }}
        </ContextMenuItem>
        <ContextMenuItem @select="prefer(packFolder(menuTarget.g, menuTarget.c))">
          <Pin /> {{ t('duplicates.menu.alwaysKeep', { folder: folderName(packFolder(menuTarget.g, menuTarget.c)) }) }}
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem @select="reveal(menuTarget.c.path)">
          <FolderOpen /> {{ t('common.actions.showInFinder') }}
        </ContextMenuItem>
        <ContextMenuItem @select="hideExtras({ group: menuTarget.g, copy: menuTarget.c })">
          <EyeOff /> {{ t('duplicates.menu.hideThis') }}
        </ContextMenuItem>
        <ContextMenuItem
          :disabled="!menuTarget.g.verified"
          class="text-destructive focus:text-destructive"
          @select="trashOne(menuTarget.g, menuTarget.c)"
        >
          <Trash2 /> {{ t('duplicates.menu.trashThis') }}
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  </div>
</template>

<style scoped>
.copy-row {
  padding: 0 1rem 0 2.75rem;
  display: flex;
  align-items: center;
  gap: 0.625rem;
}

.copy-row:hover {
  background: var(--accent);
}

.copy-row.focused {
  background: var(--primary-soft);
}

.copy-row.extra:not(.focused) {
  opacity: 0.75;
}

/* Sized by its label: "Keep" and «Оставить» differ a lot in width. */
.keep {
  min-width: 4.5rem;
  padding: 0 0.375rem;
  height: 1.25rem;
  flex-shrink: 0;
  border-radius: calc(var(--radius) - 4px);
  font-size: 11px;
  border: 1px solid var(--border);
  color: var(--muted-foreground);
}

.keep:hover {
  border-color: var(--primary);
  color: var(--primary);
}

.keep.on {
  background: var(--primary);
  border-color: var(--primary);
  color: var(--primary-foreground);
}
</style>
