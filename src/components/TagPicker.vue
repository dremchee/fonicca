<script setup lang="ts">
import { useTagColors } from '@/composables/tagColors'
import type { TagRef } from '@/api'
import { Check, Plus } from '@lucide/vue'
import { ListboxItem } from 'reka-ui'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { formatNumber } from '@/format'
import { useLibraryStore } from '@/stores/library'

// "+ Tag" button with a searchable list of known tags and a way to create a new one.
const props = defineProps<{ assigned: TagRef[], count: number }>()

const { t } = useI18n()
const { tagStyle } = useTagColors()
const library = useLibraryStore()
const open = ref(false)
const query = ref('')

const available = computed(() => {
  const taken = new Set(props.assigned.map(tag => tag.id))
  return library.tags.filter(tag => !taken.has(tag.id))
})
const canCreate = computed(() => {
  const q = query.value.trim().toLowerCase()
  return !!q && !library.tags.some(tag => tag.name.toLowerCase() === q)
})

// T and "Add tag…" in the row menu open the picker.
watch(() => library.tagInputRequest, () => (open.value = true))
watch(open, (v) => {
  if (v)
    query.value = ''
})

async function add(name: string) {
  open.value = false
  await library.addTag(name)
}
</script>

<template>
  <!-- The tooltip wraps the whole picker: inside it, it would take the popup's anchor. -->
  <Hint :text="t('list.menu.addTag')" shortcut="T">
    <span class="inline-flex">
      <Popover v-model:open="open">
        <PopoverTrigger as-child>
          <Button variant="ghost" size="xs" class="text-muted-foreground data-[state=open]:bg-accent">
            <Plus /> {{ t('sidebar.tagPicker.button') }}
          </Button>
        </PopoverTrigger>
        <PopoverContent align="start" class="w-56 p-0">
          <Command>
            <CommandInput
              :placeholder="count > 1 ? t('sidebar.tagPicker.placeholderMany', { n: formatNumber(count) }, count) : t('sidebar.tagPicker.placeholder')"
              @input="query = ($event.target as HTMLInputElement).value"
            />
            <CommandList>
              <CommandEmpty v-if="!canCreate">
                {{ t('sidebar.tagPicker.empty') }}
              </CommandEmpty>
              <CommandGroup v-if="available.length">
                <CommandItem v-for="tag in available" :key="tag.id" :value="tag.name" @select="add(tag.name)">
                  <span class="tag-dot shrink-0" :style="tagStyle(tag)" aria-hidden="true" />
                  {{ tag.name }}
                  <span class="ml-auto text-xs text-muted-foreground tabular">{{ formatNumber(tag.sampleCount) }}</span>
                </CommandItem>
              </CommandGroup>
              <!-- A plain Listbox item: it must show for any query, and CommandItem filters itself
                   against the text it had when first mounted. -->
              <ListboxItem
                v-if="canCreate"
                :value="`create:${query}`"
                class="m-1 relative flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-hidden select-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground [&_svg]:size-4 [&_svg]:text-muted-foreground"
                @select="add(query.trim())"
              >
                <Plus /> {{ t('sidebar.tagPicker.create', { name: query.trim() }) }}
              </ListboxItem>
              <CommandGroup v-if="assigned.length" :heading="t('sidebar.tagPicker.assigned')">
                <CommandItem v-for="tag in assigned" :key="tag.id" :value="`on:${tag.name}`" disabled>
                  <Check /> <span class="tag-color rounded px-1" :style="tagStyle(tag)">{{ tag.name }}</span>
                </CommandItem>
              </CommandGroup>
            </CommandList>
          </Command>
        </PopoverContent>
      </Popover>
    </span>
  </Hint>
</template>
