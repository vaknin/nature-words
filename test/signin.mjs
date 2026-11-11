// Sign-in test: the kid sign-in page (the server's root) and the teacher page (/teacher/), in a real browser.
// The app section checks the teacher page is an installable app (Kitah) by Chrome's own installability check;
// The access section checks the teacher sign-in by Cloudflare Access: it starts BIN with an access.json and a key set
// of its own, mints Cloudflare-shaped tokens itself, and stands in for Cloudflare by adding the token header to every
// browser request; then it restarts BIN without the config and checks the teacher key works again.
// all runs kid and teacher; app and access run on their own (access only with --local).
// Run: cd test && npm ci && node signin.mjs kid|teacher|app|access|all (--local BIN | URL)
//   --local BIN  starts BIN on a free port with a throwaway database, board log, teacher key and pages from server/www
//   URL          uses that server (the live one: the address in ~/.local/state/lesson-server/url) and the teacher key
//                in ~/.config/lesson-server/teacher.key
// Every class it makes is a throwaway one, deleted at the end whether the test passed or not. Exits non-zero on any miss.
import puppeteer from 'puppeteer-core';
import { spawn, execFileSync } from 'child_process';
import { mkdtempSync, readFileSync, rmSync, existsSync, mkdirSync, writeFileSync } from 'fs';
import { generateKeyPairSync, sign as rsaSign } from 'crypto';
import { createServer } from 'net';
import { fileURLToPath } from 'url';
import os from 'os';
import path from 'path';

const here = path.dirname(fileURLToPath(import.meta.url));
const CHROME = process.env.CHROME || '/usr/bin/chromium';
// His word: laptop only. 1366x768 is the common school laptop; 1280x720 the smallest laptop screen in use.
const SIZES = [{ width: 1366, height: 768 }, { width: 1280, height: 720 }];
// The animals' fixed order, the same list as the page's: a kid's stored picture number is a place in it.
const ORDER = ['cat', 'dog', 'bear', 'rabbit', 'fox', 'panda', 'frog', 'pig', 'owl', 'lion', 'mouse', 'koala', 'penguin', 'chick', 'monkey', 'cow'];
// The server's limits (server/src/main.rs): 40 kids a class, 15 characters a name, the lock after 6 wrong tries.
const CLASS_MAX_KIDS = 40, NAME_MAX_CHARS = 15, LOCK_AFTER_WRONG = 6, FIRST_PAUSE_AT = 3;

// ---------- Waits, each from a measured time (the run prints its slowest of each at the end) ----------
// The local server answered /v1/health 13 to 63 ms after it was started (3 runs, measured); 5 s is 80 times the slowest.
const START_MS = 5000;
// A screen change (one API answer and a redraw): slowest seen 163 ms locally (3 runs) and 2.46 s through the live
// tunnel (one run); 8 s is 3 times the slowest live one.
const SCREEN_MS = 8000;
// From "Yes, it's mine!" to the lesson page: the page's own 1.5 s "Hi NAME!" plus one page load, slowest seen 1.68 s
// locally (3 runs) and 2.13 s live (one run); 15 s is 7 times the slowest.
const LAND_MS = 15000;
// A page load (the sign-in page is 180 KB with its fonts): slowest seen 334 ms locally (3 runs) and
// 2.64 s live (one run); 15 s is 5 times the slowest.
const LOAD_MS = 15000;
// The teacher page refreshes its kids every 5 s (its REFRESH_MS); a badge shows within one refresh plus one answer.
// Slowest seen locally 4.96 s (one refresh, as expected); 15 s is three refreshes. A waiting kid's page asks at the
// same pace (its POLL_MS), so the same wait covers an approval or a no reaching the kid.
const REFRESH_WAIT_MS = 15000;
// The teacher app's service worker, from page load to installed, active and in control of the page: slowest seen
// 28 ms locally (3 runs). Through the tunnel it is one more small fetch (sw.js, under 2 KB), like a screen change
// (slowest live 2.46 s above), so it gets the same 8 s, 3 times that.
const SW_MS = SCREEN_MS;
// His phone: 412x915 is the common Android phone viewport (Chrome DevTools' Galaxy and Pixel presets), touch on.
const PHONE = { width: 412, height: 915, isMobile: true, hasTouch: true };
const section = process.argv[2];
if (!['kid', 'teacher', 'app', 'access', 'all'].includes(section)) { console.log('usage: node signin.mjs kid|teacher|app|access|all (--local BIN | URL)'); process.exit(2); }
let base, key, child = null, tmp = null, startLog = '';
let failures = 0;
const fail = m => { failures++; console.log('  FAIL', m); };
const ok = (c, m) => { if (c) console.log('  ok  ', m); else fail(m); };
const timings = {};
const timed = async (what, p) => { const t = Date.now(); try { return await p; } finally { const d = Date.now() - t; timings[what] = Math.max(timings[what] || 0, d); } };

const freePort = () => new Promise((res, rej) => { const s = createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => res(p)); }); s.on('error', rej); });

// LESSON_ACCESS points at a file that isn't there unless `access` gives one, so a test server never picks up the live
// server's own access.json: Access is off unless the access section turns it on.
async function startLocal(bin, access) {
  tmp = mkdtempSync(path.join(os.tmpdir(), 'signin-test-'));
  const port = await freePort();
  child = spawn(path.resolve(bin), [], {
    stdio: ['ignore', 'ignore', 'pipe'],
    env: { ...process.env, LESSON_PORT: String(port), LESSON_DB: path.join(tmp, 'lesson.db'), LESSON_BOARD: path.join(tmp, 'board.log'), LESSON_TEACHER_KEY: path.join(tmp, 'teacher.key'), LESSON_WWW: path.join(here, '..', 'server', 'www'), LESSON_ACCESS: access || path.join(tmp, 'no-access.json') },
  });
  let log = '';
  child.stderr.on('data', d => { log += d; startLog = log; });
  base = `http://127.0.0.1:${port}`;
  const t0 = Date.now();
  while (Date.now() - t0 < START_MS) {
    try { if ((await fetch(base + '/v1/health')).ok) break; } catch {}
    await new Promise(r => setTimeout(r, 2));
  }
  timings['server start'] = Date.now() - t0;
  if (!existsSync(path.join(tmp, 'teacher.key'))) throw new Error('the local server did not start: ' + log);
  key = readFileSync(path.join(tmp, 'teacher.key'), 'utf8').trim();
}

