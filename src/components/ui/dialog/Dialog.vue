<script setup lang="ts">
import { Dialog } from '@ui-sdk/vue/dialog'

// Keep the modal and its backdrop in the same top-level container so the
// surrounding editor or library can be made inert independently.
const container = document.body
</script>

<template>
  <Dialog :container="container"><slot /></Dialog>
</template>
