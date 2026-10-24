const assert = require('node:assert/strict');

module.exports = async function testReaderLimits(page) {
  const result = await page.evaluate(async () => {
    const { AIX } = window;
    const { AixReaderWasm } = await import('./pkg/aix_web.js');
    const { data } = await AIX.pack([
      { path: 'app.json', data: new TextEncoder().encode('{"pages":[]}') },
      { path: 'payload', data: new Uint8Array(4096).fill(42) },
    ], { buildId: 'limits', optimize: false });
    const legacy = await AIX.From(data);
    const entries = legacy.list();
    const total = entries.reduce((sum, entry) => sum + entry.size, 0);
    const limits = { maxEntries: entries.length, maxEntryBytes: 4096, maxTotalBytes: total };
    const successes = [];
    for (const input of [data, new File([data], 'limits.aix')]) {
      const reader = await AIX.From(input, limits);
      successes.push(reader.readFile('payload').length, reader.readFile('payload').length);
    }
    const failures = [];
    for (const [policy, expected] of [
      [{ maxEntries: entries.length - 1 }, 'entry count limit'],
      [{ maxEntryBytes: 4095 }, 'per-entry byte limit'],
      [{ maxTotalBytes: total - 1 }, 'total byte limit'],
      [{ maxEntries: 0 }, 'entry count limit'],
      [{ maxEntryBytes: -1 }, 'Invalid read limits'],
      [{ maxEntryBytes: 1.5 }, 'Invalid read limits'],
      [{ maxTotalBytes: Number.MAX_SAFE_INTEGER + 1 }, 'Invalid read limits'],
      [{ maxEntryBytes: Infinity }, 'Invalid read limits'],
      [{ maxEntryBytes: NaN }, 'Invalid read limits'],
      [{ maxEntries: '1' }, 'Invalid read limits'],
      [{ maxEntryByte: 1 }, 'Invalid read limits'],
      [[], 'Invalid read limits'],
    ]) {
      for (const input of [data, new File([data], 'limits.aix')]) {
        try { await AIX.From(input, policy); failures.push('accepted'); }
        catch (error) { failures.push(String(error).includes(expected) ? 'rejected' : String(error)); }
      }
    }
    // Forge only the central size; compressed payload still expands to 4096.
    const forged = data.slice();
    const view = new DataView(forged.buffer);
    let offset = 0;
    while (view.getUint32(offset, true) !== 0x02014b50) offset++;
    while (offset < forged.length && view.getUint32(offset, true) === 0x02014b50) {
      const length = view.getUint16(offset + 28, true);
      const name = new TextDecoder().decode(forged.subarray(offset + 46, offset + 46 + length));
      if (name === 'payload') { view.setUint32(offset + 24, 64, true); break; }
      offset += 46 + length + view.getUint16(offset + 30, true) + view.getUint16(offset + 32, true);
    }
    const bounded = await AIX.From(forged, { maxEntryBytes: 4096, maxTotalBytes: total });
    let forgedError;
    try { bounded.readFile('payload'); }
    catch (error) { forgedError = String(error); }
    const raw = AixReaderWasm.with_limits(data, limits);
    const rawLength = raw.read_file('payload').length;
    raw.free();
    return { successes, failures, forgedError, rawLength, legacyLength: legacy.readFile('payload').length,
      emptyPolicyLength: (await AIX.From(data, {})).readFile('payload').length };
  });
  assert.deepEqual(result.successes, [4096, 4096, 4096, 4096]);
  assert(result.failures.every((failure) => failure === 'rejected'), JSON.stringify(result.failures));
  assert.match(result.forgedError, /Failed to extract|Extracted file size limit exceeded/);
  assert.equal(result.rawLength, 4096);
  assert.equal(result.legacyLength, 4096);
  assert.equal(result.emptyPolicyLength, 4096);
  console.log('ok - browser SDK and raw WASM archive resource limits');
};
