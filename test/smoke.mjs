// Smoke test: opens every teacher stage and every trail page (#level-1..3, the links posted in Classroom),
// plays the start quiz and every trail game to its end, screenshots each, at two laptop sizes.
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
// His word: laptop only. 1366x768 is the common school laptop; 1280x720 the smallest laptop screen in use.
const SIZES = { laptop: { width: 1366, height: 768 }, small: { width: 1280, height: 720 } };
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
  const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage', '--autoplay-policy=no-user-gesture-required'] });
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
  // A reload empties window.MISSING, so collect it before every navigation (a test that read it only at the end missed every clip).
  const missingAll = new Set();
  const harvest = async () => { for (const t of await page.evaluate(() => window.MISSING || []).catch(() => [])) missingAll.add(t); };
  const open = async hash => { await harvest(); console.log('  open', hash); await page.goto(BASE + hash, { waitUntil: 'load' }); await page.evaluate(() => window.dispatchEvent(new HashChangeEvent('hashchange'))); await sleep(120); };
  const click = async sel => { const h = await page.$(sel); ok(h, `missing ${sel}`); if (h) { await h.click(); await sleep(40); } };

  // ---- the one Classroom link: start, quiz, "Your trail is ...!"
  await open('');
  await check('start'); await shot('start');
  await click('[data-go]'); await sleep(150);
  ok(await page.$('[data-quiz]'), 'Start opens the trail quiz');
  await check('quiz'); await shot('quiz');
  // Laptop answers every quiz question right (must land on Mountain); the small laptop misses each first (River).
  const allRight = name === 'laptop';
  for (let guard = 0; guard < 30 && !(await page.$('[data-result]')); guard++) {
    const ans = await page.$eval('.stage', e => e.dataset.answer || '');
    if (!allRight) { await page.$$eval('.choice', (c, a) => { const x = c.find(x => x.dataset.w !== a); x && x.click(); }, ans); await sleep(700); continue; }
    await click(`.choice[data-w="${ans}"]`); await sleep(1300);
  }
  const res = await page.$eval('[data-result]', e => +e.dataset.result).catch(() => 0);
  ok(res === (allRight ? 3 : 1), `quiz ${allRight ? 'all right' : 'all wrong'} gave trail ${res}`);
  ok(/Start at (River|Forest|Mountain)! .* Well done!/.test(await page.$eval('[data-result]', e => e.textContent).catch(() => '')), 'quiz result line');
  await check('result'); await shot('result');

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

  // ---- the three trails: every game to its end
  for (const lv of [1, 2, 3]) {
    await open(`#level-${lv}`);
    ok(await page.$eval('.trail', e => +e.dataset.trail).catch(() => 0) === lv, `#level-${lv} opens its own trail`);
    ok(await page.$eval('.trail', e => e.innerText.trim().length).catch(() => 0) > 40, `#level-${lv} is empty`);
    await check(`level ${lv}`); await shot(`level-${lv}`);
    if (lv === 1) ok(await page.$('[data-chip][disabled]'), 'the race chip is not locked before the Last check');
    let lastGame = '', offers = 0;
    // 200 moves: 3 games x 8 rounds, a long word's 8 letters each, with room; a trail that never ends fails.
    for (let guard = 0; guard < 400 && !(await page.evaluate(() => app.dataset.ended)); guard++) {
      if (await page.$('.offer')) { offers++; await shot(`level-${lv}-offer`); await click('.offer [data-no]'); await sleep(200); continue; }
      const g = await page.$eval('.trail', e => e.dataset.game || '').catch(() => '');
      if (g && g !== lastGame) { lastGame = g; await check(`level ${lv} ${g}`); await shot(`level-${lv}-${g}`); }
      const ans = await page.$eval('.stage', e => e.dataset.answer || '').catch(() => '');
      const type = await page.$eval('.stage', e => e.dataset.type || '').catch(() => '');
      if (type === 'say') { await click('[data-n]'); await sleep(250); continue; }
      if (type === 'build') {
        const slot = await page.$$eval('.slot', s => s.findIndex(x => !x.textContent));
        if (slot < 0) { await sleep(400); continue; }
        await page.$$eval('.tile', (t, c) => { const x = t.find(x => !x.disabled && x.textContent === c); x && x.click(); }, ans[slot]);
        await sleep(30);
      } else if (await page.$(`.choice[data-w="${ans}"]:not(.ok)`)) { await click(`.choice[data-w="${ans}"]`); await sleep(400); }
      else await sleep(300);
    }
    ok(await page.evaluate(() => app.dataset.ended), `level ${lv}: never reached the end`);
    ok(await page.$('[data-done]'), `level ${lv}: no end screen with "Mark as done"`);
    ok(/Mark as done/.test(await page.$eval('[data-done]', e => e.textContent).catch(() => '')), `level ${lv}: end screen does not say "Mark as done"`);
    // Kids who get every first try right are offered the next trail up (not on the last trail).
    if (lv < 3) ok(offers > 0, `level ${lv}: no "try a new trail" offer after a run of right answers`);
    await check(`level ${lv} end`); await shot(`level-${lv}-end`);
    if (lv < 3) ok(await page.$('[data-next]'), `level ${lv}: no "next trail" button`);
    // The race card: trophy, record, GO; a race runs its fixed time and ends with the score.
    ok(await page.$('[data-racecard] .trophy') && await page.$('[data-record]'), `level ${lv}: race card without trophy or record`);
    if (lv === 1) {
      await click('[data-race]'); await sleep(200); await shot('race');
      // 3 s countdown + 30 s race; one press per question, so press only the right word.
      for (let guard = 0; guard < 300 && !(await page.$('[data-raceend]')); guard++) {
        const ans = await page.$eval('.stage', e => e.dataset.type === 'race' ? e.dataset.answer : '').catch(() => '');
        if (ans && await page.$(`.choice[data-w="${ans}"]:not(.ok)`)) { await click(`.choice[data-w="${ans}"]`); if (guard === 3) await shot('race'); }
        await sleep(150);
      }
      ok(await page.$('[data-raceend]'), 'the race never ended');
      const rec = await page.evaluate(() => JSON.parse(localStorage.getItem(Object.keys(localStorage).find(k => k.startsWith('nature-trails'))) || '{}').best || 0);
      ok(rec > 0, 'a race with right answers set no best');
      await shot('race-end');
    }
    const gold = await page.evaluate(() => Object.values(JSON.parse(localStorage.getItem(Object.keys(localStorage).find(k => k.startsWith('nature-trails'))) || '{}').stars || {}).filter(x => x.length === 3).length);
    ok(gold === 8, `level ${lv}: ${gold} of 8 gold cards after every game`);
    ok(await page.$('[data-chip].open'), `level ${lv}: the race chip is still locked after the Last check`);
  }

  // The plain link resumes where he was instead of a second quiz.
  await open('');
  ok(await page.$('[data-resume]'), 'the plain link does not offer to continue');

  // Every phrase spoken during the run must have a recorded clip.
  await harvest(); const missing = [...missingAll];
  for (const t of missing) fail(`${name}: spoken with no clip: "${t}"`);
  for (const e of errors) fail(`${name}: console error: ${e}`);
  await browser.close();
}

// Both sizes at once: one after the other took over the check's 300 s limit (each run waits out a 30 s race).
await Promise.all(Object.entries(SIZES).map(([name, vp]) => run(name, vp)));
console.log(failures ? `\n${failures} failure(s)` : '\nall passed', `· shots in ${SHOTS}`);
process.exit(failures ? 1 : 0);
