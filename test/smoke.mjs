// Smoke test: opens every teacher stage and every trail page (#level-1..3, the links posted in Classroom),
// plays the start quiz and every trail game to its end, screenshots each, at two laptop sizes.
// Run: cd test && npm install && timeout 300 node smoke.mjs [URL]   (default: the live site)
import puppeteer from 'puppeteer-core';
import { mkdirSync, rmSync, mkdtempSync, existsSync } from 'fs';
import { spawn } from 'child_process';
import os from 'os';
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
// Race boards: the test never touches the class's board. Each run gets a fresh test topic of its own; the laptop run
// reads our server through board.json (the live site) or ntfy.sh (a local page, where board.json cannot be fetched);
// the small laptop's run points both our server and ntfy.sh at a dead address, to see the offline board.
const testTopic = () => 'nw-smoke-' + Math.random().toString(16).slice(2, 14);
// A closed local port (not one of Chrome's blocked ports, which log their own error).
const DEAD = 'http://127.0.0.1:59999/';
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
  // Touch targets: every visible button at least 48x48 px (the common touch minimum, Material and WCAG 2.5.5) and at
  // least 8 px from its neighbours, so a finger hits the one it means (0.5 px: sub-pixel rounding).
  const small = els.filter(e => e.tagName === 'BUTTON').map(e => [e, e.getBoundingClientRect()]).filter(([, r]) => r.width < 47.5 || r.height < 47.5)
    .map(([e, r]) => `"${(e.textContent.trim() || e.getAttribute('aria-label') || e.className).slice(0, 20)}" ${Math.round(r.width)}x${Math.round(r.height)}`);
  const close = [];
  for (let i = 0; i < els.length; i++) for (let j = i + 1; j < els.length; j++) {
    const a = els[i].getBoundingClientRect(), b = els[j].getBoundingClientRect();
    const dx = Math.max(a.left, b.left) - Math.min(a.right, b.right), dy = Math.max(a.top, b.top) - Math.min(a.bottom, b.bottom);
    const gap = Math.max(dx, dy);
    if (gap > -2 && gap < 7.5) close.push(`"${els[i].textContent.trim().slice(0, 14)}" and "${els[j].textContent.trim().slice(0, 14)}" ${Math.round(gap)}px apart`);
  }
  return { bad, wide, off, small, close };
});

// Each picture on the stage must be its own word's picture: the big picture is the question's word, and each picture
// choice is the word it answers. Also, the listen tile must show a speaker a kid can see (an icon of real size whose
// colour differs from the tile's), not an empty square.
const stageCheck = page => page.evaluate(() => {
  const box = document.querySelector('.stage'); if (!box) return { bad: [] };
  const ans = box.dataset.answer, type = box.dataset.type, bad = [];
  // The page's own pictures, put through the same HTML parser, so markup is compared like for like.
  const norm = h => { const d = document.createElement('div'); d.innerHTML = h; return d.innerHTML; };
  const big = box.querySelector('.big');
  if (big && ans) { if (big.dataset.pic !== ans || big.innerHTML !== norm(ART[ans])) bad.push(`big picture shows ${big.dataset.pic || '?'} for "${ans}"`); }
  for (const b of box.querySelectorAll('.choice.pic')) if (b.innerHTML !== norm(ART[b.dataset.w])) bad.push(`picture choice "${b.dataset.w}" shows another picture`);
  let speaker = null;
  if (type === 'listen') {
    const t = box.querySelector('.speaker'), svg = t && t.querySelector('svg');
    const r = svg ? svg.getBoundingClientRect() : { width: 0, height: 0 };
    const bg = t ? getComputedStyle(t).backgroundColor : '', fill = svg ? getComputedStyle(svg.querySelector('path')).fill : '';
    speaker = { w: r.width, h: r.height, bg, fill, shown: !!svg && getComputedStyle(svg).visibility !== 'hidden' };
  }
  return { bad, speaker, ans, type };
});

