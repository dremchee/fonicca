import { readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

/** Resource ownership gates; RSS is reported without an invented leak threshold. */
export function auditResourceReport(report, cycles = report.mode === 'smoke' ? 1 : 3) {
  const checkpoints = report.checkpoints ?? [], checks = []
  const add = (name, passed, evidence) => checks.push({ name, passed: !!passed, evidence })
  const stage = name => checkpoints.find(point => point.stage === name)
  const required = ['baseline', 'library-scrolled', 'detached-rendered', `cycle-${cycles}-returned`,
    'closed-with-pending-spectrum', 'pending-work-drained', 'settled-after-close',
    ...Array.from({ length: cycles }, (_, i) => [`cycle-${i + 1}-opened`, `cycle-${i + 1}-spectrum`, `cycle-${i + 1}-closed`]).flat()]
  add('scenario completed', report.status === 'measured' && report.finished?.ok, report.error)
  add('all lifecycle stages recorded', required.every(name => stage(name)), required.filter(name => !stage(name)))
  add('visible unlocked observations', checkpoints.length && checkpoints.every(point => point.visibility === 'visible' && point.screenLocked === false))
  add('native host observed', Number.isInteger(report.nativeHostPid) && checkpoints.length && checkpoints.every(point => point.processes?.some(process => process.pid === report.nativeHostPid)))
  add('ordinary binary unchanged', report.normalBinary?.unchanged, report.normalBinary)
  if (report.sourceChangedDuringRun)
    add('source snapshot unchanged', report.sourceChangedDuringRun.length === 0, report.sourceChangedDuringRun)
  add('attributed processes exited', Array.isArray(report.cohortRemainingAfterHostExit) && report.cohortRemainingAfterHostExit.length === 0, report.cohortRemainingAfterHostExit)
  const baseline = stage('library-scrolled')?.callbacks ?? stage('baseline')?.callbacks
  const closed = checkpoints.filter(point => /^cycle-\d+-closed$/.test(point.stage) || point.stage === 'settled-after-close')
  add('closed editors release native sessions', closed.length === cycles + 1 && closed.every(point => point.native?.editor === null && !point.editor?.open))
  add('closed editors release renderer ownership', closed.length && closed.every(point => point.canvases === 0 && point.resources?.live === 0 && point.resources?.lanes === 0))
  add('closed editors leave no animation frame', closed.length && closed.every(point => point.pendingRaf === 0))
  add('callbacks return to baseline', Number.isFinite(baseline) && closed.length && closed.every(point => point.callbacks <= baseline), { baseline, closed: closed.map(point => ({ stage: point.stage, callbacks: point.callbacks })) })
  const drained = stage('pending-work-drained')
  add('pending work eventually drains', drained && Object.keys(drained.pendingEditor ?? {}).length === 0 && stage('settled-after-close')?.callbacks <= baseline, drained?.sinceCloseMs)
  const libraryPoints = checkpoints.filter(point => point.window === 'main')
  add('library cache stays bounded', libraryPoints.length && libraryPoints.every(point => point.library && point.library.retainedPages <= point.library.maxPages))
  const native = point => point.processes?.find(process => process.pid === report.nativeHostPid)?.rssBytes ?? null
  return { passed: checks.every(check => check.passed), checks,
    memory: checkpoints.map(point => ({ stage: point.stage, window: point.window, nativeRssBytes: native(point),
      webkitRss: (point.processes ?? []).filter(process => process.pid !== report.nativeHostPid).map(({ pid, name, rssBytes }) => ({ pid, name, rssBytes })),
      footprint: point.footprint?.processes ?? null, sinceCloseMs: point.sinceCloseMs ?? null })),
    limits: ['Ownership counters do not measure JavaScript garbage collection or GPU driver memory.',
      'Per-process RSS/footprint are not disjoint physical totals and must not be summed with PCM counters.',
      'No RSS limit or memory-leak conclusion is inferred from one process snapshot.'] }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const report = auditResourceReport(JSON.parse(readFileSync(process.argv[2], 'utf8')))
  console.log(JSON.stringify(report, null, 2))
  if (!report.passed) process.exitCode = 1
}