async function api(method, p, body, headers = {}) {
  const r = await fetch(base + p, { method, headers: { ...(body ? { 'Content-Type': 'application/json' } : {}), ...headers }, body: body ? JSON.stringify(body) : undefined, signal: AbortSignal.timeout(SCREEN_MS) });
  let data = null;
  try { data = await r.json(); } catch {}
  return { status: r.status, data };
}
const teach = (method, p, body) => api(method, p, body, { 'X-Teacher-Key': key });

// 40 names in the shape the class uses (first name, family initial), the longest at the server's 15-character cap.
const NAMES = ['Adam S.', 'Noa K.', 'Omer', 'Yael B.', 'Itay', 'Maya L.', 'Ori', 'Tamar', 'Liam R.', 'Shira', 'Eitan', 'Gal M.', 'Dana', 'Yoni',
  'Roni B.', 'Noam', 'Lia', 'Ben', 'Avigail T.', 'Hila', 'Amit D.', 'Yarden', 'Ella', 'Tomer G.', 'Michal', 'Daniel H.', 'Agam', 'Ariel N.',
  'Romi', 'Neta P.', 'Uri', 'Inbar', 'Matan Z.', 'Sapir', 'Elad', 'Hadar W.', 'Lior', 'Yuval C.', 'Maximiliano B.', 'Shoshanna A.'];

// An empty class with joining open: kids join themselves.
async function makeEmptyClass(name) {
  const r = await teach('POST', '/api/classes', { name, students: [] });
  if (r.status !== 200) throw new Error(`could not make a class: ${r.status} ${JSON.stringify(r.data)}`);
  const o = await teach('POST', `/api/classes/${r.data.code}/join`, { open: true });
  if (o.status !== 200) throw new Error(`could not open joining: ${o.status}`);
  return r.data.code;
}
// A name waiting for the teacher in CODE: its id, or undefined.
async function pendingId(code, name) {
  const r = await teach('GET', `/api/classes/${code}/students`);
  return (r.data?.students || []).find(k => k.status === 'pending' && k.name === name)?.id;
}
// NAME joins by the API and the teacher approves it.
async function joinApproved(code, name) {
  const j = await api('POST', '/api/join', { code, name });
  if (j.status !== 200) throw new Error(`${name} could not join: ${j.status} ${JSON.stringify(j.data)}`);
  const a = await teach('POST', `/api/students/${await pendingId(code, j.data.name)}/approve`);
  if (a.status !== 200) throw new Error(`${name} was not approved: ${a.status}`);
}
async function kidsOf(code) {
  const r = await api('GET', '/api/roster/' + code);
  return r.data.students;
}

// ---------- Page checks ----------
async function browserPage(vp, errors) {
  const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage'] });
  const page = await browser.newPage();
  // Touch emulated: the school laptops have touch screens, so every press here is a tap.
  await page.setViewport({ hasTouch: true, ...vp });
  page.on('pageerror', e => errors.push(e.message));
  page.on('console', m => { if (m.type() === 'error' && !/Failed to load resource/.test(m.text())) errors.push(m.text()); });
  return { browser, page };
}
const shown = (page, id, what) => timed(what || id, page.waitForFunction(i => { const s = document.getElementById(i); return s && !s.hidden; }, { timeout: SCREEN_MS }, id));
const textOf = (page, sel) => page.$eval(sel, e => e.innerText);

