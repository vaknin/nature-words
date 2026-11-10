// Design check: the start page's Full screen button and hint, and the trail pages' look (clean header, no greyed
// "Hear me", the Hebrew word in a pill, the embedded fonts, English voices only), at two laptop sizes.
// Run: cd test && npm install && timeout 150 node design.mjs start|trail|all|screens [URL]   (default URL: the live site)
// screens: every kid screen and projector stage at 1366x768 into design/screens, for the polish review (design/review.md).
// Screenshots go to test/shots-design (smoke.mjs empties test/shots when it starts, so they live apart).
import puppeteer from 'puppeteer-core';
import { mkdirSync, rmSync, readdirSync } from 'fs';
import { fileURLToPath } from 'url';
import path from 'path';

const here = path.dirname(fileURLToPath(import.meta.url));
const SECTION = process.argv[2] || 'all';
if (!['start', 'trail', 'all', 'screens'].includes(SECTION)) { console.log('section must be start, trail, all or screens'); process.exit(2); }
const BASE = (process.argv[3] || 'https://vaknin.github.io/nature-words/').replace(/#.*$/, '');
const SHOTS = path.join(here, 'shots-design');
rmSync(SHOTS, { recursive: true, force: true }); mkdirSync(SHOTS, { recursive: true });
const CHROME = process.env.CHROME || '/usr/bin/chromium';
// The same two sizes as smoke.mjs: 1366x768 the common school laptop, 1280x720 the smallest laptop screen in use.
const SIZES = { laptop: { width: 1366, height: 768 }, small: { width: 1280, height: 720 } };
const HEB = /[֐-׿]/;

const sleep = ms => new Promise(r => setTimeout(r, ms));
let failures = 0;
const fail = m => { failures++; console.log('  FAIL', m); };
const ok = (c, m) => { if (!c) fail(m); else console.log('  ok', m); };

async function run(name, vp) {
  console.log(`\n== ${name} ${vp.width}x${vp.height}`);
  const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage', '--autoplay-policy=no-user-gesture-required'] });
  const page = await browser.newPage();
  await page.setViewport(vp);
  // Count Full screen requests, then let the real one run (headless Chromium can enter full screen).
  await page.evaluateOnNewDocument(() => {
    const real = Element.prototype.requestFullscreen;
    window.__fsCalls = 0;
    Element.prototype.requestFullscreen = function (...a) { window.__fsCalls++; return real ? real.apply(this, a).catch(() => {}) : Promise.resolve(); };
  });
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  const shot = async what => { await sleep(300); await page.screenshot({ path: path.join(SHOTS, `design-${name}-${what}.png`) }); };
  const open = async hash => { await page.goto(BASE + hash, { waitUntil: 'load' }); await page.evaluate(() => window.dispatchEvent(new HashChangeEvent('hashchange'))); await sleep(200); };
  const onScreen = sel => page.$eval(sel, e => { const r = e.getBoundingClientRect(), s = getComputedStyle(e);
    return s.display !== 'none' && s.visibility !== 'hidden' && r.width > 0 && r.top >= 0 && r.left >= 0 && r.bottom <= innerHeight && r.right <= innerWidth; }).catch(() => false);

  if (SECTION !== 'trail') {
    // A new kid (Start) and a kid coming back (Continue) both get the button and the hint.
    for (const who of ['new', 'back']) {
      await open('');
      if (who === 'new') await page.evaluate(() => localStorage.clear());
      else await page.evaluate(() => localStorage.setItem('nature-trails-' + new Date().toISOString().slice(0, 10), JSON.stringify({ trail: 2, game: 1 })));
      await open('');
      const go = who === 'new' ? '[data-go]' : '[data-resume]';
      ok(await onScreen('[data-fs-start]'), `${name} ${who}: Full screen button visible and fully on screen`);
      ok(await onScreen(go), `${name} ${who}: ${go} still fully on screen with the hint`);
      ok(/Full screen/.test(await page.$eval('[data-fs-start]', e => e.textContent).catch(() => '')) && await page.$('[data-fs-start] svg'), `${name} ${who}: the button has its picture and the words "Full screen"`);
      const hint = await page.$eval('[data-fs-hint]', e => ({ svg: !!e.querySelector('svg'), text: e.innerText,
        he: [...e.querySelectorAll('*')].some(x => x.children.length === 0 && /[֐-׿]/.test(x.textContent) && x.closest('[dir=rtl]')) })).catch(() => null);
      ok(hint && hint.svg, `${name} ${who}: the hint has a picture`);
      ok(hint && /[A-Za-z]{3,}.*!/.test(hint.text), `${name} ${who}: the hint has an English line`);
      ok(hint && hint.he, `${name} ${who}: the hint has a right-to-left Hebrew line`);
      await shot(`start-${who}`);
      const before = await page.evaluate(() => window.__fsCalls);
      await page.click('[data-fs-start]'); await sleep(400);
      ok(await page.evaluate(() => window.__fsCalls) > before, `${name} ${who}: pressing Full screen asks for full screen`);
      if (await page.evaluate(() => !!document.fullscreenElement)) {
        ok(!(await onScreen('[data-fs-start]')) && !(await onScreen('[data-fs-hint]')), `${name} ${who}: button and hint hide once full screen`);
        await shot(`start-${who}-fullscreen`);
        await page.evaluate(() => document.exitFullscreen()); await sleep(300);
      }
    }
  }

  if (SECTION !== 'start') {
    await open(''); await page.evaluate(() => localStorage.clear());
    for (const lv of [1, 2, 3]) {
      await open(`#level-${lv}`);
      const head = await page.$eval('.thead', e => e.innerText).catch(() => '');
      ok(head && !/LV|x1|after/.test(head), `${name} level ${lv}: header has no "LV", "x1" or "after" (${JSON.stringify(head.replace(/\s+/g, ' '))})`);
      ok(await page.$('[data-chip][disabled]'), `${name} level ${lv}: the race chip is there and locked`);
      ok(await page.evaluate(() => /[֐-׿]/.test(document.querySelector('[data-chip]')?.title || '')), `${name} level ${lv}: the locked chip has its Hebrew tooltip`);
      ok(await page.$eval('.trail', e => e.dataset.game).catch(() => '') === 'say', `${name} level ${lv}: opens on the say game`);
      ok(!(await page.$$eval('[data-h]', hs => hs.some(h => { const r = h.getBoundingClientRect(), s = getComputedStyle(h); return !h.hidden && s.display !== 'none' && s.visibility !== 'hidden' && r.width > 0; }))), `${name} level ${lv}: no "Hear me" before a recording`);
      const bg = await page.$eval('.heword', e => getComputedStyle(e).backgroundColor).catch(() => '');
      ok(bg && bg !== 'transparent' && !/rgba\(0, 0, 0, 0\)/.test(bg), `${name} level ${lv}: the Hebrew word sits in a coloured pill (${bg})`);
      // The pill sits right under its English word, centred on it.
      const tie = await page.evaluate(() => { const a = document.querySelector('.sayline')?.getBoundingClientRect(), b = document.querySelector('.heword')?.getBoundingClientRect();
        return a && b ? { gap: b.top - a.bottom, off: Math.abs((a.left + a.right) / 2 - (b.left + b.right) / 2) } : null; });
      // 24px: closer than the gap between any two other blocks on the stage, so the eye ties them; 4px: centring rounding.
      ok(tie && tie.gap >= -2 && tie.gap <= 24 && tie.off <= 4, `${name} level ${lv}: the Hebrew pill is right under the English (${JSON.stringify(tie)})`);
      // Record, Hear me and Next are one row: same top line.
      const row = await page.$$eval('.rec > *', es => es.filter(e => getComputedStyle(e).display !== 'none' && !e.hidden).map(e => Math.round(e.getBoundingClientRect().top + e.getBoundingClientRect().height / 2)));
      ok(row.length >= 2 && Math.max(...row) - Math.min(...row) <= 2, `${name} level ${lv}: the say buttons sit in one row (${row})`);
      await shot(`level-${lv}`);
    }
    // Fonts: document.fonts.check is true even for a family with no face at all, so each family must also have a
    // loaded face and draw text at a different width than the browser's fallback.
    const fonts = await page.evaluate(async () => {
      await Promise.all([document.fonts.load('20px Fredoka', 'Nature'), document.fonts.load('20px Rubik', 'שלום'), document.fonts.load('20px Rubik', 'Nature')]);
      const loaded = fam => [...document.fonts].some(f => f.family.replace(/"/g, '') === fam && f.status === 'loaded');
      const c = document.createElement('canvas').getContext('2d');
      const w = (font, t) => { c.font = font; return c.measureText(t).width; };
      return {
        fredoka: document.fonts.check('20px Fredoka', 'Nature') && loaded('Fredoka') && w('40px Fredoka, monospace', 'Nature words') !== w('40px monospace', 'Nature words'),
        rubikHe: document.fonts.check('20px Rubik', 'שלום') && loaded('Rubik') && w('40px Rubik, monospace', 'מילים של טבע') !== w('40px monospace', 'מילים של טבע')
      };
    });
    ok(fonts.fredoka, `${name}: Fredoka is embedded and loaded`);
    ok(fonts.rubikHe, `${name}: Rubik is embedded, loaded and draws Hebrew`);
    // His standing word: English voices only. No clip and nothing spoken may be Hebrew.
    const voices = await page.evaluate(() => ({ clips: Object.keys(CLIPS).filter(k => /[֐-׿]/.test(k)), spoken: (window.SPOKEN || []).filter(t => /[֐-׿]/.test(t)), n: (window.SPOKEN || []).length }));
    ok(!voices.clips.length, `${name}: no Hebrew clip (${voices.clips})`);
    ok(!voices.spoken.length, `${name}: nothing Hebrew spoken (${voices.spoken})`);
  }
  for (const e of errors) fail(`${name}: page error: ${e}`);
  await browser.close();
}


// ---- every screen, for the polish review: design/screens/NN-name.png at 1366x768 (the common school laptop).
// Reached the way a kid reaches them where that is quick, else by calling the page's own screen functions.
async function screens() {
  const OUT = path.join(here, '..', 'design', 'screens');
  mkdirSync(OUT, { recursive: true });
  for (const f of readdirSync(OUT)) if (f.endsWith('.png')) rmSync(path.join(OUT, f));
  const vp = SIZES.laptop;
  const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage', '--autoplay-policy=no-user-gesture-required'] });
  const page = await browser.newPage();
  await page.setViewport({ ...vp, hasTouch: true });
  // A test board on ntfy.sh (never the class one), filled with a few made-up names so the boards look lived in.
  const topic = 'nw-design-' + Math.random().toString(16).slice(2, 14);
  const seed = [['Noa', 14, 1], ['Daniel', 11, 1], ['Yael', 9, 1], ['Ori', 7, 1], ['Tamar', 19, 3], ['Lior', 17, 3], ['Adam', 16, 3], ['Shira', 15, 3], ['Eitan', 13, 3]];
  for (const [n, s, p] of seed) await fetch('https://ntfy.sh/' + topic, { method: 'POST', body: JSON.stringify({ n, s, p, t: Date.now() }) }).catch(() => {});
  await sleep(1500);
  await page.evaluateOnNewDocument(t => { window.NW_BOARD_TOPIC = t; }, topic);
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  let n = 0;
  const shot = async (what, wait = 650) => { await sleep(wait); await page.screenshot({ path: path.join(OUT, `${String(++n).padStart(2, '0')}-${what}.png`) }); console.log('  shot', `${String(n).padStart(2, '0')}-${what}`); };
  const KEY = "'nature-trails-' + new Date().toISOString().slice(0, 10)";
  const setStore = o => page.evaluate(new Function('o', `localStorage.clear(); localStorage.setItem(${KEY}, JSON.stringify(o));`), o);
  const open = async hash => { await page.goto(BASE + hash, { waitUntil: 'load' }); await page.evaluate(() => window.dispatchEvent(new HashChangeEvent('hashchange'))); await sleep(200); };
  const tap = async sel => { const h = await page.$(sel); if (h) await h.tap(); else fail(`screens: missing ${sel}`); };
  const answer = async () => { const a = await page.$eval('.stage', e => e.dataset.answer || '').catch(() => ''); if (a) await tap(`.choice[data-w="${a}"]`); };

  await open(''); await setStore({}); await open('');
  await shot('start-new');
  await setStore({ trail: 2, game: 1 }); await open('');
  await shot('start-continue');
  await setStore({}); await open(''); await tap('[data-go]');
  await shot('quiz-listen-picture');
  for (let k = 0; k < 8; k++) {
    if (k === 2) await shot('quiz-picture-word');
    if (k === 4) await shot('quiz-listen-word');
    if (k === 6) await shot('quiz-sentence-gap');
    await answer(); await sleep(1400);
  }
  await shot('reveal-mountain', 900);

  const names = { 1: 'river', 2: 'forest', 3: 'mountain' };
  for (const lv of [1, 2, 3]) {
    const games = await page.evaluate(l => LESSON.games[l].map(g => g.type), lv);
    for (let gi = 0; gi < games.length; gi++) {
      await setStore({ trail: lv, game: gi, revealAt: Date.now(), points: 60 * gi + 20 * lv });
      await open(`#level-${lv}`);
      if (games[gi] === 'build') { await page.evaluate(() => { const t = document.querySelector('.tile'); t && t.closest('.stage').dataset.answer[0] === t.textContent && t.click(); }); }
      await shot(`${names[lv]}-${gi + 1}-${games[gi]}`);
    }
  }
  // Offers: up after a run of first tries, down after misses; the flag in the header after 11 minutes.
  await setStore({ trail: 1, game: 1, revealAt: Date.now() }); await open('#level-1');
  await page.evaluate(() => offer(2, T.up, T.heUp, 1, () => {})); await shot('offer-up-forest');
  await setStore({ trail: 2, game: 2, revealAt: Date.now() }); await open('#level-2');
  await page.evaluate(() => offer(1, T.down, T.heDown, 2)); await shot('offer-down-river');
  await setStore({ trail: 2, game: 2, revealAt: Date.now() - 12 * 60000, points: 180 }); await open('#level-2');
  await shot('flag-in-header');
  // The locked race chip, tapped: the toast says when it opens (it used to be a hover-only tooltip).
  const chip = await page.$eval('[data-chip]', e => { const r = e.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; });
  await page.touchscreen.tap(chip.x, chip.y); await shot('locked-chip-tapped', 300);
  await tap('[data-flag]'); await shot('last-check');
  await setStore({ trail: 1, game: 4, checked: 3, revealAt: Date.now(), points: 240, best: 0 }); await open('#level-1');
  await shot('end-river-race-card', 1800);
  // A short race for the screenshots (6 s instead of 30): the countdown, the race, a name for the board, the board.
  await page.evaluate(() => { LESSON.race.seconds = 6; });
  await tap('[data-race]'); await shot('race-countdown', 300);
  await sleep(2600); await shot('race-running', 100);
  for (let g = 0; g < 40 && !(await page.$('[data-raceend]')); g++) { await answer(); await sleep(250); }
  await page.waitForSelector('[data-name]', { timeout: 8000 }).catch(() => fail('screens: no name entry after a top-5 race'));
  await shot('race-name-entry', 700);
  await page.type('[data-name]', 'Gal'); await tap('[data-save]');
  await page.waitForSelector('.board li.me', { timeout: 8000 }).catch(() => fail('screens: no lit row on the board'));
  await shot('race-board-new-row', 900);
  // Mountain's board is full of high scores: a short race that does not make the top 5 shows the board, no name.
  await setStore({ trail: 3, game: 4, checked: 4, revealAt: Date.now(), points: 300 }); await open('#level-3');
  await page.evaluate(() => { LESSON.race.seconds = 4; });
  await sleep(1500); await tap('[data-race]');
  await page.waitForSelector('[data-again]', { timeout: 15000 }).catch(() => fail('screens: no Try again after a race off the board'));
  await shot('race-end-not-top5', 900);
  // No internet: the laptop's own board, and it says so.
  await page.evaluate(() => { window.NW_BOARD_URL = 'http://127.0.0.1:9/'; localStorage.removeItem('nature-board-' + boardTopic()); });
  await tap('[data-again]');
  await page.waitForSelector('[data-again]', { timeout: 15000 }).catch(() => {});
  await shot('race-end-offline', 900);
  await setStore({ trail: 3, game: 4, checked: 4, revealAt: Date.now(), points: 420 }); await open('#level-3');
  await shot('end-mountain-last-place', 1500);

  // The projector: every stage, and the steps inside the ones that have them.
  await open('#teacher');
  const press = async (k, what, wait = 500) => { await page.keyboard.press(k); if (what) await shot(what, wait); };
  await press('1', 'projector-1-welcome');
  await press('2', 'projector-2-meet-word');
  await press(' ', 'projector-2-meet-word-shown');
  for (let i = 0; i < 8; i++) await press('ArrowRight');
  await shot('projector-2-meet-all-pictures');
  await press('3', 'projector-3-quiz-time');
  await press('4', 'projector-4-play-and-race');
  await press('5', 'projector-5-lets-go-two');
  await press('ArrowRight'); await press('ArrowRight', 'projector-5-lets-go-four');
  await press('6', 'projector-6-whats-missing');
  await press(' ', 'projector-6-one-hidden');
  for (let i = 0; i < 4; i++) await press('ArrowRight');
  await shot('projector-6-summary');
  // The teacher's plan, restyled to match the site.
  await page.goto(new URL('plan.html', BASE).href, { waitUntil: 'load' }); await shot('plan-top', 400);
  await page.evaluate(() => document.getElementById('stage-4').scrollIntoView()); await shot('plan-stage-4', 300);
  for (const e of errors) fail(`screens: page error: ${e}`);
  await browser.close();
  console.log(`  ${n} screens in ${OUT}`);
}

if (SECTION === 'screens') await screens();
else await Promise.all(Object.entries(SIZES).map(([name, vp]) => run(name, vp)));
console.log(failures ? `\n${failures} failure(s)` : '\nall passed', `· shots in ${SHOTS}`);
process.exit(failures ? 1 : 0);
