// Smoke test: opens every teacher stage and every trail page (#level-1..3, the links posted in Classroom),
// plays every mission to its end, screenshots each, at laptop and phone size.
// Run: cd test && npm install && timeout 300 node smoke.mjs [URL]   (default: the live site)
import puppeteer from 'puppeteer-core';
import { mkdirSync, rmSync } from 'fs';
import { fileURLToPath } from 'url';
import path from 'path';

const here = path.dirname(fileURLToPath(import.meta.url));
const BASE = (process.argv[2] || 'https://vaknin.github.io/nature-words/').replace(/#.*$/, '');
const SHOTS = path.join(here, 'shots');
rmSync(SHOTS, { recursive: true, force: true }); mkdirSync(SHOTS, { recursive: true });
const CHROME = process.env.CHROME || '/usr/bin/chromium';
// His two check sizes: the school laptop's 1366x768 and a 390px-wide phone.
const SIZES = { laptop: { width: 1366, height: 768 }, phone: { width: 390, height: 844 } };
// His limits for this lesson: at most 6 stages, 40-45 minutes in all.
const MAX_STAGES = 6, MIN_MIN = 40, MAX_MIN = 45;

const sleep = ms => new Promise(r => setTimeout(r, ms));
let failures = 0;
const fail = m => { failures++; console.log('  FAIL', m); };
const ok = (c, m) => { if (!c) fail(m); };

// Buttons and links that overlap each other, and anything wider than the screen.
const layout = page => page.evaluate(() => {
  const vis = e => { const r = e.getBoundingClientRect(), s = getComputedStyle(e); return r.width > 2 && r.height > 2 && s.visibility !== 'hidden' && s.display !== 'none' && +s.opacity > 0.05; };
  const els = [...document.querySelectorAll('button, a')].filter(vis).filter(e => !e.parentElement.closest('button, a'));
  const bad = [];
  for (let i = 0; i < els.length; i++) for (let j = i + 1; j < els.length; j++) {
    const a = els[i].getBoundingClientRect(), b = els[j].getBoundingClientRect();
    // 2px slack: rounded shadows and borders touching is fine, covering is not.
    const ox = Math.min(a.right, b.right) - Math.max(a.left, b.left), oy = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
    if (ox > 2 && oy > 2) bad.push(`"${els[i].textContent.trim().slice(0, 20)}" covers "${els[j].textContent.trim().slice(0, 20)}"`);
  }
  const wide = document.documentElement.scrollWidth - innerWidth;
  const off = [...document.querySelectorAll('button, a, .pin, .act')].filter(vis).filter(e => { const r = e.getBoundingClientRect(); return r.left < -1 || r.right > innerWidth + 1; }).map(e => e.textContent.trim().slice(0, 20));
  return { bad, wide, off };
});

async function run(name, vp) {
  console.log(`\n== ${name} ${vp.width}x${vp.height}`);
  const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, args: ['--no-sandbox', '--autoplay-policy=no-user-gesture-required'] });
  const page = await browser.newPage();
  await page.setViewport(vp);
  const errors = [];
  page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
  page.on('pageerror', e => errors.push(e.message));
  let n = 0;
  const shot = async what => { await sleep(150); await page.screenshot({ path: path.join(SHOTS, `${name}-${String(++n).padStart(2, '0')}-${what}.png`) }); };
  const check = async what => {
    const l = await layout(page);
    for (const b of l.bad) fail(`${name} ${what}: ${b}`);
    ok(l.wide <= 1, `${name} ${what}: page ${l.wide}px wider than the screen`);
    for (const o of l.off) fail(`${name} ${what}: "${o}" off screen`);
  };
  const open = async hash => { console.log('  open', hash); await page.goto(BASE + hash, { waitUntil: 'load' }); await page.evaluate(() => window.dispatchEvent(new HashChangeEvent('hashchange'))); await sleep(120); };
  const click = async sel => { const h = await page.$(sel); ok(h, `missing ${sel}`); if (h) { await h.click(); await sleep(40); } };

  // ---- landing
  await open('');
  ok(await page.$$eval('.pin', p => p.length) === 3, 'landing shows three trail pins');
  await check('landing'); await shot('landing');

  // ---- teacher stages
  await open('#teacher');
  const stages = await page.evaluate(() => LESSON.stages.map(s => s.min));
  const total = stages.reduce((a, b) => a + b, 0);
  ok(stages.length <= MAX_STAGES, `${stages.length} teacher stages, more than ${MAX_STAGES}`);
  ok(total >= MIN_MIN && total <= MAX_MIN, `stage minutes sum to ${total}, outside ${MIN_MIN}-${MAX_MIN}`);
  for (let i = 1; i <= stages.length; i++) {
    await page.keyboard.press(String(i)); await sleep(100);
    ok(await page.$eval('.tscreen', e => +e.dataset.stage) === i, `key ${i} opens stage ${i}`);
    await page.keyboard.press(' '); await sleep(80);
    await check(`stage ${i}`); await shot(`stage-${i}`);
    if (i < stages.length) {
      // ArrowRight must move: to the next item inside the stage, and after the last item to the next stage.
      for (let guard = 0; ; guard++) {
        const before = await page.$eval('.tscreen', e => e.dataset.stage + '|' + e.innerHTML.length + '|' + e.textContent);
        await page.keyboard.press('ArrowRight'); await sleep(80);
        const after = await page.$eval('.tscreen', e => e.dataset.stage + '|' + e.innerHTML.length + '|' + e.textContent);
        if (before === after) { fail(`stage ${i}: ArrowRight does not advance`); break; }
        if (+after.split('|')[0] === i + 1) break;
        // 30 presses: more than any stage has items (8 words), so a stage that never ends fails here.
        if (guard > 30) { fail(`stage ${i}: ArrowRight never reaches stage ${i + 1}`); break; }
      }
    }
  }

  // ---- the three trails and every mission on them
  for (const lv of [1, 2, 3]) {
    await open(`#level-${lv}`);
    ok(await page.$eval('.trail', e => +e.dataset.trail).catch(() => 0) === lv, `#level-${lv} opens its own trail`);
    const acts = await page.$$eval('.act', a => a.length);
    ok(acts >= 1, `#level-${lv} has no missions`);
    ok(await page.$eval('.trail', e => e.innerText.trim().length).catch(() => 0) > 40, `#level-${lv} is empty`);
    await check(`level ${lv}`); await shot(`level-${lv}`);
    for (let m = 0; m < acts; m++) {
      await page.$$eval('.act', (a, m) => a[m].click(), m); await sleep(120);
      const type = await page.$eval('.trail', e => e.dataset.mission); console.log('  mission', lv, type);
      await check(`level ${lv} ${type}`); await shot(`level-${lv}-${type}`);
      // 200 moves: 8 rounds of a long word's letters (mountain: 8) with room to spare; a mission that never ends fails.
      for (let guard = 0; guard < 200 && !(await page.evaluate(() => app.dataset.ended)); guard++) {
        const ans = await page.$eval('.mission', e => e.dataset.answer || '');
        if (type === 'listen' || type === 'gap') await click(`.choice[data-w="${ans}"]`);
        else if (type === 'build') {
          const slot = await page.$$eval('.slot', s => s.findIndex(x => !x.textContent));
          if (slot < 0) { await sleep(1000); continue; }
          await page.$$eval('.tile', (t, c) => t.find(x => !x.disabled && x.textContent === c).click(), ans[slot]);
        } else if (type === 'paint') {
          const w = await page.$$eval('.pal button', b => (b.find(x => !x.disabled) || {}).dataset?.w);
          if (!w) { await sleep(700); continue; }
          await click(`.pal [data-w="${w}"]`);
          await page.$eval(`.canvas [data-part="${w}"]`, p => p.dispatchEvent(new MouseEvent('click', { bubbles: true })));
        } else if (type === 'say') { await click('[data-c]'); await click('[data-n]'); }
        else { fail(`unknown mission ${type}`); break; }
        // Engines wait up to 1.6 s between rounds (the gap sentence is read aloud); poll until the next round shows.
        await sleep(type === 'gap' ? 1700 : type === 'paint' ? 50 : 950);
      }
      ok(await page.evaluate(() => app.dataset.ended), `level ${lv} ${type}: never reached its end`);
      await shot(`level-${lv}-${type}-end`);
      await click('[data-end]');
    }
    const done = await page.$$eval('.act.done', a => a.length);
    ok(done === acts, `level ${lv}: ${done} of ${acts} missions marked done`);
    const nx = await page.$('.fin .nx');
    if (lv < 3) ok(nx, `level ${lv}: no "next trail" button`);
  }

  // Every phrase spoken during the run must have a recorded clip.
  const missing = await page.evaluate(() => [...new Set(window.MISSING)]);
  for (const t of missing) fail(`${name}: spoken with no clip: "${t}"`);
  for (const e of errors) fail(`${name}: console error: ${e}`);
  await browser.close();
}

for (const [name, vp] of Object.entries(SIZES)) await run(name, vp);
console.log(failures ? `\n${failures} failure(s)` : '\nall passed', `· shots in ${SHOTS}`);
process.exit(failures ? 1 : 0);