async function run(name, vp) {
  console.log(`\n== ${name} ${vp.width}x${vp.height}`);
  const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage', '--autoplay-policy=no-user-gesture-required'] });
  const page = await browser.newPage();
  const topic = testTopic(), offline = name === 'small';
  await page.evaluateOnNewDocument((t, dead) => { window.NW_BOARD_TOPIC = t; if (dead) window.NW_BOARD_URL = window.NW_SERVER_URL = dead; }, topic, offline ? DEAD : '');
  // Touch emulated: the school laptops have touch screens, so every press in this test is a tap.
  await page.setViewport({ ...vp, hasTouch: true });
  // His word: never record. Count every microphone request (and MediaRecorder use) the page makes; any one fails.
  await page.evaluateOnNewDocument(() => {
    window.__micCalls = 0;
    const md = navigator.mediaDevices || {};
    md.getUserMedia = () => { window.__micCalls++; return Promise.reject(new Error('blocked by smoke')); };
    if (!navigator.mediaDevices) Object.defineProperty(navigator, 'mediaDevices', { value: md });
    window.MediaRecorder = function () { window.__micCalls++; throw new Error('blocked by smoke'); };
  });
  let micCalls = 0;
  const errors = [];
  // Offline run: the refused board requests are the point, not errors.
  // A local page cannot fetch board.json (file:// has no fetch): it falls back to ntfy.sh, which is the point too.
  page.on('console', m => { const u = m.location()?.url || ''; if (m.type() === 'error' && !(offline && u.startsWith(DEAD)) && !u.endsWith('board.json')) errors.push(m.text()); });
  page.on('pageerror', e => errors.push(e.message));
  let n = 0;
  const shot = async what => { await sleep(150); await page.screenshot({ path: path.join(SHOTS, `${name}-${String(++n).padStart(2, '0')}-${what}.png`) }); };
  const check = async what => {
    // Let entry animations end first: a button caught mid-pop is smaller than it is.
    await page.evaluate(() => Promise.race([Promise.all(document.getAnimations().filter(a => a.effect?.getTiming().iterations !== Infinity).map(a => a.finished.catch(() => {}))), new Promise(r => setTimeout(r, 1500))]));
    const l = await layout(page);
    for (const b of l.bad) fail(`${name} ${what}: ${b}`);
    ok(l.wide <= 1, `${name} ${what}: page ${l.wide}px wider than the screen`);
    for (const o of l.off) fail(`${name} ${what}: "${o}" off screen`);
    for (const o of l.small) fail(`${name} ${what}: touch targets: ${o}, under 48x48`);
    for (const o of l.close) fail(`${name} ${what}: touch targets: ${o}`);
  };
  // A reload empties window.MISSING, so collect it before every navigation (a test that read it only at the end missed every clip).
  const missingAll = new Set();
  const harvest = async () => { for (const t of await page.evaluate(() => window.MISSING || []).catch(() => [])) missingAll.add(t); micCalls += await page.evaluate(() => window.__micCalls || 0).catch(() => 0); };
  const open = async hash => { await harvest(); console.log('  open', hash); await page.goto(BASE + hash, { waitUntil: 'load' }); await page.evaluate(() => window.dispatchEvent(new HashChangeEvent('hashchange'))); await sleep(120); };
  // A tap on a button the page just replaced (the race moves on every 0.3 s) is a lost tap, not a failure.
  const safeTap = async h => { try { await h.tap(); } catch (e) { if (!/detached|not clickable|not visible|no bounding box/i.test(e.message)) throw e; } };
  const click = async sel => { const h = await page.$(sel); ok(h, `missing ${sel}`); if (h) { await safeTap(h); await sleep(40); } };
  // Tap the first element of sel whose data-w (or letter) passes test.
  const tapWhere = async (sel, f, arg) => { const i = await page.$$eval(sel, (es, src, a) => { const fn = new Function('x', 'a', src); return es.findIndex(x => fn(x, a)); }, f, arg).catch(() => -1);
    if (i < 0) return false; const h = (await page.$$(sel))[i]; if (h) await safeTap(h); return true; };

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
    if (!allRight) { await tapWhere('.choice', 'return x.dataset.w !== a', ans); await sleep(700); continue; }
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
    let lastGame = '', offers = 0, sayChecked = false, lastQ = '', picsSeen = 0, speakers = 0, dragged = false;
    // 200 moves: 3 games x 8 rounds, a long word's 8 letters each, with room; a trail that never ends fails.
    for (let guard = 0; guard < 400 && !(await page.evaluate(() => app.dataset.ended)); guard++) {
      if (await page.$('.offer')) { offers++; await shot(`level-${lv}-offer`); await click('.offer [data-no]'); await sleep(200); continue; }
      const g = await page.$eval('.trail', e => e.dataset.game || '').catch(() => '');
      if (g && g !== lastGame) { lastGame = g; await check(`level ${lv} ${g}`); await shot(`level-${lv}-${g}`); }
      const ans = await page.$eval('.stage', e => e.dataset.answer || '').catch(() => '');
      const type = await page.$eval('.stage', e => e.dataset.type || '').catch(() => '');
      if (ans && `${type}|${ans}` !== lastQ) {
        lastQ = `${type}|${ans}`; picsSeen++;
        const sc = await stageCheck(page);
        for (const b of sc.bad) fail(`${name} level ${lv} ${type}: picture matches its word failed: ${b}`);
        if (sc.speaker) { speakers++;
          ok(sc.speaker.shown && sc.speaker.w >= 32 && sc.speaker.h >= 32 && sc.speaker.bg !== sc.speaker.fill && !/rgba\(0, 0, 0, 0\)/.test(sc.speaker.bg),
            `${name} level ${lv}: listen tile shows a speaker (${JSON.stringify(sc.speaker)})`); }
      }
      if (type === 'say') {
        // Say it out loud, then Next: the prompt is on screen and there is nothing to record.
        if (!sayChecked) { sayChecked = true; ok(/out loud/i.test(await page.$eval('[data-outloud]', e => e.innerText).catch(() => '')), `level ${lv}: no "Say it out loud" prompt`);
          ok(!(await page.$('[data-r], [data-h]')), `level ${lv}: a Record or Hear me button is still there`); }
        await click('[data-n]'); await sleep(250); continue;
      }
      if (type === 'build') {
        const slot = await page.$$eval('.slot', s => s.findIndex(x => !x.textContent));
        if (slot < 0) { await sleep(400); continue; }
        if (!dragged) {
          // Once per place, drag the letter with a finger onto the slots instead of tapping it.
          dragged = true;
          const i = await page.$$eval('.tile', (t, c) => t.findIndex(x => !x.disabled && x.textContent === c), ans[slot]);
          const from = await (await page.$$('.tile'))[i].boundingBox(), to = await (await page.$$('.slot'))[slot].boundingBox();
          const fx = from.x + from.width / 2, fy = from.y + from.height / 2, tx = to.x + to.width / 2, ty = to.y + to.height / 2;
          await page.touchscreen.touchStart(fx, fy);
          for (let k = 1; k <= 8; k++) { await page.touchscreen.touchMove(fx + (tx - fx) * k / 8, fy + (ty - fy) * k / 8); await sleep(16); }
          await page.touchscreen.touchEnd(); await sleep(120);
          ok(await page.$$eval('.slot', (s, k) => s[k].textContent, slot) === ans[slot], `${name} level ${lv}: dragging a letter onto the slots does not place it`);
          continue;
        }
        await tapWhere('.tile', 'return !x.disabled && x.textContent === a', ans[slot]);
        await sleep(30);
      } else if (await page.$(`.choice[data-w="${ans}"]:not(.ok)`)) { await click(`.choice[data-w="${ans}"]`); await sleep(400); }
      else await sleep(300);
    }
    ok(await page.evaluate(() => app.dataset.ended), `level ${lv}: never reached the end`);
    // 30: four games of eight rounds less repeats; fewer means the picture check did not look at the place's rounds.
    ok(picsSeen >= 30, `${name} level ${lv}: picture matches its word checked on only ${picsSeen} rounds`);
    ok(speakers >= 8, `${name} level ${lv}: listen tile shows a speaker checked on only ${speakers} rounds`);
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
      // A top-5 score (any score on an empty test board) asks for a first name, then lights up his row.
      await page.waitForSelector('[data-name], [data-again]', { timeout: 8000 }).catch(() => {});
      ok(await page.$('[data-name]'), `${name}: race board: a first score did not ask for a name`);
      await shot('race-name');
      if (await page.$('[data-name]')) {
        await page.$eval('[data-name]', e => { e.value = ''; });
        await page.type('[data-name]', name === 'laptop' ? 'Noa' : 'Itai');
        await click('[data-save]');
        await page.waitForSelector('.board li.me', { timeout: 8000 }).catch(() => {});
        ok(await page.$eval('.board li.me', e => e.textContent).catch(() => '').then(t => t.includes(name === 'laptop' ? 'Noa' : 'Itai')), `${name}: race board: his row is not lit on the board`);
        ok(await page.$('[data-again]'), `${name}: race board: no Try again after the name`);
        if (offline) ok(await page.$('[data-offline]'), `${name}: race board: offline, the board does not say it is this laptop's own`);
        else ok(!(await page.$('[data-offline]')), `${name}: race board: online, yet the board says offline`);
      }
      await check('race end');
      // His screenshot: Back spilled out of the race's end card. Every button must lie inside the card box.
      const inside = await page.evaluate(() => { const c = document.querySelector('[data-raceend]')?.getBoundingClientRect();
        return [...document.querySelectorAll('[data-raceend] button')].map(b => { const r = b.getBoundingClientRect();
          return { t: b.textContent.trim(), in: !!c && r.left >= c.left - 0.5 && r.right <= c.right + 0.5 && r.top >= c.top - 0.5 && r.bottom <= c.bottom + 0.5 }; }); });
      ok(inside.some(b => /Back/.test(b.t)), `${name}: back inside card: no Back button on the race end`);
      for (const b of inside) ok(b.in, `${name}: back inside card: "${b.t}" spills out of the race end card`);
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
  ok(micCalls === 0, `${name}: no microphone request expected, the page made ${micCalls}`);
  for (const t of missing) fail(`${name}: spoken with no clip: "${t}"`);
  for (const e of errors) fail(`${name}: console error: ${e}`);
  await browser.close();
}