// Anything on the shown screen outside the window, the page scrolling, or a button under 48x48 px (the common touch
// minimum, Material and WCAG 2.5.5).
function layout(page) {
  return page.evaluate(() => {
    const vis = e => { const r = e.getBoundingClientRect(), s = getComputedStyle(e); return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none'; };
    const out = [];
    const d = document.documentElement;
    if (d.scrollWidth > innerWidth || d.scrollHeight > innerHeight) out.push(`page scrolls: ${d.scrollWidth}x${d.scrollHeight}`);
    for (const e of document.querySelectorAll('body *')) {
      if (e.closest('.party') || e.closest('[hidden]') || e.closest('script,style') || !vis(e)) continue;
      const r = e.getBoundingClientRect();
      // 1 px: sub-pixel rounding.
      if (r.left < -1 || r.top < -1 || r.right > innerWidth + 1 || r.bottom > innerHeight + 1) out.push(`${e.tagName.toLowerCase()}${e.id ? '#' + e.id : ''} "${(e.innerText || '').trim().slice(0, 20)}" off screen`);
      if (e.tagName === 'BUTTON' && (r.width < 47.5 || r.height < 47.5)) out.push(`button "${(e.innerText || '').trim().slice(0, 20)}" ${Math.round(r.width)}x${Math.round(r.height)}, under 48x48`);
      // Text cut off inside its own box.
      if (e.tagName === 'BUTTON' && (e.scrollWidth > e.clientWidth + 1 || e.scrollHeight > e.clientHeight + 1)) out.push(`button "${(e.innerText || '').trim().slice(0, 20)}" text does not fit`);
    }
    return out;
  });
}
// SIGNIN_SHOTS=1 keeps a picture of every screen checked, in test/shots, to look at.
async function checkLayout(page, where) {
  // Let the entry pop end first: a tile caught mid-pop is smaller than it is.
  await page.evaluate(() => Promise.all(document.getAnimations().map(a => a.finished.catch(() => {}))));
  if (process.env.SIGNIN_SHOTS) { mkdirSync(path.join(here, 'shots'), { recursive: true }); await page.screenshot({ path: path.join(here, 'shots', `signin-${where.replace(/\W+/g, '-')}.png`) }); }
  const bad = await layout(page);
  ok(bad.length === 0, `${where}: everything on screen${bad.length ? ': ' + bad.slice(0, 6).join('; ') : ''}`);
}
const tileOrder = page => page.$$eval('#grid .tile svg', s => s.map(x => x.getAttribute('aria-label')));
const tapTile = (page, animal) => page.$eval(`#grid .tile svg[aria-label="${animal}"]`, s => s.closest('button').dataset.pic).then(pic => page.tap(`#grid .tile[data-pic="${pic}"]`));
const tapName = (page, name) => page.$$eval('#names button', (bs, n) => bs.findIndex(b => b.textContent === n), name).then(i => page.tap(`#names button[data-i="${i}"]`));

// ---------- Kid ----------
// The names the kids type through the page, one per laptop size: the second typed small, so the server's tidying
// (capital, the initial's dot) shows on the page.
const TYPED = [['Zohar', 'Zohar'], ['rotem g', 'Rotem G.']];
async function kidSection() {
  console.log('\n== kid sign-in');
  const code = await makeEmptyClass('signin test');
  try {
    // The rest of a full class joins by the API, so the names screen is checked at 40 with the new kids.
    for (const n of NAMES.slice(0, CLASS_MAX_KIDS - SIZES.length)) await joinApproved(code, n);
    const kids = await kidsOf(code);
    ok(kids.length === CLASS_MAX_KIDS - SIZES.length, `an empty class, joining open, ${kids.length} kids joined and approved through the API`);
    const signed = [];
    for (const [n, vp] of SIZES.entries()) signed.push(await kidRun(code, TYPED[n], vp, n));
    await homeRun(code, signed[0], SIZES[0]);
  } finally {
    const r = await teach('DELETE', '/api/classes/' + code);
    ok(r.status === 200, 'the throwaway class is deleted');
  }
}

// Types NAME on the shown join screen and sends it.
async function typeName(page, name) {
  await page.$eval('#join-name', i => { i.value = ''; });
  await page.tap('#join-name');
  await page.keyboard.type(name);
  await page.tap('#join-send');
}

async function kidRun(code, [typed, tidy], vp, n) {
  const at = `${vp.width}x${vp.height}`;
  console.log(`-- ${at}, a new kid types "${typed}"`);
  const errors = [];
  const { browser, page } = await browserPage(vp, errors);
  const mine = ORDER[(5 * n + 4) % 16];
  try {
    await timed('page load', page.goto(base + '/', { waitUntil: 'load', timeout: LOAD_MS }));
    ok(await page.$eval('html', h => h.getAttribute('translate') === 'no' && h.classList.contains('notranslate')), `${at}: html has translate=no and notranslate`);
    await shown(page, 's-code');
    await checkLayout(page, `${at} class code`);
    // A code no class has is refused kindly (asked of the API first, so a real class is never hit).
    if (n === 0) {
      let wrong = code;
      for (let i = 0; i < 10 && (wrong === code || (await api('GET', '/api/roster/' + wrong)).status !== 404); i++) wrong = String((+wrong + 1 + i) % 1e6).padStart(6, '0');
      await page.tap('#code input');
      await page.keyboard.type(wrong);
      await page.tap('#go');
      await timed('screen', page.waitForFunction(() => /No class with this code/.test(document.getElementById('code-msg').innerText), { timeout: SCREEN_MS }));
      ok(true, `${at}: an unknown code says "No class with this code"`);
    }
    await page.tap('#code input');
    await page.keyboard.type('ab' + code);   // letters are dropped; digits fill the boxes one by one
    ok(await page.$$eval('#code input', b => b.map(x => x.value).join('')) === code, `${at}: digits only, each box advances to the next`);
    await page.tap('#go');
    await shown(page, 's-names', 'screen');
    const before = CLASS_MAX_KIDS - SIZES.length + n;
    const chips = await page.$$eval('#names button', bs => bs.map(b => { const r = b.getBoundingClientRect(); return { w: r.width, h: r.height, b: r.bottom, t: r.top }; }));
    ok(chips.length === before, `${at}: ${chips.length} name chips, approved kids only`);
    ok(chips.every(c => c.w >= 47.5 && c.h >= 47.5), `${at}: every name chip at least 48x48 (smallest ${Math.round(Math.min(...chips.map(c => c.w)))}x${Math.round(Math.min(...chips.map(c => c.h)))})`);
    ok(chips.every(c => c.t >= 0 && c.b <= vp.height), `${at}: all ${before} names fit with no scroll`);
    ok(await page.$eval('#i-am-new', b => !b.hidden), `${at}: joining is open, so "I am new" shows`);
    await checkLayout(page, `${at} names`);
    await page.tap('#i-am-new');
    await shown(page, 's-join', 'screen');
    await checkLayout(page, `${at} join`);
    if (n === 0) {
      // Two words: caught on the page before anything is sent.
      await typeName(page, 'Noa Ben');
      await timed('screen', page.waitForFunction(() => /Only your first name/.test(document.getElementById('join-msg').innerText), { timeout: SCREEN_MS }));
      ok(!(await teach('GET', `/api/classes/${code}/students`)).data.students.some(k => k.status === 'pending'), `${at}: a two-word name is refused on the page, nothing sent`);
      // A name the teacher says no to: back to typing, with the message.
      await typeName(page, 'Bobo');
      await shown(page, 's-pending', 'screen');
      const no = await pendingId(code, 'Bobo');
      ok((await teach('POST', `/api/students/${no}/reject`)).status === 200, 'the teacher rejects Bobo (API)');
      await timed('poll', page.waitForFunction(() => !document.getElementById('s-join').hidden && /Your teacher said no\. Type your name again\./.test(document.getElementById('join-msg').innerText), { timeout: REFRESH_WAIT_MS }));
      ok(true, `${at}: a rejected name sees "Your teacher said no. Type your name again."`);
      ok(await page.evaluate(() => localStorage.getItem('lesson-join') === null), `${at}: the rejected ticket is dropped`);
      await checkLayout(page, `${at} join after no`);
    }
    await typeName(page, typed);
    await shown(page, 's-pending', 'screen');
    ok(/Waiting for your teacher to say OK/.test(await textOf(page, '#s-pending')) && (await textOf(page, '#pending-name')) === tidy, `${at}: waits for the teacher as ${tidy}`);
    await checkLayout(page, `${at} pending`);
    const j = await page.evaluate(() => JSON.parse(localStorage.getItem('lesson-join') || 'null'));
    ok(j && j.code === code && /^[0-9a-f]{32}$/.test(j.ticket), `${at}: the ticket is kept on the laptop`);
    if (n === 0) {
      await timed('page load', page.goto(base + '/', { waitUntil: 'load', timeout: LOAD_MS }));
      await shown(page, 's-pending');
      ok(true, `${at}: a reload keeps waiting`);
    }
    ok((await kidsOf(code)).every(k => k.name !== tidy), `${at}: the roster does not list ${tidy} while waiting`);
    const id = await pendingId(code, tidy);
    ok((await teach('POST', `/api/students/${id}/approve`)).status === 200, `the teacher approves ${tidy} (API)`);
    await timed('poll', page.waitForFunction(() => !document.getElementById('s-grid').hidden, { timeout: REFRESH_WAIT_MS }));
    ok((await textOf(page, '#grid-name')) === tidy && /Pick your secret animal/.test(await textOf(page, '#grid-hint')), `${at}: approved, straight to picking an animal as ${tidy}`);
    const tiles = await page.$$eval('#grid .tile', ts => ts.map(t => { const r = t.getBoundingClientRect(); return [r.width, r.height]; }));
    ok(tiles.length === 16 && tiles.every(([w, h]) => w >= 119.5 && h >= 119.5), `${at}: 16 animal tiles, each at least 120 px`);
    await checkLayout(page, `${at} animals`);
    const first = await tileOrder(page);
    await tapTile(page, mine);
    await shown(page, 's-confirm', 'screen');
    const conf = await textOf(page, '#s-confirm');
    ok(/This is your secret animal!/.test(conf) && /Remember it\. Don't tell friends\./.test(conf), `${at}: happy confirm shows`);
    ok(await page.$eval('#big-pick svg', s => s.getAttribute('aria-label')) === mine, `${at}: the confirm shows the tapped animal (${mine})`);
    await checkLayout(page, `${at} confirm`);
    await page.tap('#pick-yes');
    await shown(page, 's-hi', 'screen');
    ok((await textOf(page, '#hi')) === `Hi ${tidy}!`, `${at}: says Hi ${tidy}!`);
    await timed('land', page.waitForFunction(() => location.pathname === '/nature-words/', { timeout: LAND_MS }));
    const sess = await page.evaluate(() => JSON.parse(localStorage.getItem('lesson-session') || 'null'));
    ok(sess && /^[0-9a-f]{32}$/.test(sess.token) && sess.name === tidy && sess.expires > Date.now(), `${at}: lands on the lesson with lesson-session stored`);
    ok(await page.evaluate(() => localStorage.getItem('lesson-join') === null), `${at}: no ticket left behind`);

    // Again: the laptop remembers the class, so the kid starts at the names; the animals are in a new order.
    await timed('page load', page.goto(base + '/', { waitUntil: 'load', timeout: LOAD_MS }));
    await shown(page, 's-names');
    ok((await page.$$eval('#names button', bs => bs.length)) === before + 1, `${at}: the next visit starts at the names, ${tidy} among them`);
    await tapName(page, tidy);
    await shown(page, 's-grid', 'screen');
    ok(/Tap your secret animal/.test(await textOf(page, '#grid-hint')), `${at}: second visit asks for the secret animal`);
    const second = await tileOrder(page);
    ok(second.join() !== first.join(), `${at}: the animals are in a different order`);
    await tapTile(page, mine);
    await timed('land', page.waitForFunction(() => location.pathname === '/nature-words/', { timeout: LAND_MS }));
    ok(true, `${at}: the right animal signs in`);

    // Three wrong animals: "Not this one" with a reshuffled grid, then the wait screen.
    await timed('page load', page.goto(base + '/', { waitUntil: 'load', timeout: LOAD_MS }));
    await shown(page, 's-names');
    await tapName(page, tidy);
    await shown(page, 's-grid', 'screen');
    const wrongs = ORDER.filter(a => a !== mine);
    for (let i = 0; i < FIRST_PAUSE_AT; i++) {
      const before = await tileOrder(page);
      await tapTile(page, wrongs[i]);
      if (i < FIRST_PAUSE_AT - 1) {
        await timed('screen', page.waitForFunction(() => /Not this one/.test(document.getElementById('grid-hint').innerText) && !document.getElementById('grid').classList.contains('busy'), { timeout: SCREEN_MS }));
        const left = FIRST_PAUSE_AT - 1 - i;
        ok(new RegExp(`${left} tr(y|ies) left`).test(await textOf(page, '#grid-hint')), `${at}: wrong try ${i + 1} says "Not this one", ${left} left`);
        ok((await tileOrder(page)).join() !== before.join(), `${at}: the grid reshuffles after a wrong try`);
      }
    }
    await shown(page, 's-wait', 'screen');
    const count = (await textOf(page, '#count')).split(':').reduce((a, x) => a * 60 + +x, 0);
    ok(count > 0 && count <= 60, `${at}: the wait screen counts down from the server's wait (${count} s)`);
    await checkLayout(page, `${at} wait`);
    ok(errors.length === 0, `${at}: no page errors${errors.length ? ': ' + errors.slice(0, 3).join(' | ') : ''}`);
  } catch (e) {
    fail(`${at}: ${e.message.split('\n')[0]}`);
    mkdirSync(path.join(here, 'shots'), { recursive: true });
    await page.screenshot({ path: path.join(here, 'shots', `signin-kid-${at}.png`) }).catch(() => {});
  } finally {
    await browser.close();
  }
  return { name: tidy, animal: mine };
}

// From home: a fresh browser opens the class's home link, joining closed, and the approved kid signs in with the
// animal. The wrong tries above paused her, so the teacher's Unlock ends that pause first.
async function homeRun(code, kid, vp) {
  const at = `${vp.width}x${vp.height}`;
  console.log(`-- ${at}, home: ${kid.name} through the class link, joining closed`);
  ok((await teach('POST', `/api/classes/${code}/join`, { open: false })).status === 200, 'the teacher closes joining (API)');
  const k = (await kidsOf(code)).find(x => x.name === kid.name);
  ok((await teach('POST', `/api/students/${k.id}/unlock`)).status === 200, `the teacher unlocks ${kid.name}'s pause (API)`);
  const errors = [];
  const { browser, page } = await browserPage(vp, errors);
  try {
    await timed('page load', page.goto(`${base}/?class=${code}`, { waitUntil: 'load', timeout: LOAD_MS }));
    await shown(page, 's-names');
    ok(true, `${at}: the home link starts at the class's names`);
    ok(await page.evaluate(c => localStorage.getItem('lesson-class') === c, code), `${at}: the link's class is remembered as this laptop's`);
    ok(await page.$eval('#i-am-new', b => b.hidden), `${at}: joining closed, so no "I am new"`);
    await tapName(page, kid.name);
    await shown(page, 's-grid', 'screen');
    ok(/Tap your secret animal/.test(await textOf(page, '#grid-hint')), `${at}: asks for the secret animal`);
    await tapTile(page, kid.animal);
    await timed('land', page.waitForFunction(() => location.pathname === '/nature-words/', { timeout: LAND_MS }));
    ok(true, `${at}: ${kid.name} signs in from home with the animal`);
    ok(errors.length === 0, `${at}: no page errors${errors.length ? ': ' + errors.slice(0, 3).join(' | ') : ''}`);
  } catch (e) {
    fail(`home ${at}: ${e.message.split('\n')[0]}`);
    mkdirSync(path.join(here, 'shots'), { recursive: true });
    await page.screenshot({ path: path.join(here, 'shots', `signin-home-${at}.png`) }).catch(() => {});
  } finally {
    await browser.close();
  }
}

// ---------- Teacher ----------
// A wrong try made while a pause runs is not counted (the server's rule), so reaching the lock through the API takes
// over 6 minutes of pauses. Against a local server the test owns its database, so it ends each pause there instead of
// waiting (sqlite3); the wrong sign-ins themselves are real. Against a URL it stops at the first pause.
function endPause(kidId) {
  execFileSync('sqlite3', [path.join(tmp, 'lesson.db'), `PRAGMA busy_timeout=${SCREEN_MS}; UPDATE students SET paused_until=0 WHERE id=${Number(kidId)};`]);
}
const kidRow = (page, id) => page.$eval(`#kids tr[data-kid="${id}"]`, r => r.innerText).catch(() => '');

async function teacherSection() {
  console.log('\n== teacher page');
  const vp = SIZES[0], at = `${vp.width}x${vp.height}`;
  const name = 'test ' + Math.random().toString(16).slice(2, 6);
  const kids = ['Noa K.', 'Omer', 'Yael B.', 'Itay', 'Maya L.', 'Ori'];
  const errors = [];
  const { browser, page } = await browserPage(vp, errors);
  let code = null;
  try {
    await timed('page load', page.goto(base + '/teacher/', { waitUntil: 'load', timeout: LOAD_MS }));
    ok(await page.$eval('html', h => h.getAttribute('translate') === 'no' && h.classList.contains('notranslate')), `${at}: html has translate=no and notranslate`);
    await shown(page, 'v-key');
    await page.type('#key', 'not-the-key');
    await page.keyboard.press('Enter');
    await timed('screen', page.waitForFunction(() => /does not work/.test(document.getElementById('key-msg').innerText), { timeout: SCREEN_MS }));
    ok(true, `${at}: a wrong key says "This key does not work"`);
    await page.type('#key', key);
    await page.keyboard.press('Enter');
    await shown(page, 'v-classes', 'screen');
    ok(await page.evaluate(k => localStorage.getItem('lesson-teacher-key') === k, key), `${at}: the right key opens the classes and is remembered`);
    await checkLayout(page, `${at} teacher classes`);

    await page.tap('#new-class');
    await shown(page, 'v-new', 'screen');
    await page.type('#new-name', name);
    await page.type('#new-kids', [...kids, 'Avigailalexandra'].join('\n'));
    ok(/Longer than 15 letters: Avigailalexandra/.test(await textOf(page, '#new-msg')), `${at}: a 16-letter name is caught before sending`);
    await page.$eval('#new-kids', (t, v) => { t.value = v; t.dispatchEvent(new Event('input')); }, kids.join('\n'));
    ok((await textOf(page, '#new-msg')) === '', `${at}: the fixed list has no warning`);
    await checkLayout(page, `${at} teacher new class`);
    await page.tap('#new-make');
    await shown(page, 'v-class', 'screen');
    code = await textOf(page, '#class-code');
    const listed = (await teach('GET', '/api/classes')).data.find(c => c.code === code);
    ok(listed && listed.name === name && listed.kids === kids.length, `${at}: the class made through the page is on the server (${code}, ${kids.length} kids)`);
    await timed('screen', page.waitForFunction(n => document.querySelectorAll('#kids tr').length === n, { timeout: SCREEN_MS }, kids.length));
    await checkLayout(page, `${at} teacher class`);

    await page.tap('#show-proj');
    await shown(page, 'projector', 'screen');
    const digits = await page.$$eval('#proj-digits span', s => s.map(x => x.textContent).join(''));
    ok(digits === code && /Class code/.test(await textOf(page, '#projector')) && /Go to the lesson and type this code/.test(await textOf(page, '#projector')), `${at}: the projector shows the six digits the API gave (${digits})`);
    ok((await textOf(page, '#proj-addr')) === new URL(base).host, `${at}: the projector shows the server's address`);
    await checkLayout(page, `${at} teacher projector`);
    await page.keyboard.press('Escape');
    await timed('screen', page.waitForFunction(() => document.getElementById('projector').hidden, { timeout: SCREEN_MS }));

    // One kid picks an animal, then gets it wrong through the API.
    const kid = (await kidsOf(code)).find(k => k.name === 'Omer');
    const right = 3, wrong = 4;
    ok((await api('POST', '/api/pick', { code, student: kid.id, picture: right })).status === 200, 'the kid picks an animal (API)');
    const signin = pic => api('POST', '/api/signin', { code, student: kid.id, picture: pic });
    for (let i = 0; i < FIRST_PAUSE_AT; i++) await signin(wrong);
    await timed('refresh', page.waitForFunction(id => /Waiting/.test(document.querySelector(`#kids tr[data-kid="${id}"]`)?.innerText || ''), { timeout: REFRESH_WAIT_MS }, kid.id));
    ok(true, `${at}: after ${FIRST_PAUSE_AT} wrong tries the page shows Waiting without a reload`);
    if (tmp) {
      // Local: on to the lock, each pause ended in the test's own database.
      let last = 0;
      for (let i = FIRST_PAUSE_AT; i < LOCK_AFTER_WRONG; i++) { endPause(kid.id); last = (await signin(wrong)).status; }
      ok(last === 423, `the ${LOCK_AFTER_WRONG}th wrong sign-in locks the kid (${last})`);
      await timed('refresh', page.waitForFunction(id => /Locked/.test(document.querySelector(`#kids tr[data-kid="${id}"]`)?.innerText || ''), { timeout: REFRESH_WAIT_MS }, kid.id));
      ok(true, `${at}: the Locked badge shows without a reload`);
      await checkLayout(page, `${at} teacher locked`);
    }
    await page.tap(`#kids button[data-unlock="${kid.id}"]`);
    await timed('screen', page.waitForFunction(id => /Ready/.test(document.querySelector(`#kids tr[data-kid="${id}"]`)?.innerText || ''), { timeout: SCREEN_MS }, kid.id));
    ok(true, `${at}: Unlock clears the badge`);
    const after = await signin(right);
    ok(after.status === 200, `the next sign-in with the right animal answers ${after.status}`);

    // New animal sits behind a confirm: Cancel keeps it, Yes clears it.
    await page.tap(`#kids button[data-reset="${kid.id}"]`);
    await shown(page, 'dialog', 'screen');
    await page.tap('#dialog-no');
    ok((await kidsOf(code)).find(k => k.id === kid.id).picked, `${at}: Cancel on "New animal" keeps the animal`);
    await page.tap(`#kids button[data-reset="${kid.id}"]`);
    await shown(page, 'dialog', 'screen');
    await page.tap('#dialog-yes');
    await timed('screen', page.waitForFunction(id => /No animal yet/.test(document.querySelector(`#kids tr[data-kid="${id}"]`)?.innerText || ''), { timeout: SCREEN_MS }, kid.id));
    ok(!(await kidsOf(code)).find(k => k.id === kid.id).picked, `${at}: Yes on "New animal" clears it`);
    ok(errors.length === 0, `${at}: no page errors${errors.length ? ': ' + errors.slice(0, 3).join(' | ') : ''}`);
  } catch (e) {
    fail(`teacher ${at}: ${e.message.split('\n')[0]}`);
    mkdirSync(path.join(here, 'shots'), { recursive: true });
    await page.screenshot({ path: path.join(here, 'shots', `signin-teacher-${at}.png`) }).catch(() => {});
  } finally {
    await browser.close();
    // The class goes either way; found by its name if the page never showed its code.
    if (!code) code = ((await teach('GET', '/api/classes').catch(() => ({ data: [] }))).data || []).find(c => c.name === name)?.code;
    if (code) ok((await teach('DELETE', '/api/classes/' + code)).status === 200, 'the throwaway class is deleted');
  }
}

// The teacher page as an installable app (Kitah), on a phone: Chrome's own install check through the DevTools protocol
// (what Lighthouse's PWA audit wrapped before Lighthouse 12 dropped it), and the service worker leaving /api alone.
async function appSection() {
  console.log('\n== teacher app (Kitah)');
  const at = `${PHONE.width}x${PHONE.height} phone`;
  const errors = [];
  const { browser, page } = await browserPage(PHONE, errors);
  try {
    await timed('page load', page.goto(base + '/teacher/', { waitUntil: 'load', timeout: LOAD_MS }));
    await timed('service worker', page.waitForFunction(() => navigator.serviceWorker.ready.then(() => true), { timeout: SW_MS }));
    // The first load ran before the worker existed; clients.claim() takes it over, a reload makes sure.
    if (!(await page.evaluate(() => !!navigator.serviceWorker.controller))) await timed('page load', page.reload({ waitUntil: 'load', timeout: LOAD_MS }));
    await timed('service worker', page.waitForFunction(() => !!navigator.serviceWorker.controller, { timeout: SW_MS }));
    ok(await page.evaluate(() => navigator.serviceWorker.controller.scriptURL.endsWith('/teacher/sw.js')), `${at}: the teacher page is controlled by its service worker`);

    const cdp = await page.createCDPSession();
    const inst = await cdp.send('Page.getInstallabilityErrors');
    const errs = (inst.installabilityErrors || []).map(e => e.errorId + (e.errorArguments || []).map(a => ` ${a.name}=${a.value}`).join(''));
    ok(errs.length === 0, `${at}: Chrome's installability check finds no errors${errs.length ? ': ' + errs.join(', ') : ''}`);
    const man = await cdp.send('Page.getAppManifest');
    let parsed = {};
    try { parsed = JSON.parse(man.data || '{}'); } catch {}
    ok(man.url === base + '/teacher/manifest.webmanifest' && (man.errors || []).length === 0, `${at}: the manifest loads with no errors${(man.errors || []).length ? ': ' + man.errors.map(e => e.message).join(', ') : ''}`);
    ok(parsed.name === 'Kitah' && parsed.display === 'standalone', `${at}: the manifest names the app Kitah and opens it standalone`);

    // The page itself came through the worker (proof the check below can see it), /api never does.
    const seen = {};
    page.on('response', r => { const u = new URL(r.url()); seen[u.pathname] = r.fromServiceWorker(); });
    await timed('page load', page.reload({ waitUntil: 'load', timeout: LOAD_MS }));
    ok(seen['/teacher/'] === true, `${at}: the page itself is answered by the service worker (network-first)`);
    const [api] = await timed('screen', Promise.all([
      page.waitForResponse(r => new URL(r.url()).pathname === '/api/classes', { timeout: SCREEN_MS }),
      page.evaluate(() => fetch('/api/classes').then(r => r.status)),
    ]));
    ok(api.fromServiceWorker() === false, `${at}: /api/classes went to the server, not the service worker (status ${api.status()})`);
    ok(errors.length === 0, `${at}: no page errors${errors.length ? ': ' + errors.slice(0, 3).join(' | ') : ''}`);
  } catch (e) {
    fail(`app ${at}: ${e.message.split('\n')[0]}`);
    mkdirSync(path.join(here, 'shots'), { recursive: true });
    await page.screenshot({ path: path.join(here, 'shots', `signin-app-${at.replace(' ', '-')}.png`) }).catch(() => {});
  } finally {
    await browser.close();
  }
}

// ---------- Cloudflare Access: the teacher by Google sign-in only (local only) ----------
// The test plays Cloudflare: its own RSA key set is the team's (in the certs file access.json names), it mints RS256
// tokens in Cloudflare's shape, and the browser sends one in Cf-Access-Jwt-Assertion on every request, as Cloudflare
// adds it at the edge. The page's own requests bypass its service worker here: a worker's fetches don't carry the
// test's extra header, while behind Cloudflare the edge adds it to those too.
const TEAM = 'kitah-test', AUD = 'aud-of-the-test-app', EMAIL = 'teacher@example.com';
const b64url = b => Buffer.from(b).toString('base64url');
let teamKey = null;
function mint(email) {
  const now = Math.floor(Date.now() / 1000);
  const head = b64url(JSON.stringify({ alg: 'RS256', kid: 'test-1', typ: 'JWT' }));
  // An hour, Cloudflare's shortest session choice besides "no duration"; the run takes well under that.
  const body = b64url(JSON.stringify({ iss: `https://${TEAM}.cloudflareaccess.com`, aud: [AUD], email, sub: 'x', iat: now, nbf: now, exp: now + 3600 }));
  return `${head}.${body}.${b64url(rsaSign('sha256', Buffer.from(`${head}.${body}`), teamKey))}`;
}

async function accessSection(bin) {
  console.log('\n== teacher sign-in by Cloudflare Access');
  const cfg = mkdtempSync(path.join(os.tmpdir(), 'signin-access-'));
  const { publicKey, privateKey } = generateKeyPairSync('rsa', { modulusLength: 2048 });
  teamKey = privateKey;
  writeFileSync(path.join(cfg, 'certs.json'), JSON.stringify({ keys: [{ ...publicKey.export({ format: 'jwk' }), kid: 'test-1', alg: 'RS256', use: 'sig' }] }));
  writeFileSync(path.join(cfg, 'access.json'), JSON.stringify({ team: TEAM, aud: AUD, email: EMAIL, certs: path.join(cfg, 'certs.json') }), { mode: 0o600 });
  const vp = PHONE, at = `${vp.width}x${vp.height} phone`;
  const name = 'test ' + Math.random().toString(16).slice(2, 6);
  const good = mint(EMAIL), other = mint('someone.else@example.com');
  const asTeacher = (method, p, body) => api(method, p, body, { 'Cf-Access-Jwt-Assertion': good });
  let code = null;
  try {
    await startLocal(bin, path.join(cfg, 'access.json'));
    ok(/Access on/.test(startLog), `the start line says Access is on: ${startLog.trim().split('\n').pop()}`);
    const page401 = await fetch(base + '/teacher/', { signal: AbortSignal.timeout(SCREEN_MS) });
    ok(page401.status === 401, `/teacher/ with no token answers ${page401.status}`);
    const man = await fetch(base + '/teacher/manifest.webmanifest', { signal: AbortSignal.timeout(SCREEN_MS) });
    ok(man.status === 200, `/teacher/manifest.webmanifest with no token answers ${man.status} (Install still works)`);
    ok((await teach('GET', '/api/classes')).status === 401, 'the teacher key alone gets 401 from /api/classes');
    ok((await api('GET', '/api/classes', null, { 'Cf-Access-Jwt-Assertion': other })).status === 401, 'another email\'s token gets 401');

    const errors = [];
    const { browser, page } = await browserPage(vp, errors);
    try {
      const cdp = await page.createCDPSession();
      await cdp.send('Network.enable');
      await cdp.send('Network.setBypassServiceWorker', { bypass: true });
      // Every time the key box is shown is counted, from the first moment of every page load.
      await page.evaluateOnNewDocument(() => {
        window.__keyShown = 0;
        addEventListener('DOMContentLoaded', () => {
          const k = document.getElementById('v-key');
          const look = () => { if (!k.hidden) window.__keyShown++; };
          new MutationObserver(look).observe(k, { attributes: true, attributeFilter: ['hidden'] });
          look();
        });
      });
      await page.setExtraHTTPHeaders({ 'Cf-Access-Jwt-Assertion': good });
      await timed('page load', page.goto(base + '/teacher/', { waitUntil: 'load', timeout: LOAD_MS }));
      await shown(page, 'v-classes', 'screen');
      ok((await page.evaluate(() => window.__keyShown)) === 0, `${at}: with a good token the page opens on the classes, no key screen`);
      ok((await textOf(page, '#forget')).trim() === 'Sign out', `${at}: the key button reads Sign out`);
      await checkLayout(page, `${at} access classes`);

      await page.tap('#new-class');
      await shown(page, 'v-new', 'screen');
      await page.type('#new-name', name);
      await page.type('#new-kids', 'Noa K.\nOmer');
      await page.tap('#new-make');
      await shown(page, 'v-class', 'screen');
      code = await textOf(page, '#class-code');
      const listed = (await asTeacher('GET', '/api/classes')).data.find(c => c.code === code);
      ok(listed && listed.name === name, `${at}: a class made through the page is on the server (${code})`);

      await page.setExtraHTTPHeaders({ 'Cf-Access-Jwt-Assertion': other });
      // The kids' list refreshes every 5 s (the page's REFRESH_MS): the ended screen comes within one refresh.
      await timed('refresh', page.waitForFunction(() => !document.getElementById('v-ended').hidden, { timeout: REFRESH_WAIT_MS }));
      ok(true, `${at}: with another email's token, the next refresh shows "Your sign-in ended"`);
      await checkLayout(page, `${at} access ended`);
      ok((await page.evaluate(() => window.__keyShown)) === 0, `${at}: the key box never showed`);

      await page.setExtraHTTPHeaders({ 'Cf-Access-Jwt-Assertion': good });
      await page.tap('#sign-in-again');
      await timed('page load', page.waitForNavigation({ waitUntil: 'load', timeout: LOAD_MS }));
      await shown(page, 'v-classes', 'screen');
      ok((await page.evaluate(() => window.__keyShown)) === 0, `${at}: "Sign in again" reloads into the classes`);
      // Deleted by the page's own request (same origin, with its X-Kitah-App header), as its buttons send them.
      const del = await page.evaluate(c => api('DELETE', '/api/classes/' + c).then(r => r.status), code);
      ok(del === 200 && !(await asTeacher('GET', '/api/classes')).data.some(c => c.code === code), `${at}: the class is deleted through the page (${del})`);
      if (del === 200) code = null;
      ok(errors.length === 0, `${at}: no page errors${errors.length ? ': ' + errors.slice(0, 3).join(' | ') : ''}`);
    } catch (e) {
      fail(`access ${at}: ${e.message.split('\n')[0]}`);
      mkdirSync(path.join(here, 'shots'), { recursive: true });
      await page.screenshot({ path: path.join(here, 'shots', `signin-access-${at.replace(' ', '-')}.png`) }).catch(() => {});
    } finally {
      await browser.close();
      if (code) ok((await asTeacher('DELETE', '/api/classes/' + code)).status === 200, 'the throwaway class is deleted');
    }

    // The same binary without the config: the key, as before.
    child.kill();
    rmSync(tmp, { recursive: true, force: true });
    await startLocal(bin);
    ok(/Access off/.test(startLog), `restarted without access.json, the start line says Access is off`);
    const errors2 = [];
    const { browser: b2, page: p2 } = await browserPage(vp, errors2);
    try {
      await timed('page load', p2.goto(base + '/teacher/', { waitUntil: 'load', timeout: LOAD_MS }));
      await shown(p2, 'v-key', 'screen');
      await p2.type('#key', key);
      await p2.keyboard.press('Enter');
      await shown(p2, 'v-classes', 'screen');
      ok((await textOf(p2, '#forget')).trim() === 'Forget key', `${at}: Access off, the key screen opens and the key still works`);
      ok(errors2.length === 0, `${at}: no page errors${errors2.length ? ': ' + errors2.slice(0, 3).join(' | ') : ''}`);
    } catch (e) {
      fail(`access off ${at}: ${e.message.split('\n')[0]}`);
    } finally {
      await b2.close();
    }
  } finally {
    rmSync(cfg, { recursive: true, force: true });
  }
}

// ---------- Main ----------
try {
  const local = process.argv.indexOf('--local');
  if (section === 'access') {
    if (local < 0) throw new Error('access runs only with --local BIN: it starts the server with a key set of its own');
    await accessSection(process.argv[local + 1]);
  } else if (local > 0) await startLocal(process.argv[local + 1]);
  else {
    base = (process.argv[3] || '').replace(/\/+$/, '');
    if (!/^https?:\/\//.test(base)) throw new Error('give --local BIN or the server address');
    key = readFileSync(path.join(os.homedir(), '.config/lesson-server/teacher.key'), 'utf8').trim();
  }
  if (section !== 'access') console.log('server', base);
  if (section === 'kid' || section === 'all') await kidSection();
  if (section === 'teacher' || section === 'all') await teacherSection();
  if (section === 'app') await appSection();
} catch (e) {
  fail(e.message);
} finally {
  if (child) child.kill();
  if (tmp) rmSync(tmp, { recursive: true, force: true });
}
console.log('\nslowest seen (ms):', JSON.stringify(timings));
console.log(failures ? `\n${failures} FAILED` : '\nALL OK');
process.exit(failures ? 1 : 0);
