// Builds mockup/signin/index.html from mockup/signin/template.html, one self-contained file:
// - fetches the three icon packages with `npm pack` into a temp dir (pinned versions below) and inlines only the
//   glyphs the page uses, as <symbol id="A-cat"> etc. (A Phosphor duotone, B Tabler outline, C game-icons);
// - credits game-icons per icon, authors read from tools/game-icons-authors.txt (CC BY 3.0 asks for it);
// - inlines Google Fonts subset to the page's own characters as data: URLs, so the page loads nothing from outside;
// - expands the template's macros: {{icon SET NAME}}, {{tiles SET}}, {{names}}, {{colours}}.
// Run from the repo root: node tools/mockup-build.mjs
import { readFileSync, writeFileSync, mkdtempSync, rmSync, mkdirSync } from 'fs';
import { execFileSync } from 'child_process';
import { fileURLToPath } from 'url';
import os from 'os';
import path from 'path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const SRC = path.join(root, 'mockup/signin/template.html');
const OUT = path.join(root, 'mockup/signin/index.html');
const PKGS = { A: '@phosphor-icons/core@2.1.1', B: '@tabler/icons@3.51.0', C: '@iconify-json/game-icons@1.2.4' };

// ---------- Icon packages, fetched fresh into a temp dir ----------
const tmp = mkdtempSync(path.join(os.tmpdir(), 'mockup-icons-'));
const dirs = {};
try {
  for (const [k, spec] of Object.entries(PKGS)) {
    const out = execFileSync('npm', ['pack', spec, '--silent', '--pack-destination', tmp], { cwd: tmp, encoding: 'utf8' }).trim().split('\n').pop();
    dirs[k] = path.join(tmp, k);
    mkdirSync(dirs[k]);
    execFileSync('tar', ['xzf', path.join(tmp, out), '-C', dirs[k]]);
  }
  await build();
} finally {
  rmSync(tmp, { recursive: true, force: true });
}

