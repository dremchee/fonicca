/** The destination stages its UI before authorizing the native ownership move. */
export interface EditorTransferPort<T> {
  prepareTransfer: (token: number) => Promise<T>
  acceptTransfer: (token: number) => Promise<void>
  rejectTransfer: (token: number) => Promise<void>
}

export async function acceptEditorTransfer<T>(
  token: number,
  port: EditorTransferPort<T>,
  ui: { stage: (opened: T) => void | Promise<void>, rollback: () => void },
): Promise<void> {
  try {
    const opened = await port.prepareTransfer(token)
    await ui.stage(opened)
    await port.acceptTransfer(token)
  } catch (error) {
    ui.rollback()
    // A timeout can already have removed the offer. Preserve the original error.
    await port.rejectTransfer(token).catch(() => {})
    throw error
  }
}
