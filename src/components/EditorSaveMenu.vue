<script setup lang="ts">
import { computed } from 'vue'
import { formatNumber } from '@/format'
import { ChevronDown, Save } from '@lucide/vue'
import { useI18n } from 'vue-i18n'
import { useEditorStore } from '@/stores/editor'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuShortcut, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'

const editor = useEditorStore()
const { t } = useI18n()
const regionCount = computed(() => editor.regions.filter(region => !region.skip && region.end > region.start).length)
</script>

<template>
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <Button
        variant="ghost" size="sm" class="gap-1 px-2 data-[state=open]:bg-accent"
        :disabled="editor.saving || editor.processing" :aria-busy="editor.saving"
        :aria-label="t(editor.saving ? 'editor.save.saving' : 'common.actions.save')"
        :title="t(editor.saving ? 'editor.save.saving' : 'common.actions.save')"
      >
        <Save /> <ChevronDown class="size-3" />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end" class="min-w-56">
      <DropdownMenuItem :disabled="!editor.state?.dirty" @select="editor.save()">
        {{ t('common.actions.save') }} <DropdownMenuShortcut>⌘S</DropdownMenuShortcut>
      </DropdownMenuItem>
      <DropdownMenuItem @select="editor.save('saveAs')">
        {{ t('editor.save.as') }} <DropdownMenuShortcut>⇧⌘S</DropdownMenuShortcut>
      </DropdownMenuItem>
      <DropdownMenuItem :disabled="!editor.selection" @select="editor.save('selection')">
        {{ t('editor.save.selection') }}
      </DropdownMenuItem>
      <DropdownMenuItem :disabled="!regionCount" @select="editor.saveRegions()">
        {{ t('editor.regions.save') }}
        <span v-if="regionCount" class="ml-auto text-xs text-muted-foreground tabular">{{ formatNumber(regionCount) }}</span>
      </DropdownMenuItem>
      <DropdownMenuSeparator />
      <DropdownMenuItem :disabled="!editor.canOverwrite || (!editor.state?.dirty && editor.sample?.path === editor.state?.originalPath)" @select="editor.save('replaceOriginal')">
        {{ t('editor.save.replace') }}
      </DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
