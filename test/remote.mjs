// Phone remote test: the projected lesson (nature-words/#teacher-N) moved from the teacher's phone, through the server.
// Run: cd test && npm ci && node remote.mjs projector|phone|all (--local BIN | URL)
//   --local BIN  starts BIN on a free port with a throwaway database, board log and teacher key, serving a temp pages
//                folder: nature-words/index.html from ../index.html, the sign-in and teacher/ from ../server/www
//   URL          uses that server (the live one: the address in ~/.local/state/lesson-server/url) and the teacher key
//                in ~/.config/lesson-server/teacher.key
// Nothing it makes is kept: screens live in the server's memory only and are dropped when unused. Exits non-zero on
// any miss, and prints the slowest command-to-move time.
import puppeteer from 'puppeteer-core';
import { spawn } from 'child_process';
import { mkdtempSync, readFileSync, rmSync, existsSync, mkdirSync, cpSync } from 'fs';
import { createServer } from 'net';
import { fileURLToPath } from 'url';
import os from 'os';
import path from 'path';

const here = path.dirname(fileURLToPath(import.meta.url));
const CHROME = process.env.CHROME || '/usr/bin/chromium';
// The projector: 1366x768 is the common school laptop; 1280x720 the smallest laptop screen in use (as test/signin.mjs).
const PROJ = [{ width: 1366, height: 768 }, { width: 1280, height: 720 }];
// The phone: 412x915 and 360x800, the two most common Android viewports (StatCounter, mobile screen resolution
// stats, 2025: 360x800 first among Android sizes, 412x915 the Samsung Galaxy S20 to S24 and Pixel class).
const PHONES = [{ width: 412, height: 915 }, { width: 360, height: 800 }];
// The GitHub Pages copy of the lesson, stood in for by request interception: it finds the server through board.json.
const PAGES = 'https://vaknin.github.io/nature-words/';
// The lesson's stages, the same list as the page's LESSON.stages: stage 2 shows one word at a time, stage 4 is the race.
const STAGES = ['Welcome to nature', 'Meet the words', 'Quiz time', 'Play and race', "Let's go!", "What's missing?"];

// ---------- Waits, each from a measured time (the run prints its slowest of each at the end) ----------
// The local server answered /v1/health 13 to 63 ms after it was started (test/signin.mjs); 5 s is 80 times the slowest.
const START_MS = 5000;
// A page load: slowest seen 0.14 s locally (4 runs of this test) and 2.64 s live (test/signin.mjs's page load);
// 15 s is 5 times the slowest live one.
const LOAD_MS = 15000;
// From the page load to the "Phone: 123456" label (a health check, an open and a poll: three answers): slowest seen
// 31 ms locally (4 runs). Live, each answer is one tunnel round trip, under 2.46 s (test/signin.mjs's slowest screen
// change); 10 s is above three of those.
const LABEL_MS = 10000;
// From a command sent to the projector moving (one held poll answering, then a redraw): slowest seen 66 ms locally
// (4 runs). Live it is one tunnel answer, under 2.46 s as above; 5 s is twice that.
const MOVE_MS = 5000;
// From the projector moving to its state readable through the API, or shown on the phone (which asks every second,
// its REMOTE_REFRESH_MS): slowest seen 53 ms locally (4 runs); 6 s is one refresh plus twice the slowest live answer.
const STATE_MS = 6000;
// One API answer or one phone screen change: slowest seen 33 ms locally (4 runs) and 2.46 s live (test/signin.mjs);
// 8 s is 3 times the slowest live one.
const SCREEN_MS = 8000;

const section = process.argv[2];
if (!['projector', 'phone', 'all'].includes(section)) { console.log('usage: node remote.mjs projector|phone|all (--local BIN | URL)'); process.exit(2); }
let base, key, child = null, tmp = null;
let failures = 0;
const fail = m => { failures++; console.log('  FAIL', m); };
const ok = (c, m) => { if (c) console.log('  ok  ', m); else fail(m); };
const timings = {};
const timed = async (what, p) => { const t = Date.now(); try { return await p; } finally { const d = Date.now() - t; timings[what] = Math.max(timings[what] || 0, d); } };

