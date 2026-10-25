export interface ActiveStemProgress {
  sourceId: number
  stage: string
  percent: number
  indeterminate: boolean
}

/** The source comes from the job, never from a mutable selection or filename. */
export function activeStemProgress(status: {
  sourceId?: number | null
  running: boolean
  stage: string
  progress: number
}): ActiveStemProgress | null {
  if (!status.running || status.sourceId == null)
    return null
  return {
    sourceId: status.sourceId,
    stage: status.stage,
    percent: Number.isFinite(status.progress) ? Math.round(Math.min(1, Math.max(0, status.progress)) * 100) : 0,
    indeterminate: status.stage === 'preparing' || status.stage === 'importing',
  }
}
