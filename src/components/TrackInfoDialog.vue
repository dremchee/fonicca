<script setup lang="ts">
import type { TagEdit } from '@/api'
import { ImagePlus, X } from '@lucide/vue'
import { open as pickFile } from '@tauri-apps/plugin-dialog'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { api } from '@/api'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Field, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { trackInfoFor, trackInfoRevision } from '@/composables/trackInfo'

const { t } = useI18n()

const form = ref({ title: '', artist: '', album: '', year: '', genre: '' })
const cover = ref<TagEdit['cover']>({ kind: 'keep' })
// The file's current cover, and the preview of a newly picked image.
const coverUrl = ref<string | null>(null)
const newUrl = ref<string | null>(null)
const saving = ref(false)

const open = computed({
  get: () => !!trackInfoFor.value,
  set: (v) => {
    if (!v)
      trackInfoFor.value = null
  },
})

function setCoverUrl(url: string | null) {
  if (coverUrl.value)
    URL.revokeObjectURL(coverUrl.value)
  coverUrl.value = url
}

function setNewUrl(url: string | null) {
  if (newUrl.value)
    URL.revokeObjectURL(newUrl.value)
  newUrl.value = url
}

const shownUrl = computed(() => (cover.value.kind === 'file' ? newUrl.value : cover.value.kind === 'keep' ? coverUrl.value : null))
const hasCover = computed(() => !!shownUrl.value || cover.value.kind === 'file')

// Load what the file has each time the dialog opens.
watch(trackInfoFor, async (sample) => {
  cover.value = { kind: 'keep' }
  setCoverUrl(null)
  setNewUrl(null)
  if (!sample)
    return
  form.value = { title: '', artist: '', album: '', year: '', genre: '' }
  const tags = await api.sampleTags(sample.id).catch(() => null)
  if (trackInfoFor.value?.id !== sample.id)
    return
  form.value = {
    title: tags?.title ?? '',
    artist: tags?.artist ?? '',
    album: tags?.album ?? '',
    year: tags?.year ? String(tags.year) : '',
    genre: tags?.genre ?? '',
  }
  if (tags?.hasCover) {
    const bytes = await api.sampleCover(sample.id).catch(() => new ArrayBuffer(0))
    if (trackInfoFor.value?.id === sample.id && bytes.byteLength)
      setCoverUrl(URL.createObjectURL(new Blob([bytes])))
  }
})

const fileName = (path: string) => path.split('/').pop() ?? path

async function chooseImage() {
  const path = await pickFile({ filters: [{ name: t('trackInfo.imageFiles'), extensions: ['jpg', 'jpeg', 'png', 'gif', 'bmp', 'webp'] }] })
  if (typeof path !== 'string')
    return
  try {
    const bytes = await api.coverPreview(path)
    setNewUrl(URL.createObjectURL(new Blob([bytes])))
    cover.value = { kind: 'file', path }
  }
  catch (e) {
    toast.error(String(e))
  }
}

