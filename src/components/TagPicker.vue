<script setup lang="ts">
import type { TagRef } from '@/api'
import { Check, Plus } from '@lucide/vue'
import { ListboxItem } from 'reka-ui'
import { computed, ref, watch } from 'vue'
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
import { useLibraryStore } from '@/stores/library'

// "+ Tag" button with a searchable list of known tags and a way to create a new one.
const props = defineProps<{ assigned: TagRef[], count: number }>()

const library = useLibraryStore()
const open = ref(false)
const query = ref('')

const available = computed(() => {
  const taken = new Set(props.assigned.map(t => t.id))
  return library.tags.filter(t => !taken.has(t.id))
})
const canCreate = computed(() => {
  const q = query.value.trim().toLowerCase()
  return !!q && !library.tags.some(t => t.name.toLowerCase() === q)
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
  <Popover v-model:open="open">
    <!-- No Hint here: a tooltip trigger around the popover trigger takes its anchor away. -->
    <PopoverTrigger as-child>
      <Button variant="ghost" size="xs" class="text-muted-foreground data-[state=open]:bg-accent">
        <Plus /> Tag
      </Button>
    </PopoverTrigger>
    <PopoverContent align="start" class="w-56 p-0">
      <Command>
        <CommandInput
          :placeholder="count > 1 ? `Tag ${count} samples…` : 'Find or create a tag…'"
          @input="query = ($event.target as HTMLInputElement).value"
        />
        <CommandList>
          <CommandEmpty v-if="!canCreate">
            No tags yet.
          </CommandEmpty>
          <CommandGroup v-if="available.length">
            <CommandItem v-for="t in available" :key="t.id" :value="t.name" @select="add(t.name)">
              {{ t.name }}
              <span class="ml-auto text-xs text-muted-foreground tabular">{{ t.sampleCount.toLocaleString() }}</span>
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
            <Plus /> Create “{{ query.trim() }}”
          </ListboxItem>
          <CommandGroup v-if="assigned.length" heading="On this sample">
            <CommandItem v-for="t in assigned" :key="t.id" :value="`on:${t.name}`" disabled>
              <Check /> {{ t.name }}
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </Command>
    </PopoverContent>
  </Popover>
</template>
