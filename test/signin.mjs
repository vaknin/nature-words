// Sign-in test: the kid sign-in page (the server's root) and the teacher page (/teacher/), in a real browser.
// Run: cd test && npm ci && node signin.mjs kid|teacher|all (--local BIN | URL)
//   --local BIN  starts BIN on a free port with a throwaway database, board log, teacher key and pages from server/www
//   URL          uses that server (the live one: the address in ~/.local/state/lesson-server/url) and the teacher key
//                in ~/.config/lesson-server/teacher.key
// Every class it makes is a throwaway one, deleted at the end whether the test passed or not. Exits non-zero on any miss.
import puppeteer from 'puppeteer-core';
import { spawn, execFileSync } from 'child_process';
import { mkdtempSync, readFileSync, rmSync, existsSync, mkdirSync } from 'fs';
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
// Slowest seen locally 4.96 s (one refresh, as expected); 15 s is three refreshes.
const REFRESH_WAIT_MS = 15000;
const section = process.argv[2];
if (!['kid', 'teacher', 'all'].includes(section)) { console.log('usage: node signin.mjs kid|teacher|all (--local BIN | URL)'); process.exit(2); }
let base, key, child = null, tmp = null;
let failures = 0;
const fail = m => { failures++; console.log('  FAIL', m); };
const ok = (c, m) => { if (c) console.log('  ok  ', m); else fail(m); };
const timings = {};
const timed = async (what, p) => { const t = Date.now(); try { return await p; } finally { const d = Date.now() - t; timings[what] = Math.max(timings[what] || 0, d); } };

const freePort = () => new Promise((res, rej) => { const s = createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => res(p)); }); s.on('error', rej); });

async function startLocal(bin) {
  tmp = mkdtempSync(path.join(os.tmpdir(), 'signin-test-'));
  const port = await freePort();
  child = spawn(path.resolve(bin), [], {
    stdio: ['ignore', 'ignore', 'pipe'],
    env: { ...process.env, LESSON_PORT: String(port), LESSON_DB: path.join(tmp, 'lesson.db'), LESSON_BOARD: path.join(tmp, 'board.log'), LESSON_TEACHER_KEY: path.join(tmp, 'teacher.key'), LESSON_WWW: path.join(here, '..', 'server', 'www') },
  });
  let log = '';
  child.stderr.on('data', d => log += d);
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

async function makeClass(name) {
  const r = await teach('POST', '/api/classes', { name, students: NAMES.slice(0, CLASS_MAX_KIDS) });
  if (r.status !== 200) throw new Error(`could not make a class: ${r.status} ${JSON.stringify(r.data)}`);
  return r.data.code;
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
  await page.setViewport({ ...vp, hasTouch: true });
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
async function kidSection() {
  console.log('\n== kid sign-in');
  const code = await makeClass('signin test');
  try {
    const kids = await kidsOf(code);
    ok(kids.length === CLASS_MAX_KIDS, `a ${CLASS_MAX_KIDS}-kid class made through the API`);
    for (const [n, vp] of SIZES.entries()) await kidRun(code, kids[n * 7], vp, n);
  } finally {
    const r = await teach('DELETE', '/api/classes/' + code);
    ok(r.status === 200, 'the throwaway class is deleted');
  }
}

async function kidRun(code, kid, vp, n) {
  const at = `${vp.width}x${vp.height}`;
  console.log(`-- ${at}, ${kid.name}`);
  const errors = [];
  const { browser, page } = await browserPage(vp, errors);
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
    const chips = await page.$$eval('#names button', bs => bs.map(b => { const r = b.getBoundingClientRect(); return { w: r.width, h: r.height, b: r.bottom, t: r.top }; }));
    ok(chips.length === CLASS_MAX_KIDS, `${at}: ${chips.length} name chips`);
    ok(chips.every(c => c.w >= 47.5 && c.h >= 47.5), `${at}: every name chip at least 48x48 (smallest ${Math.round(Math.min(...chips.map(c => c.w)))}x${Math.round(Math.min(...chips.map(c => c.h)))})`);
    ok(chips.every(c => c.t >= 0 && c.b <= vp.height), `${at}: all ${CLASS_MAX_KIDS} names fit with no scroll`);
    await checkLayout(page, `${at} names`);
    await tapName(page, kid.name);
    await shown(page, 's-grid', 'screen');
    const tiles = await page.$$eval('#grid .tile', ts => ts.map(t => { const r = t.getBoundingClientRect(); return [r.width, r.height]; }));
    ok(tiles.length === 16 && tiles.every(([w, h]) => w >= 119.5 && h >= 119.5), `${at}: 16 animal tiles, each at least 120 px`);
    ok(/Pick your secret animal/.test(await textOf(page, '#grid-hint')), `${at}: first visit asks to pick an animal`);
    await checkLayout(page, `${at} animals`);
    const first = await tileOrder(page);
    const mine = ORDER[(5 * n + 4) % 16];
    await tapTile(page, mine);
    await shown(page, 's-confirm', 'screen');
    const conf = await textOf(page, '#s-confirm');
    ok(/This is your secret animal!/.test(conf) && /Remember it\. Don't tell friends\./.test(conf), `${at}: happy confirm shows`);
    ok(await page.$eval('#big-pick svg', s => s.getAttribute('aria-label')) === mine, `${at}: the confirm shows the tapped animal (${mine})`);
    await checkLayout(page, `${at} confirm`);
    await page.tap('#pick-yes');
    await shown(page, 's-hi', 'screen');
    ok((await textOf(page, '#hi')) === `Hi ${kid.name}!`, `${at}: says Hi ${kid.name}!`);
    await timed('land', page.waitForFunction(() => location.pathname === '/nature-words/', { timeout: LAND_MS }));
    const sess = await page.evaluate(() => JSON.parse(localStorage.getItem('lesson-session') || 'null'));
    ok(sess && /^[0-9a-f]{32}$/.test(sess.token) && sess.name === kid.name && sess.expires > Date.now(), `${at}: lands on the lesson with lesson-session stored`);

    // Again: the laptop remembers the class, so the kid starts at the names; the animals are in a new order.
    await timed('page load', page.goto(base + '/', { waitUntil: 'load', timeout: LOAD_MS }));
    await shown(page, 's-names');
    ok(true, `${at}: the next visit starts at the names (class remembered)`);
    await tapName(page, kid.name);
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
    await tapName(page, kid.name);
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

// ---------- Main ----------
try {
  const local = process.argv.indexOf('--local');
  if (local > 0) await startLocal(process.argv[local + 1]);
  else {
    base = (process.argv[3] || '').replace(/\/+$/, '');
    if (!/^https?:\/\//.test(base)) throw new Error('give --local BIN or the server address');
    key = readFileSync(path.join(os.homedir(), '.config/lesson-server/teacher.key'), 'utf8').trim();
  }
  console.log('server', base);
  if (section === 'kid' || section === 'all') await kidSection();
  if (section === 'teacher' || section === 'all') await teacherSection();
} catch (e) {
  fail(e.message);
} finally {
  if (child) child.kill();
  if (tmp) rmSync(tmp, { recursive: true, force: true });
}
console.log('\nslowest seen (ms):', JSON.stringify(timings));
console.log(failures ? `\n${failures} FAILED` : '\nALL OK');
process.exit(failures ? 1 : 0);
