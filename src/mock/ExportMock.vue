<script setup lang="ts">
import { ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'
import KitExportDialog from './KitExportDialog.vue'

const open = ref(true)
</script>

<template>
  <TooltipProvider :delay-duration="500" :skip-delay-duration="300">
    <!-- Stand-in for the main window: the dialog opens from a collection's context menu. -->
    <div class="h-full flex">
      <aside class="w-52 shrink-0 border-r border-border bg-card p-2 flex flex-col gap-px">
        <div class="px-2 pt-3 pb-1 text-[11px] font-medium text-muted-foreground">
          Collections
        </div>
        <button class="h-7 px-2 rounded-md flex items-center gap-2 bg-primary-soft text-primary" @click="open = true">
          <span class="i-lucide-layers" /> Techno Kit
        </button>
        <div class="h-7 px-2 flex items-center gap-2 text-muted-foreground">
          <span class="i-lucide-layers" /> Lo-fi chops
        </div>
      </aside>
      <main class="flex-1 grid place-items-center text-muted-foreground">
        <Button variant="outline" @click="open = true">
          <span class="i-lucide-hard-drive-upload" /> Export kit…
        </Button>
      </main>
    </div>
    <KitExportDialog v-model:open="open" />
    <Toaster theme="system" position="bottom-right" />
  </TooltipProvider>
</template>
