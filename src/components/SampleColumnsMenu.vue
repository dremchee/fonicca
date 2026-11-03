<script setup lang="ts">
import type { SampleTable } from '@/composables/sampleTable'
import { Columns3, RotateCcw } from '@lucide/vue'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuContent, DropdownMenuGroup,
  DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

const props = defineProps<{ table: SampleTable }>()
const { t } = useI18n()
const emit = defineEmits<{ resetWidths: [], resetView: [] }>()
const columns = computed(() => props.table.getAllLeafColumns())
</script>

<template>
  <Hint :text="t('list.columns.configure')">
    <span class="inline-flex shrink-0">
      <DropdownMenu>
        <DropdownMenuTrigger as-child>
          <Button variant="outline" size="icon-sm" :aria-label="t('list.columns.configure')">
            <Columns3 />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" class="w-56 normal-case tracking-normal">
          <DropdownMenuLabel>{{ t('list.columns.configure') }}</DropdownMenuLabel>
          <DropdownMenuGroup>
            <DropdownMenuCheckboxItem
              v-for="column in columns" :key="column.id" :model-value="column.getIsVisible()" :disabled="!column.getCanHide()"
              @update:model-value="column.toggleVisibility($event === true)" @select.prevent
            >
              {{ column.id === 'bpm' ? 'BPM' : t(`list.columns.${column.id}`) }}
              <span v-if="!column.getCanHide()" class="ml-auto text-xs text-muted-foreground">{{ t('list.columns.required') }}</span>
            </DropdownMenuCheckboxItem>
          </DropdownMenuGroup>
          <DropdownMenuSeparator />
          <DropdownMenuGroup>
            <DropdownMenuItem @select="emit('resetWidths')">
              <RotateCcw /> {{ t('list.columns.resetWidths') }}
            </DropdownMenuItem>
            <DropdownMenuItem @select="emit('resetView')">
              <RotateCcw /> {{ t('list.columns.reset') }}
            </DropdownMenuItem>
          </DropdownMenuGroup>
        </DropdownMenuContent>
      </DropdownMenu>
    </span>
  </Hint>
</template>
