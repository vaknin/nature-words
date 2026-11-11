// Kitah's service worker: what makes the teacher page an installable app, and lets it open with no network.
// It answers only the teacher page and its files under /teacher/. Anything under /api (classes, kids, the remote)
// and anything outside /teacher/ goes straight to the network untouched, so the teacher always sees live data.
// The page is network-first: a fresh copy whenever the server answers, the last good copy when it does not.
// JSON has no comments, so the manifest's icon sizes are explained here: 192 and 512 px are Chrome's install
// criteria (web.dev/articles/install-criteria), and the maskable one keeps the K inside the centre 80%.
'use strict';
const CACHE = 'kitah-page-1';
const SCOPE = '/teacher/';

const skipped = url => url.origin !== self.location.origin || url.pathname === '/api' || url.pathname.startsWith('/api/') || !url.pathname.startsWith(SCOPE);

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', e => e.waitUntil((async () => {
  for (const k of await caches.keys()) if (k !== CACHE) await caches.delete(k);
  await self.clients.claim();
})()));

self.addEventListener('fetch', e => {
  const url = new URL(e.request.url);
  if (e.request.method !== 'GET' || skipped(url)) return;
  e.respondWith((async () => {
    try {
      const r = await fetch(e.request);
      if (r.ok) { const c = await caches.open(CACHE); await c.put(e.request, r.clone()); }
      return r;
    } catch (err) {
      const hit = await caches.match(e.request, { ignoreSearch: true });
      if (hit) return hit;
      throw err;
    }
  })());
});
