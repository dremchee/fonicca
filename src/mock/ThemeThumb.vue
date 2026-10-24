<script setup lang="ts">
import type { Palette } from './themes'

// A tiny drawing of the app in a palette: sidebar, rows, a waveform.
defineProps<{ pal: Palette }>()

const bars = [3, 6, 9, 5, 11, 7, 4, 8, 12, 6, 3, 9, 5, 7, 10, 4, 6, 8, 3, 5]
</script>

<template>
  <div class="thumb" :style="{ background: pal.background, borderColor: pal.border }">
    <div class="side" :style="{ background: pal.surface, borderColor: pal.border }">
      <i :style="{ background: pal.accent, opacity: 0.85 }" />
      <i :style="{ background: pal.muted }" />
      <i :style="{ background: pal.muted }" />
      <i :style="{ background: pal.muted, width: '55%' }" />
    </div>
    <div class="main">
      <div v-for="n in 3" :key="n" class="row" :style="{ borderColor: pal.border }">
        <i :style="{ background: pal.text, width: `${70 - n * 12}%` }" />
        <i :style="{ background: pal.muted, width: '18%' }" />
      </div>
      <div class="wave">
        <b v-for="(h, i) in bars" :key="i" :style="{ height: `${h * 1.6}px`, background: i < 8 ? pal.accent : pal.wave }" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.thumb {
  height: 84px;
  display: flex;
  border: 1px solid;
  border-radius: calc(var(--radius) - 2px);
  overflow: hidden;
}

.side {
  width: 30%;
  border-right: 1px solid;
  padding: 8px 6px;
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.side i,
.row i {
  display: block;
  height: 4px;
  border-radius: 2px;
}

.main {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.row {
  height: 13px;
  padding: 0 6px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid;
}

.row i {
  opacity: 0.8;
}

.wave {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 0 6px;
}

.wave b {
  flex: 1;
  border-radius: 1px;
}
</style>
