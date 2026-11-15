import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { onRequestGet } from '../edge-functions/s/[[default]].js';

const root = new URL('../', import.meta.url);
const dist = new URL('docs/.vitepress/dist/', root);
const shell = await readFile(new URL('s/index.html', dist), 'utf8');
assert.match(shell, /edgeone-share-meta/);
assert.match(shell, /share-landing\.js/);
assert.match(shell, /share-landing\.css/);
for (const name of ['share-landing.js', 'share-landing.css']) {
  assert.deepEqual(await readFile(new URL(name, dist)), await readFile(new URL(`docs/public/${name}`, root)), `${name} was not copied to the build`);
}
for (const scene of ['cs2', 'deltaforce', 'valorant']) {
  const image = await readFile(new URL(`game-previews/${scene}.jpg`, dist));
  assert.equal(image[0], 0xff);
  assert.equal(image[1], 0xd8, `${scene} is not a JPEG`);
}

const realFetch = globalThis.fetch;
const slug = '0123456789abcdefghijkL';
try {
  for (const [scene, coverKey, expectedImage] of [
    ['cs2', '', 'https://filter-manage.6ya.site/game-previews/cs2.jpg'],
    ['siege', '', 'https://filter-manage.6ya.site/game-previews/deltaforce.jpg'],
    ['valorant', 'icc-images/existing.webp', 'https://filter-manage-download.xyls.us.kg/icc-images/existing.webp'],
    ['__proto__', '', 'https://filter-manage.6ya.site/game-previews/deltaforce.jpg'],
  ]) {
    let shellFetches = 0;
    globalThis.fetch = async (input) => {
      const url = String(input);
      if (url.endsWith('/s/index.html')) { shellFetches++; return new Response(shell); }
      assert.equal(url, `https://filter-manage-admin.xyls.us.kg/api/market/shares/${slug}`);
      return Response.json({ item: { title: '<script>test</script>', description: 'A "quote"', previewScene: scene, coverKey } });
    };
    const response = await onRequestGet({ params: { default: slug }, request: new Request(`https://filter-manage.6ya.site/s/${slug}`) });
    assert.equal(response.status, 200);
    assert.equal(response.headers.get('cache-control'), 'no-store');
    const html = await response.text();
    assert.match(html, /&lt;script&gt;test&lt;\/script&gt;/);
    assert.ok(html.includes(expectedImage), `missing card image for ${scene}`);
    assert.equal(shellFetches, 1);
    assert.ok(!html.includes('<!--edgeone-share-meta-->'));
  }
  globalThis.fetch = async (input) => {
    assert.equal(String(input), 'https://filter-manage.6ya.site/s/index.html');
    return new Response(shell);
  };
  const response = await onRequestGet({ params: { default: 'invalid!' }, request: new Request('https://filter-manage.6ya.site/s/invalid!') });
  assert.match(await response.text(), /Filter Manage 方案分享/);
} finally {
  globalThis.fetch = realFetch;
}
console.log(`Share routing, metadata escaping, scene cards and built assets passed (${fileURLToPath(dist)}).`);
