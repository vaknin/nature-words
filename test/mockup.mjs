// Mockup test: the three-look sign-in mockup (mockup/signin/index.html), opened from disk in a real browser at 1366x768.
// Fails unless every look holds its parts, nothing on the page is an emoji, the class-only words hold, a tapped name
// pops out and leaves (and leaves at once with reduced motion), and making a secret buddy reaches its reveal.
// Saves mockup/signin/preview.png. Run: cd test && npm ci && node mockup.mjs   (SHOTS=DIR also saves the demos mid-way)
import puppeteer from 'puppeteer-core';
import { fileURLToPath, pathToFileURL } from 'url';
import path from 'path';

const here = path.dirname(fileURLToPath(import.meta.url));
const CHROME = process.env.CHROME || '/usr/bin/chromium';
const PAGE = path.join(here, '../mockup/signin/index.html');
const PREVIEW = path.join(here, '../mockup/signin/preview.png');
const SHOTS = process.env.SHOTS;
// How long after the pop's end the name may still be there: the end event comes on a later frame. Measured on the server,
// headless chromium, 15 runs: removed 271-347 ms after the tap for a 250 ms pop (21-97 ms late). 150 ms covers that with room.
const SLACK_MS = Number(process.env.SLACK_MS || 150);
const SIZE = { width: 1366, height: 768 }; // the class's Asus laptops (research/kid-ux.md)

const misses = [];
const check = (ok, what) => { console.log((ok ? 'ok   ' : 'MISS ') + what); if (!ok) misses.push(what); };

