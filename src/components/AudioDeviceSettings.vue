<script setup lang="ts">
import type { AudioOutputDevices } from '@/api'
import { RefreshCw } from '@lucide/vue'
import { computed, onMounted, shallowRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { api } from '@/api'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { useSettingsStore } from '@/stores/settings'

const props = defineProps<{ direction: 'input' | 'output' }>()
const input = computed(() => props.direction === 'input')
const label = computed(() => `settings.playback.${props.direction}`)
const { t } = useI18n()
const store = useSettingsStore()
const preference = computed(() => input.value ? store.settings.inputDevice : store.settings.outputDevice)
const outputs = shallowRef<AudioOutputDevices | null>(null)
const loading = shallowRef(false)
const switching = shallowRef(false)
const error = shallowRef('')
const selected = computed(() => preference.value ?? 'system')
const missing = computed(() => !!preference.value && !!outputs.value
  && !outputs.value.devices.some(device => device.id === preference.value))
const active = computed(() => outputs.value?.devices.find(device => device.id === outputs.value?.activeDeviceId))
const systemName = computed(() => outputs.value?.devices.find(device => device.isDefault)?.name)

async function refresh() {
  loading.value = true
  error.value = ''
  try {
    outputs.value = input.value
      ? { devices: await api.audioInputDevices(), activeDeviceId: null }
      : await api.audioOutputDevices()
  }
  catch (cause) {
    error.value = String(cause)
  }
  finally {
    loading.value = false
  }
}

async function select(value: unknown) {
  if (typeof value !== 'string' || value === selected.value || switching.value)
    return
  switching.value = true
  try {
    // Outputs are opened before saving; inputs are validated without starting capture.
    const device = value === 'system' ? null : value
    await store.update(input.value ? { inputDevice: device } : { outputDevice: device })
    await refresh()
  }
  catch (cause) {
    toast.error(String(cause))
  }
  finally {
    switching.value = false
  }
}

onMounted(refresh)
</script>

<template>
  <section class="p-4 flex flex-col gap-3" :aria-label="t(label)" :aria-busy="loading || switching">
    <div class="flex flex-wrap items-start justify-between gap-4">
      <div class="min-w-0 flex-1 basis-52">
        <h3 class="text-sm font-medium">{{ t(label) }}</h3>
        <p class="mt-1 text-xs text-muted-foreground">{{ t(`${label}Desc`) }}</p>
      </div>
      <div class="flex min-w-0 max-w-full items-center gap-2">
        <Select :model-value="selected" :disabled="loading || switching || !outputs" @update:model-value="select">
          <SelectTrigger class="w-64 max-w-full" :aria-label="t(label)">
            <SelectValue :placeholder="t('settings.playback.loadingOutputs')" />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              <SelectItem value="system">{{ t('settings.playback.systemOutput') }}<span v-if="systemName"> · {{ systemName }}</span></SelectItem>
              <SelectItem v-if="missing" :value="preference!" disabled>{{ t('settings.playback.savedOutput') }}</SelectItem>
              <SelectItem v-for="device in outputs?.devices ?? []" :key="device.id" :value="device.id">{{ device.name }}</SelectItem>
            </SelectGroup>
          </SelectContent>
        </Select>
        <Button variant="outline" size="icon" :disabled="loading || switching" :aria-label="t('settings.playback.refreshOutputs')" :title="t('settings.playback.refreshOutputs')" @click="refresh">
          <RefreshCw :class="{ 'animate-spin': loading }" />
        </Button>
      </div>
    </div>
    <p v-if="error" role="alert" class="text-xs text-destructive">{{ error }}</p>
    <p v-else-if="missing" role="status" class="text-xs text-muted-foreground">{{ t(input ? 'settings.playback.inputUnavailable' : 'settings.playback.outputUnavailable') }}</p>
    <p v-else-if="outputs && (input ? !outputs.devices.length : !active)" role="status" class="text-xs text-muted-foreground">{{ t(input ? 'settings.playback.noInput' : 'settings.playback.noOutput') }}</p>
    <p v-if="active" class="text-xs text-muted-foreground">{{ t('settings.playback.activeOutput', { name: active.name }) }}</p>
  </section>
</template>
