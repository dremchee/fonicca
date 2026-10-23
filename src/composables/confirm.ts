import { shallowRef } from 'vue'

export interface ConfirmOptions {
  title: string
  description?: string
  action?: string
  destructive?: boolean
}

interface PendingConfirm extends ConfirmOptions {
  resolve: (ok: boolean) => void
}

/** The confirmation currently on screen; rendered by ConfirmDialog in App. */
export const pendingConfirm = shallowRef<PendingConfirm | null>(null)

/** Asks the user to confirm an action; resolves false on cancel or Escape. */
export function confirm(options: ConfirmOptions): Promise<boolean> {
  pendingConfirm.value?.resolve(false)
  return new Promise(resolve => (pendingConfirm.value = { ...options, resolve }))
}

export function settleConfirm(ok: boolean) {
  pendingConfirm.value?.resolve(ok)
  pendingConfirm.value = null
}
