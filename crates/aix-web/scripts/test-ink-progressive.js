const assert = require('node:assert/strict');

async function testInkProgressive(page, origin, upload, studioUploadSource, media) {
  await page.evaluate(async ({ studioUploadSource, media }) => {
    const { AIX } = await import('/index.js');
    const studioUpload = Function('return (' + studioUploadSource + ')')();
    const pages = {
      audio: `<script setup>
import { AudioPlayer } from 'audio';
import tone from './tone.m4a' with { type: 'audio', progressive: 'prefer' };
let player;
export default {
  onLoad() {
    player = new AudioPlayer();
    player.onTimeUpdate(() => this.postMessage({ name: 'audio-time', time: player.currentTime }));
    player.onError(error => this.postMessage({ name: 'error', error }));
    player.src = tone;
    this.postMessage({ name: 'audio-ready' });
  },
  onUnload() { if (player) player.destroy(); },
  startAudio() { player.seek(4.25); player.play(); },
};
</script>
<page>
  <view style="width:160px;height:56px;background:#abcdef;" bindtap="startAudio"><text>Play audio</text></view>
</page>`,
      video: `<script setup>
import movie from './movie.mp4' with { type: 'video', progressive: 'prefer' };
export default {
  data: { movie },
  startVideo() {
    const video = wx.createVideoContext('movie');
    video.seek(4.25);
    video.play();
  },
  metadata() { this.postMessage({ name: 'video-ready' }); },
  videoTime(event) { this.postMessage({ name: 'video-time', time: event.detail.currentTime }); },
  error(event) { this.postMessage({ name: 'error', error: event.detail }); },
};
</script>
<page>
  <view style="width:160px;height:56px;background:#abcdef;" bindtap="startVideo"><text>Play video</text></view>
  <video id="movie" src="{{movie}}" preload="metadata" muted="{{true}}" controls="{{false}}"
    style="width:160px;height:100px;" bindloadedmetadata="metadata" bindtimeupdate="videoTime" binderror="error" />
</page>`,
    };
    const inputs = [
      ['app.json', JSON.stringify({ pages: Object.keys(pages) })],
      ...Object.entries(pages).map(([name, source]) => [name + '.ink', source]),
      ['tone.m4a', Uint8Array.from(media.tone)],
      ['movie.mp4', Uint8Array.from(media.video)],
    ].map(([path, value]) => ({
      path, data: typeof value === 'string' ? new TextEncoder().encode(value) : value,
    }));
    const assets = {};
    const packed = await AIX.pack(inputs, {
      async onProgressiveAsset(candidate, context) {
        const plan = await context.prepare();
        const receipt = await studioUpload(plan, context.signal);
        assets[candidate.kind] = plan.layout.fragments.map(fragment => fragment.object);
        return { delivery: 'progressive', planId: plan.id, receipt };
      },
    });
    const archive = await AIX.From(packed.data);
    const files = Object.fromEntries(archive.list().filter(entry => !entry.name.endsWith('/'))
      .map(({ name }) => [name, Array.from(archive.readFile(name))]));
    window.progressiveBundle = { files, assets };
  }, { studioUploadSource, media });

  const bundle = await page.evaluate(() => window.progressiveBundle);
  const getObjects = () => upload.requests.filter(([method]) => method === 'GET').map(([, p]) => p.slice(9));
  const appId = 'studio-progressive-browser';
  const start = async (mode, id = appId) => {
    await page.evaluate(async ({ bundle, mode, id }) => {
      window.unbindInkEvents?.();
      window.ink?.destroy();
      const canvas = document.createElement('canvas');
      canvas.width = 256;
      canvas.height = 240;
      document.body.style.margin = '0';
      document.body.replaceChildren(canvas);
      window.inkMessages = [];
      const { createInkView } = await import('/ink/index.js');
      window.ink = await createInkView({
        width: 256, height: 240, canvas, autoRender: true,
        onMessage: message => window.inkMessages.push(message.data),
      });
      window.unbindInkEvents = window.ink.bindDomEvents();
      window.ink.openBundle({
        appId: id,
        files: Object.fromEntries(Object.entries(bundle.files).map(([name, bytes]) => [name, Uint8Array.from(bytes)])),
        initialPage: mode,
        hostOptions: { initialTarget: '_blank' },
      });
    }, { bundle, mode, id });
  };
  const waitFor = async (name, minTime = 0) => {
    await page.waitForFunction(({ name, minTime }) =>
      window.inkMessages.some(message => message.name === 'error'
        || (message.name === name && (message.time === undefined || message.time >= minTime))),
    { name, minTime }, { timeout: 8000 }).catch(async error => {
      throw new Error(`Waiting for ${name}: ${JSON.stringify(await page.evaluate(() => window.inkMessages))}`, { cause: error });
    });
    assert(!await page.evaluate(() => window.inkMessages.find(message => message.name === 'error')),
      JSON.stringify(await page.evaluate(() => window.inkMessages)));
  };

  const lastAudio = bundle.assets.audio.at(-1);
  upload.invalid.add(lastAudio);
  try {
    await start('audio', appId + '-tampered');
    await waitFor('audio-ready');
    await page.mouse.click(40, 26);
    await page.waitForFunction(() => window.inkMessages.some(message => message.name === 'error'));
    assert(getObjects().includes(lastAudio), 'Ink requested the tampered fragment');
  } finally {
    upload.invalid.delete(lastAudio);
  }

  const hasColor = async (x, y, width, height) => page.evaluate(async ({ x, y, width, height }) => {
    const context = document.querySelector('canvas').getContext('2d');
    for (let attempt = 0; attempt < 100; attempt++) {
      const pixels = context.getImageData(x, y, width, height).data;
      for (let i = 0; i < pixels.length; i += 4) {
        const [r, g, b, a] = pixels.subarray(i, i + 4);
        if (a && Math.max(r, g, b) - Math.min(r, g, b) > 32) return true;
      }
      await new Promise(resolve => setTimeout(resolve, 20));
    }
    return false;
  }, { x, y, width, height });
  await start('audio');
  await waitFor('audio-ready');
  let before = getObjects().length;
  await page.waitForTimeout(250);
  assert.equal(getObjects().length, before, 'audio metadata is local');
  await page.mouse.click(40, 26);
  await waitFor('audio-time', 4.27);
  assert.equal(getObjects()[before], bundle.assets.audio.at(-1), 'audio seek fetched the last fragment first');

  await start('video');
  await waitFor('video-ready');
  before = getObjects().length;
  await page.waitForTimeout(250);
  assert.equal(getObjects().length, before, 'video metadata is local');
  await page.mouse.click(40, 26);
  await waitFor('video-time', 3.9);
  assert.equal(getObjects()[before], bundle.assets.video.at(-1), 'video seek fetched the last fragment first');
  assert(await hasColor(0, 100, 160, 48), 'Ink painted decoded H.264 pixels');

  // Reopen through a fresh page and WASM instance. HTTP cache is bypassed and
  // the object origin is unreachable for these requests.
  before = getObjects().length;
  const missed = [];
  await page.route('**/objects/*', route => {
    missed.push(route.request().url());
    return route.abort('internetdisconnected');
  });
  await page.goto(origin);
  await start('audio');
  await waitFor('audio-ready');
  await page.mouse.click(40, 26);
  await waitFor('audio-time', 4.27);
  await start('video');
  await waitFor('video-ready');
  await page.mouse.click(40, 26);
  await waitFor('video-time', 3.9);
  assert(await hasColor(0, 100, 160, 48), 'Ink painted cached H.264 pixels');
  assert.deepEqual(missed, []);
  assert.equal(getObjects().length, before);
  console.log('Ink Web rejected tampering, sought MP4/AAC, and reopened verified IndexedDB data offline');
}

module.exports = { testInkProgressive };
