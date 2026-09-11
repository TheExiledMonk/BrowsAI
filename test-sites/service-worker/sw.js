self.addEventListener('fetch', event => {
  event.respondWith(new Response('worker response', {
    headers: {'Content-Type': 'text/plain'}
  }));
});

self.addEventListener('message', event => {
  if (!event.data || event.data.type !== 'probe') return;
  const result = {
    realm: 'ServiceWorker',
    offscreenCanvas: typeof OffscreenCanvas === 'function',
    context2d: 'not-probed',
    width: null,
    height: null,
    fetch: typeof fetch === 'function',
    webSocket: typeof WebSocket === 'function',
    crypto: typeof crypto === 'object',
    textEncoder: typeof TextEncoder === 'function',
    url: typeof URL === 'function'
  };
  try {
    const canvas = new OffscreenCanvas(320, 180);
    result.width = canvas.width;
    result.height = canvas.height;
    result.context2d = canvas.getContext('2d') ? 'available' : 'unavailable';
  } catch (error) {
    result.context2d = 'error:' + (error && error.name || 'unknown');
  }
  if (event.ports && event.ports[0]) event.ports[0].postMessage(result);
});
