<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useEditorStore } from '@/stores/editor'
import { Button } from '@/components/ui/button'
import { AlertDialog, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog'

const editor = useEditorStore()
const { t } = useI18n()
</script>

<template>
  <AlertDialog :open="editor.unsavedPrompt" @update:open="!$event && editor.resolveUnsaved('cancel')">
    <AlertDialogContent>
      <AlertDialogHeader>
        <AlertDialogTitle>{{ t('editor.unsaved.title') }}</AlertDialogTitle>
        <AlertDialogDescription>{{ t('editor.unsaved.description', { name: editor.sample?.name ?? '' }) }}</AlertDialogDescription>
      </AlertDialogHeader>
      <AlertDialogFooter class="flex-wrap">
        <Button variant="ghost" @click="editor.resolveUnsaved('cancel')">{{ t('common.actions.cancel') }}</Button>
        <Button variant="outline" @click="editor.resolveUnsaved('discard')">{{ t('editor.unsaved.dontSave') }}</Button>
        <Button @click="editor.resolveUnsaved('save')">{{ t('common.actions.save') }}</Button>
      </AlertDialogFooter>
    </AlertDialogContent>
  </AlertDialog>
</template>
