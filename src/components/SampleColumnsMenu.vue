<script setup lang="ts">
import type { SampleColumnId, SampleColumnVisibility } from '@/sample-columns'
import { Columns3, RotateCcw } from '@lucide/vue'
import { useI18n } from 'vue-i18n'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuContent, DropdownMenuGroup,
  DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { defaultSampleColumns, SAMPLE_COLUMNS } from '@/sample-columns'

const visibility = defineModel<SampleColumnVisibility>({ required: true })
const { t } = useI18n()
const emit = defineEmits<{ resetWidths: [] }>()

function resetAll() {
  visibility.value = defaultSampleColumns()
  emit('resetWidths')
}

function toggle(id: SampleColumnId, checked: boolean | 'indeterminate') {
  visibility.value = { ...visibility.value, [id]: checked === true }
}
</script>

<template>
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <Button variant="ghost" size="icon-xs" class="sticky right-3 justify-self-end bg-background" :aria-label="t('list.columns.configure')" :title="t('list.columns.configure')">
        <Columns3 />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end" class="w-56 normal-case tracking-normal">
      <DropdownMenuLabel>{{ t('list.columns.configure') }}</DropdownMenuLabel>
      <DropdownMenuGroup>
        <DropdownMenuCheckboxItem :model-value="true" disabled>
          {{ t('list.columns.name') }}
          <span class="ml-auto text-xs text-muted-foreground">{{ t('list.columns.required') }}</span>
        </DropdownMenuCheckboxItem>
        <DropdownMenuCheckboxItem
          v-for="column in SAMPLE_COLUMNS" :key="column.id" :model-value="visibility[column.id]"
          @update:model-value="toggle(column.id, $event)" @select.prevent
        >
          {{ column.id === 'bpm' ? 'BPM' : t(`list.columns.${column.id}`) }}
        </DropdownMenuCheckboxItem>
      </DropdownMenuGroup>
      <DropdownMenuSeparator />
      <DropdownMenuGroup>
        <DropdownMenuItem @select="emit('resetWidths')">
          <RotateCcw /> {{ t('list.columns.resetWidths') }}
        </DropdownMenuItem>
        <DropdownMenuItem @select="resetAll">
          <RotateCcw /> {{ t('list.columns.reset') }}
        </DropdownMenuItem>
      </DropdownMenuGroup>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
