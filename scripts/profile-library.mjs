// Opt-in browser workload; requires an existing Playwright CLI session. No audio,
// database, model downloads, or application settings are changed by this script.
import { spawnSync } from 'node:child_process'
import { writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

async function scrollWorkload() {
  const viewport = document.querySelector('.sample-list [aria-label="All samples"][tabindex="0"]')
  if (!viewport || viewport.scrollHeight < 3_000_000) throw new Error('100k mock library is not loaded')
  const longTasks = [], frameGapsMs = []
  const observer = new PerformanceObserver(list => longTasks.push(...list.getEntries().map(entry => entry.duration)))
  observer.observe({ type: 'longtask' })
  let last, raf
  const frame = time => {
    if (last !== undefined) frameGapsMs.push(time - last)
    last = time
    raf = requestAnimationFrame(frame)
  }
  raf = requestAnimationFrame(frame)
  const start = performance.now()
  try {
    for (let i = 1; i <= 35; i++) {
      viewport.scrollTop = i * 90_000
      await new Promise(done => setTimeout(done, 80))
    }
    await new Promise(done => setTimeout(done, 1100))
    return { elapsedMs: performance.now() - start, longTasks, frameGapsMs,
      scrollTop: viewport.scrollTop, rows: viewport.querySelectorAll('.row').length,
      firstRow: viewport.querySelector('.row')?.dataset.index,
      viewport: { width: innerWidth, height: innerHeight } }
  }
  finally { cancelAnimationFrame(raf); observer.disconnect() }
}

export function runLibraryProfile({ url, session = 'fonicca-profile', cli = process.env.PLAYWRIGHT_CLI ?? 'playwright-cli', cpu = false }) {
  const target = new URL(url)
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(target.hostname)
    || target.pathname !== '/mock.html' || target.hash !== '#library-large' || target.searchParams.get('size') !== '100000') {
    throw new Error('Use a local /mock.html?size=100000#library-large fixture, never a user library')
  }
  const code = `async page => {
    if (page.url() === ${JSON.stringify(url)}) await page.reload();
    else await page.goto(${JSON.stringify(url)});
    await page.locator('.sample-list [aria-label="All samples"][tabindex="0"]').evaluate(element => element.scrollTop = 0);
    await page.locator('.sample-list .row[data-index="0"]').waitFor();
    await page.waitForTimeout(1200);
    const cdp = await page.context().newCDPSession(page);
    let profile;
    await cdp.send('Performance.enable');
    const before = await cdp.send('Performance.getMetrics');
    try {
      ${cpu ? "await cdp.send('Profiler.enable'); await cdp.send('Profiler.start');" : ''}
      const result = await page.evaluate(${scrollWorkload.toString()});
      const after = await cdp.send('Performance.getMetrics');
      ${cpu ? "profile = (await cdp.send('Profiler.stop')).profile;" : ''}
      const names = ['ScriptDuration', 'TaskDuration', 'LayoutDuration', 'RecalcStyleDuration'];
      const metric = (data, name) => data.metrics.find(item => item.name === name)?.value ?? 0;
      result.cpuMs = Object.fromEntries(names.map(name => [name, (metric(after, name) - metric(before, name)) * 1000]));
      result.browser = page.context().browser().version();
      return { ...result, profile };
    } finally { await cdp.detach(); }
  }`
  const command = cli.endsWith('.sh') ? 'bash' : cli
  const args = [...(command === 'bash' ? [cli] : []), '--session', session, 'run-code', code]
  const result = spawnSync(command, args, { encoding: 'utf8', timeout: 60_000, maxBuffer: 32 * 1024 ** 2,
    env: { ...process.env, npm_config_offline: 'true' } })
  if (result.error || result.status !== 0) throw new Error(result.error?.message ?? result.stderr + result.stdout)
  const json = result.stdout.split('### Result\n')[1]?.split('\n### Ran')[0]
  if (!json) throw new Error(`Playwright did not return a measurement: ${result.stdout}`)
  return JSON.parse(json)
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    const args = process.argv.slice(2), option = name => args[args.indexOf(name) + 1]
    if (!args.includes('--url') || !args.includes('--output')) throw new Error('Usage: node scripts/profile-library.mjs --url http://127.0.0.1:1434/mock.html?size=100000#library-large --output /tmp/library.json [--session name] [--cpu]')
    const report = runLibraryProfile({ url: option('--url'), session: args.includes('--session') ? option('--session') : undefined, cpu: args.includes('--cpu') })
    writeFileSync(option('--output'), `${JSON.stringify(report, null, 2)}\n`)
    console.log(JSON.stringify({ cpuMs: report.cpuMs, longTasks: report.longTasks, rows: report.rows }))
  }
  catch (error) { console.error(error.message); process.exitCode = 1 }
}