// Our board server on a free port with a fresh file: the 'own server' run reads it. The installed binary (the one the
// class's server runs) when there is one; else built from board/ (cargo can wait minutes behind other builds here).
async function boardServer() {
  const port = 20000 + Math.floor(Math.random() * 20000), dir = mkdtempSync(path.join(os.tmpdir(), 'nw-board-'));
  const bin = process.env.BOARD_BIN || path.join(os.homedir(), '.local', 'bin', 'nature-board');
  const [cmd, args] = existsSync(bin) ? [bin, []] : ['cargo', ['run', '-q', '--manifest-path', path.join(here, '..', 'board', 'Cargo.toml')]];
  const proc = spawn(cmd, args,
    { env: { ...process.env, BOARD_ADDR: `127.0.0.1:${port}`, BOARD_FILE: path.join(dir, 'board.log') }, stdio: ['ignore', 'ignore', 'inherit'] });
  const url = `http://127.0.0.1:${port}/`;
  for (let i = 0; i < 600; i++) {
    if (await fetch(url + 'v1/health').then(r => r.ok, () => false)) return { url, stop: () => proc.kill() };
    await sleep(200);
  }
  proc.kill(); throw new Error('board server did not start');
}

// Race board: two laptops of the class (two browser contexts, nothing shared but the board) race River on one test
// topic; each enters a first name; the second laptop's board must show both. 'own server': the page reads and posts
// our board server, ntfy.sh dead, so both names can only come from our server. 'server down, ntfy answers': our
// server's address is dead, so both names can only come from ntfy.sh.
async function raceBoard(label, server, ntfy) {
  console.log(`\n== race board (two laptops, ${label})`);
  const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage', '--autoplay-policy=no-user-gesture-required'] });
  const topic = testTopic(), names = ['Maya', 'Omer'];
  const one = async (k) => {
    const ctx = await browser.createBrowserContext(), page = await ctx.newPage();
    await page.setViewport({ ...SIZES.laptop, hasTouch: true });
    await page.evaluateOnNewDocument((t, s, n) => { window.NW_BOARD_TOPIC = t; window.NW_SERVER_URL = s; if (n) window.NW_BOARD_URL = n; }, topic, server, ntfy);
    await page.goto(BASE, { waitUntil: 'load' });
    // A kid who has done River's Last check: the end screen with the race card.
    await page.evaluate(() => localStorage.setItem('nature-trails-' + new Date().toISOString().slice(0, 10), JSON.stringify({ trail: 1, game: 4, checked: 4, revealAt: Date.now() })));
    await page.goto(BASE + '#level-1', { waitUntil: 'load' });
    await page.waitForSelector('[data-race]', { timeout: 5000 });
    await (await page.$('[data-race]')).tap();
    for (let guard = 0; guard < 300 && !(await page.$('[data-raceend]')); guard++) {
      const ans = await page.$eval('.stage', e => e.dataset.type === 'race' ? e.dataset.answer : '').catch(() => '');
      const h = ans && await page.$(`.choice[data-w="${ans}"]:not(.ok)`);
      if (h) await h.tap().catch(() => {});
      await sleep(200);
    }
    await page.waitForSelector('[data-name]', { timeout: 8000 }).catch(() => {});
    if (!(await page.$('[data-name]'))) { fail(`race board: laptop ${k + 1} was not asked for a name (${JSON.stringify(await page.$eval('[data-raceend]', e => e.innerText).catch(() => 'no end card'))})`); await ctx.close(); return []; }
    // The second laptop saves after the first one's score is on the board.
    if (k === 1) await sleep(5000);
    await page.type('[data-name]', names[k]); await (await page.$('[data-save]')).tap();
    await page.waitForSelector('.board li.me', { timeout: 8000 }).catch(() => {});
    await sleep(300);
    const rows = await page.$$eval('.board li', ls => ls.map(l => l.textContent.trim()));
    await page.screenshot({ path: path.join(SHOTS, `race-board-${label.split(',')[0].replace(/ /g, '-')}-laptop${k + 1}.png`) });
    return rows;
  };
  const [a, b] = await Promise.all([one(0), one(1)]);
  console.log('  laptop 1 board:', JSON.stringify(a), '\n  laptop 2 board:', JSON.stringify(b));
  ok(b.some(r => r.includes('Maya')) && b.some(r => r.includes('Omer')), `race board (${label}): the second laptop's board does not show both names (${JSON.stringify(b)})`);
  await browser.close();
}

// Both sizes and the race board at once: one after the other took over the check's time limit (each waits out a 30 s race).
// SMOKE_ONLY=board runs the race board alone (to look at it quickly); the check always runs everything.
const own = await boardServer();
const boards = [raceBoard('own server', own.url, DEAD), raceBoard('server down, ntfy answers', DEAD, '')];
await Promise.all(process.env.SMOKE_ONLY === 'board' ? boards : [...Object.entries(SIZES).map(([name, vp]) => run(name, vp)), ...boards]);
own.stop();
console.log(failures ? `\n${failures} failure(s)` : '\nall passed', `· shots in ${SHOTS}`);
process.exit(failures ? 1 : 0);