const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, args: ['--no-sandbox'] });
try {
  const page = await browser.newPage();
  await page.setViewport(SIZE);
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
  page.on('request', r => { if (!r.url().startsWith('file:') && !r.url().startsWith('data:')) errors.push('loads from outside: ' + r.url()); });
  await page.goto(pathToFileURL(PAGE).href, { waitUntil: 'load' });
  await page.evaluate(() => document.fonts.ready);

  // Every look holds its parts.
  const looks = await page.$$eval('section[data-lang]', ss => ss.map(s => ({
    lang: s.dataset.lang, palette: !!s.dataset.palette, icons: s.dataset.icons || '',
    logos: s.querySelectorAll('[data-logo]').length, names: s.querySelectorAll('[data-demo="names"] [data-name]').length,
    pics: s.querySelectorAll('[data-demo="pick"] [data-pic]').length, guest: s.querySelectorAll('[data-guest]').length,
  })));
  check(looks.length === 3, `3 looks (found ${looks.length})`);
  for (const l of looks) {
    check(l.palette, `${l.lang}: has its palette`);
    check(/MIT|CC BY|ISC|Apache/.test(l.icons), `${l.lang}: names its icon set's license (${l.icons})`);
    check(l.logos >= 2, `${l.lang}: 2 logo ideas (found ${l.logos})`);
    check(l.names >= 6, `${l.lang}: name demo with 6+ names (found ${l.names})`);
    check(l.pics >= 6, `${l.lang}: buddy demo with 6+ tiles (found ${l.pics})`);
    check(l.guest >= 1, `${l.lang}: guest card`);
  }

  // No emoji: no Extended_Pictographic character in any text or attribute the page renders.
  const pict = await page.evaluate(() => {
    const re = /\p{Extended_Pictographic}/u, hits = [];
    const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    while (w.nextNode()) if (re.test(w.currentNode.data) && w.currentNode.parentElement.tagName !== 'SCRIPT') hits.push(w.currentNode.data.trim().slice(0, 40));
    for (const el of document.querySelectorAll('*')) for (const a of el.attributes) if (re.test(a.value)) hits.push(`${el.tagName}[${a.name}]`);
    if (re.test(document.title)) hits.push('title');
    return hits;
  });
  check(pict.length === 0, `no emoji in text or attributes ${pict.length ? JSON.stringify(pict) : ''}`);

  // Class-only: no talk of the other sign-in route outside his quoted words.
  const outside = await page.evaluate(() => {
    const body = document.body.cloneNode(true);
    body.querySelectorAll('blockquote.his-words, script, style').forEach(e => e.remove());
    const attrs = [...body.querySelectorAll('*')].flatMap(e => [...e.attributes].map(a => a.value)).join(' ');
    return [document.title, body.textContent, attrs].join(' ');
  });
  for (const w of ['classroom', 'at home', 'from home']) check(!outside.toLowerCase().includes(w), `no "${w}" outside his words`);

  // A taken name pops, then is gone and the list is one shorter.
  const popped = await page.evaluate(async SLACK_MS => {
    const list = document.querySelector('[data-lang=A] [data-demo="names"] .names');
    const btn = list.querySelector('[data-name]'), li = btn.closest('li'), name = btn.dataset.name, n = list.querySelectorAll('[data-name]').length;
    btn.scrollIntoView({ block: 'center', behavior: 'instant' });
    let removedAt = null;
    new MutationObserver(() => { if (!li.isConnected && removedAt === null) removedAt = performance.now(); }).observe(list, { childList: true });
    const t0 = performance.now();
    btn.click();
    const cs = getComputedStyle(li), anim = cs.animationName, ms = parseFloat(cs.animationDuration) * (cs.animationDuration.endsWith('ms') ? 1 : 1000);
    const disabled = btn.disabled, running = li.getAnimations().length;
    await new Promise(r => setTimeout(r, ms + SLACK_MS));
    return { name, anim, ms, disabled, running, took: removedAt === null ? null : Math.round(removedAt - t0),
      gone: !list.querySelector(`[data-name="${name}"]`), before: n, after: list.querySelectorAll('[data-name]').length };
  }, SLACK_MS);
  check(popped.disabled, 'a tapped name stops working on the same tap');
  check(popped.anim !== 'none' && popped.running > 0 && popped.ms > 0, `a tapped name starts a CSS animation (${popped.anim}, ${popped.ms} ms)`);
  check(popped.took >= popped.ms, `the name stays while it pops (left after ${popped.took} ms)`);
  check(popped.gone && popped.after === popped.before - 1, `within ${popped.ms} ms + ${SLACK_MS} ms the name is gone and the list is one shorter (${popped.before} to ${popped.after}; removed after ${popped.took} ms)`);
  if (SHOTS) await page.screenshot({ path: path.join(SHOTS, 'names-after.png') });

  // Reduced motion: the name leaves at once.
  await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'reduce' }]);
  const still = await page.evaluate(() => {
    const list = document.querySelector('[data-lang=B] [data-demo="names"] .names');
    const btn = list.querySelector('[data-name]'), name = btn.dataset.name, n = list.querySelectorAll('[data-name]').length;
    btn.click();
    return { gone: !list.querySelector(`[data-name="${name}"]`), before: n, after: list.querySelectorAll('[data-name]').length };
  });
  check(still.gone && still.after === still.before - 1, 'with reduced motion the name leaves on the same tap');
  await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'no-preference' }]);

  // Secret buddy: pick a tile, then a colour: the reveal shows; once more with the same buddy: saved.
  for (const lang of ['A', 'B', 'C']) {
    const box = `[data-lang=${lang}] [data-demo="pick"]`;
    await page.$eval(box, b => b.scrollIntoView({ block: 'center', behavior: 'instant' }));
    const tile = await page.$eval(`${box} [data-pic]`, t => t.dataset.pic);
    await page.click(`${box} [data-pic="${tile}"]`);
    await page.waitForSelector(`${box} [data-paint]:not([hidden])`, { timeout: 2000 });
    await page.click(`${box} [data-colour="purple"]`);
    const shown = await page.waitForSelector(`${box} [data-reveal]:not([hidden])`, { timeout: 2000 }).then(() => true, () => false);
    const said = shown ? await page.$eval(`${box} [data-buddy-name]`, e => e.textContent) : '';
    check(shown && /Purple/.test(said), `${lang}: tile then colour shows the reveal ("${said}")`);
    if (SHOTS && lang === 'A') await page.screenshot({ path: path.join(SHOTS, 'reveal.png') });
    if (!shown) continue;
    await page.click(`${box} [data-again]`);
    await page.click(`${box} [data-pic="${tile}"]`);
    await page.click(`${box} [data-colour="purple"]`);
    const saved = await page.waitForSelector(`${box} [data-done]:not([hidden])`, { timeout: 2000 }).then(() => true, () => false);
    check(saved, `${lang}: the same buddy once more is saved`);
  }

  check(errors.length === 0, `no page errors ${errors.length ? JSON.stringify(errors) : ''}`);

  // The preview: a fresh load, top of the page, as he first sees it.
  await page.goto(pathToFileURL(PAGE).href, { waitUntil: 'load' });
  await page.evaluate(() => document.fonts.ready);
  await page.screenshot({ path: PREVIEW });
  console.log('saved', path.relative(process.cwd(), PREVIEW));
} finally {
  await browser.close();
}
if (misses.length) { console.log(`${misses.length} missed`); process.exit(1); }
console.log('all passed');
