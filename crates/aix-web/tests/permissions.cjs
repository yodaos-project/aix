const assert = require('node:assert/strict');
const path = require('node:path');

module.exports = async (page, cli) => {
  const engine = require(path.join(path.dirname(cli), 'pkg/aix_web.js'));
  const cases = [
    { app: { pages: false, widgets: 42, window: false,
      permissions: ['GEOLOCATION', 'CAMERA', 'CUSTOM_PERMISSION', 'CAMERA'] },
      expected: ['GEOLOCATION', 'CAMERA', 'CUSTOM_PERMISSION', 'CAMERA'] },
    { app: { pages: [] }, expected: [] },
    { app: { pages: [], permissions: [] }, expected: [] },
    { app: { pages: [], permissions: null } },
    { app: { pages: [], permissions: 'CAMERA' } },
    { app: { pages: [], permissions: ['CAMERA', 42] } },
  ];
  for (const { app, expected } of cases) {
    const packed = engine.pack_aix([
      { path: 'app.json', data: new TextEncoder().encode(JSON.stringify(app)) },
    ], 'permissions-test', undefined, null);
    const data = packed.data;
    packed.free();
    const native = new engine.AixReaderWasm(data);
    try {
      if (expected) assert.deepEqual(native.get_permissions(), expected);
      else assert.throws(() => native.get_permissions(), /Invalid app.json:/);
    } finally { native.free(); }

    const result = await page.evaluate(async (bytes) => {
      const { AixReaderWasm } = await import('./pkg/aix_web.js');
      const data = new Uint8Array(bytes);
      const sdk = await window.AIX.From(data);
      const raw = new AixReaderWasm(data);
      const read = (get) => {
        try { return { permissions: get() }; }
        catch (error) { return { error: String(error) }; }
      };
      try {
        return { sdk: read(() => sdk.getPermissions()), raw: read(() => raw.get_permissions()) };
      } finally { raw.free(); }
    }, [...data]);
    for (const output of [result.sdk, result.raw]) {
      if (expected) assert.deepEqual(output, { permissions: expected });
      else assert.match(output.error, /Invalid app.json:/);
    }
  }
  console.log('ok - SDK, reader-only WASM, full WASM and Node preserve permission declarations and reject invalid lists');
};
