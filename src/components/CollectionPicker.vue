<script setup lang="ts">
import { Layers, Plus } from '@lucide/vue'
import { ListboxItem } from 'reka-ui'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from '@/components/ui/command'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { formatNumber } from '@/format'
import { useLibraryStore } from '@/stores/library'

const { t } = useI18n()
const library = useLibraryStore()
const open = ref(false)
const query = ref('')
const busy = ref(false)
const canCreate = computed(() => {
  const name = query.value.trim().toLocaleLowerCase()
  return !!name && !library.collections.some(c => c.name.toLocaleLowerCase() === name)
})

watch(open, (value) => {
  if (value)
    query.value = ''
})
watch(() => library.selectedIds.size, (count) => {
  if (!count)
    open.value = false
})

async function add(collectionId?: number) {
  if (busy.value || !library.selectedIds.size || (collectionId == null && !canCreate.value))
    return
  busy.value = true
  try {
    if (collectionId == null)
      await library.createCollection(query.value.trim(), true)
    else
      await library.addToCollection(collectionId)
    open.value = false
  }
  catch (error) {
    toast.error(String(error))
  }
  finally {
    busy.value = false
  }
}
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <Button
        variant="outline" size="sm" class="shrink-0"
        :disabled="!library.selectedIds.size" :aria-label="t('list.menu.addToCollection')"
        :title="t(library.selectedIds.size ? 'list.menu.addToCollection' : 'list.collectionPicker.selectSamples')"
      >
        <Layers /> {{ t('list.collectionPicker.button') }}
      </Button>
    </PopoverTrigger>
    <PopoverContent align="end" class="w-72 p-0">
      <div class="px-3 py-2 border-b border-border text-xs text-muted-foreground" aria-live="polite">
        {{ t('list.search.selected', { n: formatNumber(library.selectedIds.size) }) }}
      </div>
      <Command :aria-busy="busy">
        <CommandInput :placeholder="t('list.collectionPicker.placeholder')" @input="query = ($event.target as HTMLInputElement).value" />
        <CommandList>
          <CommandEmpty v-if="!canCreate">
            {{ t('list.collectionPicker.empty') }}
          </CommandEmpty>
          <CommandGroup v-if="library.collections.length">
            <CommandItem v-for="c in library.collections" :key="c.id" :value="c.name" :disabled="busy" @select="add(c.id)">
              <Layers /> <span class="truncate">{{ c.name }}</span>
              <span class="ml-auto text-xs text-muted-foreground tabular">{{ formatNumber(c.sampleCount) }}</span>
            </CommandItem>
          </CommandGroup>
          <ListboxItem
            v-if="canCreate" :value="`create:${query}`" :disabled="busy"
            class="m-1 flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-hidden select-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground data-[disabled]:opacity-50 [&_svg]:size-4 [&_svg]:shrink-0"
            @select="add()"
          >
            <Plus /> <span class="truncate">{{ t('list.collectionPicker.create', { name: query.trim() }) }}</span>
          </ListboxItem>
        </CommandList>
      </Command>
    </PopoverContent>
  </Popover>
</template>