async function save() {
  const sample = trackInfoFor.value
  if (!sample)
    return
  const text = (v: string) => v.trim() || null
  const year = Number.parseInt(form.value.year, 10)
  saving.value = true
  try {
    await api.updateSampleTags(sample.id, {
      title: text(form.value.title),
      artist: text(form.value.artist),
      album: text(form.value.album),
      year: Number.isFinite(year) && year > 0 ? year : null,
      genre: text(form.value.genre),
      cover: cover.value,
    })
    trackInfoRevision.value++
    toast.success(t('trackInfo.saved'))
    trackInfoFor.value = null
  }
  catch (e) {
    toast.error(String(e))
  }
  finally {
    saving.value = false
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <!-- min-w-0 on the parts: the dialog is a grid, and long names must not widen it. -->
    <DialogContent class="sm:max-w-xl">
      <DialogHeader class="min-w-0">
        <DialogTitle>{{ t('trackInfo.title') }}</DialogTitle>
        <DialogDescription class="flex min-w-0 flex-col gap-0.5">
          <span class="truncate text-foreground/80">{{ trackInfoFor ? `${trackInfoFor.name}.${trackInfoFor.ext}` : '' }}</span>
          <span>{{ t('trackInfo.description') }}</span>
        </DialogDescription>
      </DialogHeader>
      <form class="min-w-0 grid grid-cols-[148px_minmax(0,1fr)] gap-5" @submit.prevent="save">
        <div class="min-w-0 flex flex-col gap-1.5">
          <!-- The tile is the picker: click to choose an image; ✕ in the corner removes it. -->
          <div class="relative">
            <button
              type="button"
              class="group relative w-full aspect-square rounded-lg border bg-muted overflow-hidden grid place-items-center text-muted-foreground outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
              :aria-label="t('trackInfo.chooseImage')"
              @click="chooseImage"
            >
              <img v-if="shownUrl" :src="shownUrl" :alt="t('trackInfo.cover')" class="size-full object-cover">
              <span v-else class="flex flex-col items-center gap-1.5 text-xs">
                <ImagePlus class="size-6" />
                {{ t('trackInfo.noCover') }}
              </span>
              <span
                v-if="shownUrl"
                class="absolute inset-0 flex flex-col items-center justify-center gap-1.5 bg-black/55 text-xs text-white opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
              >
                <ImagePlus class="size-5" />
                {{ t('trackInfo.chooseImage') }}
              </span>
            </button>
            <Button
              v-if="hasCover"
              type="button" variant="secondary" size="icon-xs"
              class="absolute top-1.5 right-1.5 rounded-full shadow-sm"
              :aria-label="t('trackInfo.removeCover')"
              @click="cover = { kind: 'remove' }"
            >
              <X />
            </Button>
          </div>
          <p v-if="cover.kind === 'file'" class="truncate text-xs text-muted-foreground">
            {{ t('trackInfo.newCover', { name: fileName(cover.path) }) }}
          </p>
          <p v-else-if="cover.kind === 'remove' && coverUrl" class="text-xs text-muted-foreground">
            {{ t('trackInfo.coverRemoved') }} ·
            <button type="button" class="underline hover:text-foreground" @click="cover = { kind: 'keep' }">
              {{ t('common.actions.undo') }}
            </button>
          </p>
        </div>
        <FieldGroup class="min-w-0 gap-3">
          <Field class="gap-1.5">
            <FieldLabel for="ti-title">{{ t('trackInfo.fields.title') }}</FieldLabel>
            <Input id="ti-title" v-model="form.title" class="h-8" />
          </Field>
          <Field class="gap-1.5">
            <FieldLabel for="ti-artist">{{ t('trackInfo.fields.artist') }}</FieldLabel>
            <Input id="ti-artist" v-model="form.artist" class="h-8" />
          </Field>
          <Field class="gap-1.5">
            <FieldLabel for="ti-album">{{ t('trackInfo.fields.album') }}</FieldLabel>
            <Input id="ti-album" v-model="form.album" class="h-8" />
          </Field>
          <div class="grid grid-cols-[96px_1fr] gap-3">
            <Field class="gap-1.5">
              <FieldLabel for="ti-year">{{ t('trackInfo.fields.year') }}</FieldLabel>
              <Input id="ti-year" v-model="form.year" inputmode="numeric" maxlength="4" class="h-8 tabular" />
            </Field>
            <Field class="gap-1.5">
              <FieldLabel for="ti-genre">{{ t('trackInfo.fields.genre') }}</FieldLabel>
              <Input id="ti-genre" v-model="form.genre" class="h-8" />
            </Field>
          </div>
        </FieldGroup>
        <DialogFooter class="col-span-2">
          <Button type="button" variant="outline" @click="open = false">
            {{ t('common.actions.cancel') }}
          </Button>
          <Button type="submit" :disabled="saving">
            {{ t('common.actions.save') }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