function build() {
  const PH = path.join(dirs.A, 'package/assets/duotone/');
  const TB = path.join(dirs.B, 'package/icons/outline/');
  const GI = JSON.parse(readFileSync(path.join(dirs.C, 'package/icons.json'), 'utf8')).icons;
  const authors = Object.fromEntries(readFileSync(path.join(root, 'tools/game-icons-authors.txt'), 'utf8')
    .split('\n').filter(l => l.trim()).map(l => l.trim().split(/\s+/)));

  // Each set: its 16 sign-in glyphs (in order) and its UI glyphs under shared names.
  const UI = ['fullscreen', 'check', 'trophy', 'projector', 'user', 'sparkle', 'play', 'confetti', 'refresh'];
  const sets = {
    A: {
      get: n => {
        const s = readFileSync(PH + n + '-duotone.svg', 'utf8');
        return ['0 0 256 256', `<g fill="currentColor">${s.replace(/^<svg[^>]*>/, '').replace(/<\/svg>\s*$/, '')}</g>`];
      },
      animals: ['cat', 'dog', 'rabbit', 'cow', 'bird', 'fish', 'butterfly', 'horse', 'shrimp', 'bug-beetle', 'tree', 'leaf', 'cactus', 'flower-tulip', 'acorn', 'sun'],
      ui: ['corners-out', 'check-circle', 'trophy', 'projector-screen', 'user-circle', 'sparkle', 'play', 'confetti', 'arrow-counter-clockwise'],
    },
    B: {
      get: n => {
        const s = readFileSync(TB + n + '.svg', 'utf8');
        const body = s.replace(/^[\s\S]*?<svg[^>]*>/, '').replace(/<\/svg>\s*$/, '')
          .replace(/<path stroke="none" d="M0 0h24v24H0z" fill="none"\s*\/>/, '').replace(/\s*\n\s*/g, '').trim();
        return ['0 0 24 24', `<g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${body}</g>`];
      },
      animals: ['cat', 'dog', 'pig', 'horse', 'deer', 'fish', 'butterfly', 'snail', 'spider', 'bat', 'bug', 'cricket', 'flower', 'cactus', 'leaf', 'seedling'],
      ui: ['maximize', 'circle-check', 'trophy', 'presentation', 'user', 'sparkles', 'player-play', 'confetti', 'refresh'],
    },
    C: {
      get: n => {
        if (!GI[n]) throw new Error('game-icons has no ' + n);
        return ['0 0 512 512', `<g fill="currentColor">${GI[n].body.replace(/ fill="currentColor"/g, '')}</g>`];
      },
      animals: ['cat', 'fox', 'panda', 'frog', 'pig', 'owl', 'lion', 'koala', 'penguin', 'monkey', 'cow', 'rabbit', 'turtle', 'elephant', 'octopus', 'hedgehog'],
      ui: ['expand', 'check-mark', 'trophy', 'light-projector', 'person', 'sparkles', 'play-button', 'party-popper', 'cycle'],
    },
  };
  // Words a kid reads for each glyph (English, Hebrew).
  const WORDS = {
    cat: ['Cat', 'חתול'], dog: ['Dog', 'כלב'], rabbit: ['Rabbit', 'ארנב'], cow: ['Cow', 'פרה'], bird: ['Bird', 'ציפור'],
    fish: ['Fish', 'דג'], butterfly: ['Butterfly', 'פרפר'], horse: ['Horse', 'סוס'], shrimp: ['Shrimp', 'שרימפס'],
    'bug-beetle': ['Beetle', 'חיפושית'], tree: ['Tree', 'עץ'], leaf: ['Leaf', 'עלה'], cactus: ['Cactus', 'קקטוס'],
    'flower-tulip': ['Tulip', 'צבעוני'], acorn: ['Acorn', 'בלוט'], sun: ['Sun', 'שמש'], pig: ['Pig', 'חזיר'],
    deer: ['Deer', 'צבי'], snail: ['Snail', 'חילזון'], spider: ['Spider', 'עכביש'], bat: ['Bat', 'עטלף'], bug: ['Bug', 'חרק'],
    cricket: ['Cricket', 'צרצר'], flower: ['Flower', 'פרח'], seedling: ['Sprout', 'נבט'], fox: ['Fox', 'שועל'],
    panda: ['Panda', 'פנדה'], frog: ['Frog', 'צפרדע'], owl: ['Owl', 'ינשוף'], lion: ['Lion', 'אריה'], koala: ['Koala', 'קואלה'],
    penguin: ['Penguin', 'פינגווין'], monkey: ['Monkey', 'קוף'], turtle: ['Turtle', 'צב'], elephant: ['Elephant', 'פיל'],
    octopus: ['Octopus', 'תמנון'], hedgehog: ['Hedgehog', 'קיפוד'],
  };

  let sprite = '<svg width="0" height="0" style="position:absolute" aria-hidden="true"><defs>\n';
  for (const [k, s] of Object.entries(sets)) {
    s.animals.forEach(n => { const [vb, body] = s.get(n); sprite += `<symbol id="${k}-${n}" viewBox="${vb}">${body}</symbol>\n`; });
    s.ui.forEach((n, i) => { const [vb, body] = s.get(n); sprite += `<symbol id="${k}-${UI[i]}" viewBox="${vb}">${body}</symbol>\n`; });
  }
  sprite += '</defs></svg>';

  const giNames = [...sets.C.animals, ...sets.C.ui];
  const missing = giNames.filter(n => !authors[n]);
  if (missing.length) throw new Error('no game-icons author for: ' + missing.join(', '));
  const byAuthor = {};
  giNames.forEach(n => (byAuthor[authors[n]] ||= []).push(n));
  const giCredit = Object.entries(byAuthor).map(([a, ns]) => `${a} (${ns.join(', ')})`).join('; ');
  const notice = `<!--
Icons inlined below, copied from the packages unchanged except for colour wrappers:
- Phosphor Icons 2.1.1 (duotone), MIT License, Copyright 2023 Phosphor Icons. https://github.com/phosphor-icons/core
- Tabler Icons 3.51.0 (outline), MIT License, Copyright 2020-2026 Pawel Kuna. https://github.com/tabler/tabler-icons
- game-icons.net via @iconify-json/game-icons 1.2.4, CC BY 3.0, https://game-icons.net, by ${giCredit}.
Fonts: Fredoka, Rubik, Secular One, Varela Round, Karantina, SIL Open Font License 1.1, subset to this page's letters.
-->`;

  const icon = (k, n, cls = 'ic') => {
    if (!sprite.includes(`<symbol id="${k}-${n}"`)) throw new Error(`no glyph ${k}-${n}: use one of the set's 16 or ${UI.join(', ')}`);
    return `<svg class="${cls}" aria-hidden="true"><use href="#${k}-${n}"/></svg>`;
  };
  const word = n => WORDS[n] || [n, ''];
  const tiles = k => sets[k].animals.map(n => `<button type="button" class="tile" data-pic="${n}" data-en="${word(n)[0]}" data-he="${word(n)[1]}">${icon(k, n)}<span>${word(n)[0]}</span></button>`).join('\n');
  const strip = k => sets[k].animals.map(n => `<li>${icon(k, n)}<span>${word(n)[0]}</span></li>`).join('');
  const NAMES = ['Noa', 'Itay', 'Maya R.', 'Maya S.', 'Omer', 'Yael', 'Daniel', 'Lior'];
  const names = () => NAMES.map(n => `<li><button type="button" class="name" data-name="${n}">${n}</button></li>`).join('\n');
  // Six colours: the chromatic basic colour words of both English and Hebrew (research/kid-ux.md, Secret picture).
  const COLOURS = [['red', 'Red', 'אדום', '#e5484d'], ['orange', 'Orange', 'כתום', '#f76b15'], ['yellow', 'Yellow', 'צהוב', '#f5b800'],
    ['green', 'Green', 'ירוק', '#2f9e5b'], ['blue', 'Blue', 'כחול', '#1e7fe0'], ['purple', 'Purple', 'סגול', '#8e4ec6']];
  const colours = () => COLOURS.map(([id, en, he, hex]) => `<button type="button" class="colour" data-colour="${id}" data-en="${en}" data-he="${he}" style="--c:${hex}"><i></i><span>${en}</span><span lang="he" dir="rtl">${he}</span></button>`).join('\n');

  let html = readFileSync(SRC, 'utf8')
    .replace('<!--SPRITES-->', notice + '\n' + sprite)
    .replaceAll('{{gi-credit}}', giCredit)
    .replace(/\{\{icon ([ABC]) ([a-z-]+)\}\}/g, (_, k, n) => icon(k, n))
    .replace(/\{\{tiles ([ABC])\}\}/g, (_, k) => tiles(k))
    .replace(/\{\{strip ([ABC])\}\}/g, (_, k) => strip(k))
    .replaceAll('{{names}}', names())
    .replaceAll('{{colours}}', colours());
  const left = html.match(/\{\{[^}]*\}\}/);
  if (left) throw new Error('unexpanded macro ' + left[0]);
  return fonts(html);
}

