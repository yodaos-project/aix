const assert = require('node:assert/strict');

module.exports = async function testCheckedFrom(page) {
  const result = await page.evaluate(async () => {
    const { AIX, AixCheckError } = await import('./index.js');
    const { AixReaderWasm } = await import('./pkg-reader/aix_web.js');
    const encode = files => Object.entries(files).map(([path, content]) => ({
      path, data: new TextEncoder().encode(content),
    }));
    const pack = async files => (await AIX.pack(encode(files), { buildId: 'checked', optimize: false })).data;
    const data = await pack({ 'app.json': '{"pages":[],"permissions":["CAMERA"]}' });
    const entries = (await AIX.From(data)).list();
    const limits = {
      maxEntries: entries.length,
      maxEntryBytes: Math.max(...entries.map(entry => entry.size)),
      maxTotalBytes: entries.reduce((sum, entry) => sum + entry.size, 0),
    };
    const successes = [];
    for (const input of [data, new File([data], 'checked.aix')]) {
      const pkg = await AIX.From(input, limits, true);
      successes.push({ instance: pkg instanceof AIX, permissions: pkg.getPermissions(), version: pkg.getVersion() });
    }
    const warnings = { 'app.json': '{"pages":[],"permissions":["CUSTOM_PERMISSION"]}' };
    const warningReport = await AIX.checkFromSource(encode(warnings));
    const warningPkg = await AIX.From(await pack(warnings), undefined, true);

    // Inline scripts survive packing, so this checks the bytes in the archive.
    const invalid = {
      'app.json': '{"pages":["home","missing"],"permissions":["CUSTOM_PERMISSION"]}',
      'home.ink': '<page><view /></page>\n<script>const value = ;</script>',
    };
    const expected = await AIX.checkFromSource(encode(invalid));
    const invalidData = await pack(invalid);
    const legacy = await AIX.From(invalidData);
    const disabled = await AIX.From(invalidData, undefined, false);
    // Rename the app entry in both ZIP headers; the packer rejects missing apps.
    const missingAppData = data.slice();
    const headers = new DataView(missingAppData.buffer);
    const decoder = new TextDecoder();
    for (let offset = 0; offset + 46 < missingAppData.length; offset++) {
      const signature = headers.getUint32(offset, true);
      const nameOffset = signature === 0x04034b50 ? 30 : signature === 0x02014b50 ? 46 : 0;
      if (!nameOffset) continue;
      const length = headers.getUint16(offset + (nameOffset === 30 ? 26 : 28), true);
      const start = offset + nameOffset;
      if (decoder.decode(missingAppData.subarray(start, start + length)) === 'app.json') {
        missingAppData.set(new TextEncoder().encode('old.json'), start);
      }
    }
    const errors = [];
    const free = AixReaderWasm.prototype.free;
    let freed = 0;
    AixReaderWasm.prototype.free = function () { freed++; return free.call(this); };
    let operationError;
    let missingApp;
    let limitError;
    try {
      for (const input of [invalidData, new File([invalidData], 'invalid.aix')]) {
        try { await AIX.From(input, undefined, true); errors.push({ accepted: true }); }
        catch (error) {
          errors.push({ typed: error instanceof AixCheckError, error: error instanceof Error,
            name: error.name, message: error.message, report: error.report,
            diagnostics: error.diagnostics, sameDiagnostics: error.diagnostics === error.report?.diagnostics });
        }
      }
      try { await AIX.From(missingAppData, undefined, true); }
      catch (error) { missingApp = { typed: error instanceof AixCheckError, diagnostics: error.diagnostics }; }
      try {
        await AIX.From(await pack({ 'app.json': '{"pages":[]}', '.aixignore': '[z-a]' }), undefined, true);
      } catch (error) { operationError = { typed: error instanceof AixCheckError, message: String(error) }; }
      try { await AIX.From(data, { maxEntries: 0 }, true); }
      catch (error) { limitError = { typed: error instanceof AixCheckError, message: String(error) }; }
    } finally { AixReaderWasm.prototype.free = free; }
    return { successes, warningReport, warningPermissions: warningPkg.getPermissions(),
      expected, errors, freed, operationError, missingApp, limitError,
      legacyVersion: legacy.getVersion(), disabledVersion: disabled.getVersion() };
  });
  assert.deepEqual(result.successes, Array(2).fill({ instance: true, permissions: ['CAMERA'], version: 'checked' }));
  assert(result.warningReport.diagnostics.length > 0);
  assert(result.warningReport.diagnostics.every(diagnostic => diagnostic.severity === 'warning'));
  assert.deepEqual(result.warningPermissions, ['CUSTOM_PERMISSION']);
  assert(result.expected.diagnostics.some(diagnostic => diagnostic.code === 'AIX029'));
  assert(result.expected.diagnostics.some(diagnostic => diagnostic.severity === 'warning'));
  for (const error of result.errors) {
    assert.equal(error.typed, true);
    assert.equal(error.error, true);
    assert.equal(error.name, 'AixCheckError');
    assert.match(error.message, /AIX check failed/);
    assert.deepEqual(error.report, result.expected);
    assert.deepEqual(error.diagnostics, result.expected.diagnostics);
    assert.equal(error.sameDiagnostics, true);
  }
  assert.equal(result.missingApp.typed, true);
  assert(result.missingApp.diagnostics.some(diagnostic => diagnostic.code === 'AIX003'));
  assert.equal(result.operationError.typed, false);
  assert.match(result.operationError.message, /Invalid .aixignore/);
  assert.equal(result.limitError.typed, false);
  assert.match(result.limitError.message, /entry count limit/);
  assert.equal(result.freed, 4, 'Rejected readers must release their WASM resources');
  assert.equal(result.legacyVersion, 'checked');
  assert.equal(result.disabledVersion, 'checked');
  console.log('ok - optional From check accepts valid/warning-only packages, rejects with full diagnostics, respects limits and frees rejected readers');
};