const freePort = () => new Promise((res, rej) => { const s = createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => res(p)); }); s.on('error', rej); });

async function startLocal(bin) {
  tmp = mkdtempSync(path.join(os.tmpdir(), 'remote-test-'));
  const www = path.join(tmp, 'www');
  mkdirSync(path.join(www, 'nature-words'), { recursive: true });
  cpSync(path.join(here, '..', 'index.html'), path.join(www, 'nature-words', 'index.html'));
  cpSync(path.join(here, '..', 'server', 'www'), www, { recursive: true });
  const port = await freePort();
  child = spawn(path.resolve(bin), [], {
    stdio: ['ignore', 'ignore', 'pipe'],
    env: { ...process.env, LESSON_PORT: String(port), LESSON_DB: path.join(tmp, 'lesson.db'), LESSON_BOARD: path.join(tmp, 'board.log'), LESSON_TEACHER_KEY: path.join(tmp, 'teacher.key'), LESSON_WWW: www },
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

// ---------- Browser and layout checks (the rules of test/signin.mjs) ----------
async function browserPage(vp, errors, mobile = false) {
  // Locally the stand-in Pages copy (a public https origin) calls the test server on 127.0.0.1, which Chromium's
  // local network access check refuses ("Permission was denied ... loopback address space"); the live server is a
  // public address, so the check is off only for --local runs.
  const lna = tmp ? ['--disable-features=LocalNetworkAccessChecks,PrivateNetworkAccessRespectPreflightResults,BlockInsecurePrivateNetworkRequests'] : [];
  const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage', ...lna] });
  const page = await browser.newPage();
  await page.setViewport({ ...vp, hasTouch: true, isMobile: mobile });
  page.on('pageerror', e => errors.push(e.message));
  page.on('console', m => { if (m.type() === 'error' && !/Failed to load resource/.test(m.text())) errors.push(m.text()); });
  // REMOTE_DEBUG=1 prints the page's console and every failed request.
  if (process.env.REMOTE_DEBUG) {
    page.on('console', m => console.log('    console', m.type(), m.text()));
    page.on('requestfailed', r => console.log('    failed', r.url(), r.failure()?.errorText));
  }
  return { browser, page };
}
// Anything on screen outside the window or the page scrolling; with `buttons`, also a button under 48x48 px (the
// common touch minimum, Material and WCAG 2.5.5) or its text cut off.
function layout(page, buttons) {
  return page.evaluate(buttons => {
    const vis = e => { const r = e.getBoundingClientRect(), s = getComputedStyle(e); return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none'; };
    const out = [];
    const d = document.documentElement;
    if (d.scrollWidth > innerWidth || d.scrollHeight > innerHeight) out.push(`page scrolls: ${d.scrollWidth}x${d.scrollHeight}`);
    for (const e of document.querySelectorAll('body *')) {
      if (e.closest('[hidden]') || e.closest('script,style') || !vis(e)) continue;
      const r = e.getBoundingClientRect();
      // 1 px: sub-pixel rounding.
      if (r.left < -1 || r.top < -1 || r.right > innerWidth + 1 || r.bottom > innerHeight + 1) out.push(`${e.tagName.toLowerCase()}${e.id ? '#' + e.id : ''}${e.className && typeof e.className === 'string' ? '.' + e.className.split(' ')[0] : ''} "${(e.innerText || '').trim().slice(0, 20)}" off screen`);
      if (!buttons || e.tagName !== 'BUTTON') continue;
      if (r.width < 47.5 || r.height < 47.5) out.push(`button "${(e.innerText || '').trim().slice(0, 20)}" ${Math.round(r.width)}x${Math.round(r.height)}, under 48x48`);
      if (e.scrollWidth > e.clientWidth + 1 || e.scrollHeight > e.clientHeight + 1) out.push(`button "${(e.innerText || '').trim().slice(0, 20)}" text does not fit`);
    }
    return out;
  }, buttons);
}
// REMOTE_SHOTS=1 keeps a picture of every screen checked, in test/shots, to look at.
async function checkLayout(page, where, buttons = false) {
  await page.evaluate(() => Promise.all(document.getAnimations().map(a => a.finished.catch(() => {}))));
  if (process.env.REMOTE_SHOTS) { mkdirSync(path.join(here, 'shots'), { recursive: true }); await page.screenshot({ path: path.join(here, 'shots', `remote-${where.replace(/\W+/g, '-')}.png`) }); }
  const bad = await layout(page, buttons);
  ok(bad.length === 0, `${where}: everything on screen${bad.length ? ': ' + bad.slice(0, 6).join('; ') : ''}`);
}
const shot = async (page, name) => { mkdirSync(path.join(here, 'shots'), { recursive: true }); await page.screenshot({ path: path.join(here, 'shots', `remote-${name}.png`) }).catch(() => {}); };

// The projector's screen now: stage, item, whether the QR is open, and the label.
const proj = page => page.evaluate(() => {
  const t = document.querySelector('.tscreen'), l = document.querySelector('[data-phone]');
  return { stage: +(t?.dataset.stage || 0), item: t?.dataset.item || '', qr: !!document.querySelector('.qrfull'), shown: !!document.querySelector('.tscreen .w.show'), label: l && !l.hidden ? l.textContent : '' };
});
// Waits for the projector to match `want`, timing it as one command-to-move.
async function moved(page, want, what) {
  await timed('command to move', page.waitForFunction(w => {
    const t = document.querySelector('.tscreen'); if (!t) return false;
    const now = { stage: +t.dataset.stage, item: t.dataset.item || '', qr: !!document.querySelector('.qrfull'), shown: !!document.querySelector('.tscreen .w.show') };
    return Object.entries(w).every(([k, v]) => k === 'notItem' ? now.item !== v : now[k] === v);
  }, { timeout: MOVE_MS }, want));
  ok(true, what);
}
// Opens the projector view at `url` and waits for its pairing code.
async function openProjector(page, url) {
  await timed('page load', page.goto(url, { waitUntil: 'load', timeout: LOAD_MS }));
  await timed('pairing label', page.waitForFunction(() => /^Phone: \d{6}$/.test(document.querySelector('[data-phone]:not([hidden])')?.textContent || ''), { timeout: LABEL_MS }));
  return (await proj(page)).label.slice(7);
}
const sendCmd = (id, cmd, stage) => teach('POST', `/api/screens/${id}/cmd`, stage ? { cmd, stage } : { cmd });
const stateIs = async (id, want, what) => {
  const t = Date.now();
  let last = null;
  while (Date.now() - t < STATE_MS) {
    last = (await teach('GET', '/api/screens/' + id)).data?.state;
    if (last && Object.entries(want).every(([k, v]) => last[k] === v)) break;
    await new Promise(r => setTimeout(r, 50));
  }
  timings['state readable'] = Math.max(timings['state readable'] || 0, Date.now() - t);
  ok(last && Object.entries(want).every(([k, v]) => last[k] === v), `${what}: ${JSON.stringify(last)}`);
};

// Request interception standing in for the GitHub Pages copy: the lesson page, a board.json naming `server` (or none),
// and a 404 for anything else there (its /v1/health).
async function asPages(page, server) {
  const html = await (await fetch(base + '/nature-words/')).text();
  await page.setRequestInterception(true);
  page.on('request', r => {
    const u = r.url();
    if (!u.startsWith('https://vaknin.github.io/')) return r.continue();
    const p = new URL(u).pathname;
    if (p === '/nature-words/' || p === '/nature-words/index.html') return r.respond({ status: 200, contentType: 'text/html; charset=utf-8', body: html });
    if (p === '/nature-words/board.json' && server) return r.respond({ status: 200, contentType: 'application/json', body: JSON.stringify({ url: server }), headers: { 'Access-Control-Allow-Origin': '*' } });
    return r.respond({ status: 404, contentType: 'text/plain', body: 'not found' });
  });
}

// ---------- Projector ----------
async function projectorSection() {
  console.log('\n== projector');
  await projectorRun('our server', base + '/nature-words/#teacher-1', null, true);
  await projectorRun('GitHub Pages copy', PAGES + '#teacher-1', base, false);
  await noServerRun();
}

async function projectorRun(where, url, pages, first) {
  const vp = PROJ[0], at = `${where} ${vp.width}x${vp.height}`;
  console.log(`-- ${at}`);
  const errors = [];
  const { browser, page } = await browserPage(vp, errors);
  try {
    if (pages) await asPages(page, pages);
    const code = await openProjector(page, url);
    ok(/^\d{6}$/.test(code), `${at}: the projector shows its pairing code (${code})`);
    ok(await page.$eval('[data-phone]', l => l.tagName !== 'BUTTON' && !!l.closest('.tbar')), `${at}: the code is a label in the top bar, not a button`);
    await checkLayout(page, `${at} with the code`);
    await page.setViewport({ ...PROJ[1], hasTouch: true });
    await checkLayout(page, `${where} ${PROJ[1].width}x${PROJ[1].height} with the code`);
    await page.setViewport({ ...vp, hasTouch: true });
    if (first) {
      ok((await api('POST', '/api/screens/pair', { code })).status === 401, 'pairing without the teacher key is refused (401)');
      const wrong = String((+code + 1) % 1e6).padStart(6, '0');
      ok((await teach('POST', '/api/screens/pair', { code: wrong })).status === 404, 'a code no projector shows is 404');
    }
    const p = await teach('POST', '/api/screens/pair', { code });
    ok(p.status === 200 && p.data.id, `${at}: pairs with the teacher key`);
    const id = p.data.id;
    if (first) ok((await teach('POST', '/api/screens/pair', { code })).status === 404, 'the same code a second time is 404');
    await timed('paired label', page.waitForFunction(() => document.querySelector('[data-phone]')?.textContent === 'Phone ✓', { timeout: MOVE_MS }));
    ok(true, `${at}: the label turns to "Phone ✓"`);
    await stateIs(id, { stage: 1, of: STAGES.length, name: STAGES[0] }, `${at}: the projector reported stage 1`);

    ok((await sendCmd(id, 'next')).status === 200, 'next sent');
    await moved(page, { stage: 2 }, `${at}: next moves to stage 2`);
    const item = (await proj(page)).item;
    ok(item !== '', `${at}: stage 2 shows a word (${item})`);
    ok((await sendCmd(id, 'space')).status === 200, 'space sent');
    await moved(page, { stage: 2, shown: true }, `${at}: space shows the word`);
    ok((await sendCmd(id, 'next')).status === 200, 'next sent');
    await moved(page, { stage: 2, notItem: item }, `${at}: next moves to the next word`);
    await stateIs(id, { stage: 2, name: STAGES[1], item: (await proj(page)).item }, `${at}: the API has the projector's word`);
    ok((await sendCmd(id, 'stage', 4)).status === 200, 'stage 4 sent');
    await moved(page, { stage: 4 }, `${at}: stage 4 jumps to the race`);
    ok((await sendCmd(id, 'qr')).status === 200, 'qr sent');
    await moved(page, { stage: 4, qr: true }, `${at}: qr opens the QR`);
    await stateIs(id, { stage: 4, name: STAGES[3], qr: true }, `${at}: the API has stage 4 with the QR open`);
    await checkLayout(page, `${at} QR open`);
    ok((await sendCmd(id, 'space')).status === 200, 'space sent');
    await moved(page, { stage: 4, qr: false }, `${at}: space closes the QR, as the key does`);
    // A change from the keys is reported too.
    await page.keyboard.press('ArrowRight');
    await moved(page, { stage: 5 }, `${at}: the → key still moves the projector`);
    await stateIs(id, { stage: 5, name: STAGES[4], qr: false }, `${at}: the API has the key's move`);
    if (first) {
      ok((await sendCmd(id, 'jump')).status === 400, 'a command that is not one of the five is 400');
      ok((await sendCmd(id, 'stage', 12)).status === 400, 'stage 12 is 400');
      const st = (await teach('GET', '/api/screens/' + id)).data;
      ok(st && st.polled_s_ago === 0, `while the projector waits for a command it counts as polling now (${st && st.polled_s_ago} s ago)`);
    }
    await checkLayout(page, `${at} paired`);
    await page.setViewport({ ...PROJ[1], hasTouch: true });
    await checkLayout(page, `${where} ${PROJ[1].width}x${PROJ[1].height} paired`);
    ok(errors.length === 0, `${at}: no page errors${errors.length ? ': ' + errors.slice(0, 3).join(' | ') : ''}`);
  } catch (e) {
    fail(`projector ${at}: ${e.message.split('\n')[0]}`);
    await shot(page, `projector-${where.replace(/\W+/g, '-')}`);
  } finally {
    await browser.close();
  }
}

// No lesson server reachable (a Pages copy whose board.json is missing): no label, no errors, the keys work as before.
async function noServerRun() {
  const vp = PROJ[0], at = `no server ${vp.width}x${vp.height}`;
  console.log(`-- ${at}`);
  const errors = [];
  const { browser, page } = await browserPage(vp, errors);
  try {
    await asPages(page, null);
    await timed('page load', page.goto(PAGES + '#teacher-1', { waitUntil: 'load', timeout: LOAD_MS }));
    // As long as the label takes to show with a server, and then as long again.
    await new Promise(r => setTimeout(r, 2 * (timings['pairing label'] || LABEL_MS / 5)));
    ok((await proj(page)).label === '', `${at}: no phone label`);
    await page.keyboard.press('ArrowRight');
    await moved(page, { stage: 2 }, `${at}: the → key moves the lesson as before`);
    const word = (await proj(page)).item;
    await page.click('[data-next]');
    await moved(page, { stage: 2, notItem: word }, `${at}: the Next button moves the lesson as before`);
    ok(errors.length === 0, `${at}: no page errors${errors.length ? ': ' + errors.slice(0, 3).join(' | ') : ''}`);
  } catch (e) {
    fail(`${at}: ${e.message.split('\n')[0]}`);
    await shot(page, 'no-server');
  } finally {
    await browser.close();
  }
}

// ---------- Phone ----------
async function phoneSection() {
  console.log('\n== phone');
  for (const [n, vp] of PHONES.entries()) await phoneRun(vp, n === 0);
}

async function phoneRun(vp, first) {
  const at = `phone ${vp.width}x${vp.height}`;
  console.log(`-- ${at}`);
  const errors = [], perrors = [];
  const P = await browserPage(PROJ[0], perrors);
  const T = await browserPage(vp, errors, true);
  const proj = P.page, phone = T.page;
  try {
    const code = await openProjector(proj, base + '/nature-words/#teacher-1');
    await timed('page load', phone.goto(base + '/teacher/', { waitUntil: 'load', timeout: LOAD_MS }));
    await phone.waitForSelector('#v-key:not([hidden])', { timeout: SCREEN_MS });
    await phone.type('#key', key);
    await phone.keyboard.press('Enter');
    await phone.waitForSelector('#v-classes:not([hidden])', { timeout: SCREEN_MS });
    await phone.tap('#open-remote');
    await phone.waitForSelector('#pair-form:not([hidden])', { timeout: SCREEN_MS });
    ok(true, `${at}: "Projector remote" opens the pairing form`);
    await checkLayout(phone, `${at} pairing`, true);
    if (first) {
      await phone.tap('#pair-code');
      await phone.keyboard.type(String((+code + 1) % 1e6).padStart(6, '0'));
      await phone.tap('#pair-go');
      await timed('screen', phone.waitForFunction(() => /No projector shows this code/.test(document.getElementById('pair-msg').innerText), { timeout: SCREEN_MS }));
      ok(true, `${at}: a wrong code says "No projector shows this code"`);
      await phone.$eval('#pair-code', i => { i.value = ''; });
    }
    await phone.tap('#pair-code');
    await phone.keyboard.type(code);
    await phone.tap('#pair-go');
    await phone.waitForSelector('#remote-ctl:not([hidden])', { timeout: SCREEN_MS });
    ok(await phone.evaluate(() => !!localStorage.getItem('lesson-remote-screen')), `${at}: paired, and the screen is remembered in this browser`);
    await timed('paired label', proj.waitForFunction(() => document.querySelector('[data-phone]')?.textContent === 'Phone ✓', { timeout: MOVE_MS }));
    await timed('phone shows stage', phone.waitForFunction(n => document.getElementById('rm-name').textContent === n, { timeout: STATE_MS }, STAGES[0]));
    ok(true, `${at}: the phone shows the projector's stage (${STAGES[0]})`);

    await phone.tap('[data-cmd="next"]');
    await moved(proj, { stage: 2 }, `${at}: Next moves the projector to stage 2`);
    await phone.tap('[data-cmd="space"]');
    await moved(proj, { stage: 2, shown: true }, `${at}: Show / Play shows the word on the projector`);
    await timed('phone shows stage', phone.waitForFunction(n => document.getElementById('rm-name').textContent === n, { timeout: STATE_MS }, STAGES[1]));
    ok(true, `${at}: the phone shows "${STAGES[1]}"`);
    await phone.tap('#rm-stages [data-stage="4"]');
    await moved(proj, { stage: 4 }, `${at}: the stage 4 chip jumps the projector to the race`);
    await timed('phone shows stage', phone.waitForFunction(n => document.getElementById('rm-name').textContent === n && document.querySelector('#rm-stages [data-stage="4"]').classList.contains('main'), { timeout: STATE_MS }, STAGES[3]));
    ok(true, `${at}: the phone shows "${STAGES[3]}" with its chip lit`);
    const pad = await phone.$$eval('#remote-ctl button', bs => bs.map(b => { const r = b.getBoundingClientRect(); return { c: b.dataset.cmd, a: r.width * r.height }; }));
    const big = pad.filter(b => b.c === 'prev' || b.c === 'next').map(b => b.a), rest = pad.filter(b => b.c !== 'prev' && b.c !== 'next').map(b => b.a);
    ok(Math.min(...big) > Math.max(...rest), `${at}: Back and Next are the two largest buttons`);
    await checkLayout(phone, `${at} remote`, true);
    await phone.tap('[data-cmd="prev"]');
    await moved(proj, { stage: 3 }, `${at}: Back moves the projector back`);

    // A screen the server no longer knows (a restart): "Projector not connected" and "Pair again".
    await phone.evaluate(() => localStorage.setItem('lesson-remote-screen', '0000000000000000'));
    await phone.reload({ waitUntil: 'load', timeout: LOAD_MS });
    await phone.waitForSelector('#v-classes:not([hidden])', { timeout: SCREEN_MS });
    await phone.tap('#open-remote');
    await timed('screen', phone.waitForSelector('#rm-lost:not([hidden])', { timeout: STATE_MS }));
    ok(/Projector not connected/.test(await phone.$eval('#rm-lost', e => e.innerText)), `${at}: an unknown screen says "Projector not connected"`);
    await checkLayout(phone, `${at} not connected`, true);
    await phone.tap('#rm-again');
    await phone.waitForSelector('#pair-form:not([hidden])', { timeout: SCREEN_MS });
    ok(true, `${at}: "Pair again" asks for the code again`);
    ok(errors.length === 0, `${at}: no page errors on the phone${errors.length ? ': ' + errors.slice(0, 3).join(' | ') : ''}`);
    ok(perrors.length === 0, `${at}: no page errors on the projector${perrors.length ? ': ' + perrors.slice(0, 3).join(' | ') : ''}`);
  } catch (e) {
    fail(`${at}: ${e.message.split('\n')[0]}`);
    await shot(phone, `phone-${vp.width}`);
    await shot(proj, `phone-projector-${vp.width}`);
  } finally {
    await P.browser.close();
    await T.browser.close();
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
  if (section === 'projector' || section === 'all') await projectorSection();
  if (section === 'phone' || section === 'all') await phoneSection();
} catch (e) {
  fail(e.message);
} finally {
  if (child) child.kill();
  if (tmp) rmSync(tmp, { recursive: true, force: true });
}
console.log('\nslowest seen (ms):', JSON.stringify(timings));
console.log(`slowest command to move: ${timings['command to move'] ?? '-'} ms`);
console.log(failures ? `\n${failures} FAILED` : '\nALL OK');
process.exit(failures ? 1 : 0);