// ---------- Fonts: one subset per family and weight, holding every character the page uses ----------
async function fonts(html) {
  const chars = [...new Set([...html.replace(/<!--[\s\S]*?-->/g, '').replace(/<[^>]*>/g, ' ')].filter(c => c.charCodeAt(0) > 126))].join('');
  const ascii = Array.from({ length: 95 }, (_, i) => String.fromCharCode(32 + i)).join('');
  const text = ascii + chars;
  const fams = [['Fredoka', [500, 700]], ['Rubik', [400, 500, 700]], ['Secular One', [400]], ['Varela Round', [400]], ['Karantina', [400, 700]]];
  const UA = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36';
  let css = '';
  for (const [fam, ws] of fams) {
    for (const w of ws) {
      const fq = fam.replace(/ /g, '+') + (ws.length > 1 || w !== 400 ? `:wght@${w}` : '');
      const r = await fetch(`https://fonts.googleapis.com/css2?family=${fq}&text=${encodeURIComponent(text)}`, { headers: { 'User-Agent': UA } });
      if (!r.ok) throw new Error(fam + ' ' + w + ': HTTP ' + r.status);
      const c = await r.text();
      const urls = [...c.matchAll(/url\((https:[^)]+)\)/g)].map(m => m[1]);
      if (urls.length !== 1) throw new Error(fam + ' ' + w + ': ' + urls.length + ' files\n' + c);
      const b = Buffer.from(await (await fetch(urls[0])).arrayBuffer());
      css += `@font-face{font-family:"${fam}";src:url(data:font/woff2;base64,${b.toString('base64')}) format("woff2");font-weight:${w};font-display:block}\n`;
      console.error(fam, w, b.length, 'bytes');
    }
  }
  html = html.replace('/*FONTS*/', css);
  writeFileSync(OUT, html);
  console.error('wrote', path.relative(root, OUT), html.length, 'bytes');
}
