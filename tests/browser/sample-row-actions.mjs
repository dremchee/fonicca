// Verify emitted row gestures against the real list/store logic on fake IPC.
export async function checkSampleRowActions(page) {
  const check = (value, message) => { if (!value) throw new Error(message) }
  await page.waitForSelector('.sample-list .row[data-index="0"]')
  await page.evaluate(() => {
    const p = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia
    const l = p._s.get('library'), settings = p._s.get('settings')
    const test = window.__rowActions = { original: window.__TAURI_INTERNALS__.invoke, calls: [],
      drag: l.dragSamples, stems: l.toggleStems, mode: settings.settings.rowPlaybackMode, gestures: [] }
    settings.settings.rowPlaybackMode = 'start'
    window.__TAURI_INTERNALS__.invoke = (cmd, args, options) => { test.calls.push({cmd,args}); return test.original(cmd,args,options) }
    l.dragSamples = async sample => { test.gestures.push({kind:'drag', id:sample.id, selected:[...l.selectedIds]}) }
    l.toggleStems = async (sample,index) => {
      test.gestures.push({kind:'stems',id:sample.id,index})
      if (l.expandedStems.has(sample.id)) l.expandedStems.delete(sample.id)
      else l.expandedStems.add(sample.id)
    }
  })
  const row = index => page.locator(`.sample-list .row[data-index="${index}"]:not([data-pinned])`)
  try {
    await row(1).locator('.track-title').click()
    await page.waitForFunction(() => {
      const p=document.querySelector('#app').__vue_app__.config.globalProperties.$pinia,l=p._s.get('library')
      return p._s.get('player').playingId===l.sampleAt(1).id && l.selectedIds.has(l.sampleAt(1).id)
    })
    await row(1).locator('.track-play').click()
    await page.waitForFunction(() => document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('player').paused)
    check(await row(1).evaluate(el=>el.classList.contains('paused')), 'paused row lost its state')
    await row(1).locator('.track-play').click()
    await page.waitForFunction(() => !document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('player').paused)
    const before = await page.evaluate(()=>window.__rowActions.calls.filter(c=>c.cmd==='play').length)
    await row(1).locator('.star').click()
    await page.waitForFunction(()=>{
      const l=document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('library')
      return l.sampleAt(1).favorite
    })
    check(await row(1).locator('.star svg').evaluate(el=>el.classList.contains('fill-current')), 'favorite is not solid')
    await row(2).locator('.track-title').click({modifiers:['Meta']})
    check(await page.evaluate(()=>{
      const p=document.querySelector('#app').__vue_app__.config.globalProperties.$pinia,l=p._s.get('library')
      return l.selectedIds.has(l.sampleAt(1).id)&&l.selectedIds.has(l.sampleAt(2).id)&&p._s.get('player').playingId===l.sampleAt(1).id
    }), 'modified selection changed playback or lost a selection')
    check(await page.evaluate(()=>window.__rowActions.calls.filter(c=>c.cmd==='play').length)===before,'favorite/modifier press restarted playback')
    await row(2).dispatchEvent('dragstart')
    check(await page.evaluate(()=>window.__rowActions.gestures.some(g=>g.kind==='drag'&&g.selected.length===2)),'drag did not preserve multi-selection')

    await page.evaluate(()=>{
      const l=document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('library')
      const rows=l.pages.get(0).slice();rows[0]={...rows[0],stemCount:2};l.pages.set(0,rows)
    })
    await row(0).locator('button.stem-toggle').click()
    check(await row(0).locator('button.stem-toggle').getAttribute('aria-expanded')==='true','stem toggle did not reach its row')
    check(await page.evaluate(()=>window.__rowActions.gestures.some(g=>g.kind==='stems'&&g.index===0)),'stem toggle lost its index')
    check(await page.evaluate(()=>window.__rowActions.calls.filter(c=>c.cmd==='play').length)===before,'stem disclosure restarted playback')
    await row(1).locator('.track-title').dblclick()
    await page.waitForFunction(()=>{
      const p=document.querySelector('#app').__vue_app__.config.globalProperties.$pinia
      return p._s.get('editor').sample?.id===p._s.get('library').sampleAt(1).id&&!p._s.get('editor').loading
    })
    check(await page.evaluate(()=>window.__rowActions.calls.filter(c=>c.cmd==='editor_open').length)===1,'double click did not open exactly one editor')
    await page.evaluate(async()=>{
      const p=document.querySelector('#app').__vue_app__.config.globalProperties.$pinia
      await p._s.get('editor').closeEmbedded()
      await p._s.get('player').play(p._s.get('library').sampleAt(0))
    })
    await page.evaluate(()=>{document.querySelector('.sample-list div[tabindex="0"].overflow-y-auto').scrollTop=200})
    const pinned=page.locator('[data-pinned="true"]')
    await pinned.waitFor()
    check(await pinned.locator('.stem-toggle').count()===0,'pinned row exposed stem disclosure')
    await pinned.locator('.track-play').click()
    await page.waitForFunction(()=>document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('player').paused)
    check(await pinned.evaluate(el=>el.classList.contains('paused')),'pinned row play button lost its target')

    await page.evaluate(async()=>{
      const p=document.querySelector('#app').__vue_app__.config.globalProperties.$pinia
      p._s.get('settings').settings.rowPlaybackMode='alt'
      await p._s.get('player').stop()
      document.querySelector('.sample-list div[tabindex="0"].overflow-y-auto').scrollTop=0
      const {useWaveformBackgrounds}=await import('/src/composables/waveformBackgrounds.ts')
      const values=new Float32Array(256*6)
      for(let i=0;i<256;i++){values[i*6]=-.5;values[i*6+1]=.5}
      useWaveformBackgrounds().remember(p._s.get('library').sampleAt(2),{count:256,lanes:1,samples:false,stride:6,values})
    })
    await pinned.waitFor({state:'hidden'})
    const name=row(2).locator('.track-name'),box=await name.boundingBox()
    await name.click({modifiers:['Alt'],position:{x:box.width/2,y:box.height/2}})
    await page.waitForFunction(()=>{
      const p=document.querySelector('#app').__vue_app__.config.globalProperties.$pinia
      return p._s.get('player').playingId===p._s.get('library').sampleAt(2).id
    })
    check(await page.evaluate(()=>{const start=window.__rowActions.calls.filter(c=>c.cmd==='play').at(-1).args.start;return start>0&&start<.5}),'row emission lost cursor coordinates or modifier')
    return {passed:['row selection/play','pause/resume button','solid favorite without playback','modified multi-selection','drag selection','stem disclosure','double-click editor','pinned row pause and hidden stems','cursor modifier and coordinates']}
  } finally {
    await page.evaluate(async()=>{
      const p=document.querySelector('#app').__vue_app__.config.globalProperties.$pinia,l=p._s.get('library'),test=window.__rowActions
      await p._s.get('player').stop()
      l.dragSamples=test.drag;l.toggleStems=test.stems;p._s.get('settings').settings.rowPlaybackMode=test.mode
      window.__TAURI_INTERNALS__.invoke=test.original
    })
  }
}
