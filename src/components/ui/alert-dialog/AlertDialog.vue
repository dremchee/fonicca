<script setup lang="ts">
import { AlertDialog } from '@ui-sdk/vue/alert-dialog'

// Keep the modal and its backdrop in the same top-level container so the
// surrounding editor or library can be made inert independently.
const container = document.body
</script>

<template>
  <AlertDialog :container="container"><slot /></AlertDialog>
</template>
