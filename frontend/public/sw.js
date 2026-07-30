const CACHE = 'sanasana-v1';
const ASSETS = ['/', '/index.html', '/pkg/frontend.js', '/pkg/frontend_bg.wasm'];

self.addEventListener('install', e =>
  e.waitUntil(caches.open(CACHE).then(c => c.addAll(ASSETS)))
);

self.addEventListener('fetch', e => {
  if (e.request.url.includes('/api') || e.request.url.includes('localhost:3000')) return;
  e.respondWith(
    caches.match(e.request).then(r => r || fetch(e.request))
  );
});
