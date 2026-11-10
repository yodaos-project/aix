const assert = require('node:assert/strict');

module.exports = async function testMetadataResolution(page) {
  const results = await page.evaluate(async () => {
    const { AIX } = window;
    const { AixReaderWasm } = await import('./pkg/aix_web.js');
    const good = '<script def>{"navigationBarTitleText":"Good"}</script><page><view /></page>';
    const cases = [
      { files: { 'app.json': '{"pages":["card","home","details"],"entryPagePath":"home"}',
        'card.ink': '<script def>{"target":"inline"}</script><page><view /></page>',
        'home.ink': good,
        'details.json': '{"target":"_blank"}',
        'details.wxml': '<view />' },
        names: ['home', 'card', 'details'], targets: ['_blank', '_current', '_blank'], errors: [], legacy: 3 },
      { files: { 'app.json': '{"pages":["home","card"]}',
        'home.ink': '<script def>{"schema":{"data":{"type":"string"}}}</script><page><view /></page>',
        'card.ink': good },
        names: ['home', 'card'], targets: ['_current', '_current'], errors: [], legacy: 2 },
      { files: { 'app.json': '{"pages":["missing","good"]}', 'good.ink': good },
        names: ['good'], errors: ['missing.json'], legacy: 2 },
      { files: { 'app.json': '{"pages":["good"],"widgets":[{"path":"card","family":"1x1","placement":"invalid"}],"window":{"navigationBarTitleText":"Title"}}', 'good.ink': good },
        names: ['good'], errors: [], legacy: 1, widgetError: true, title: 'Title' },
      { files: { 'app.json': '{"pages":["good"],"window":false}', 'good.ink': good },
        names: ['good'], errors: [], legacy: 1 },
      { files: { 'app.json': '{"pages":[]}' }, names: [], errors: [], legacy: 0 },
      { files: { 'app.json': '{"pages":false}' }, names: [], errors: ['app.json'], legacy: 0 },
      { files: { 'app.json': '{}' }, names: [], errors: ['app.json'], legacy: 0 },
      { files: { 'app.json': '{"pages":["home"]}', 'home.wxml': '<view />' },
        names: [], errors: ['home.json'], legacy: 1 },
      { files: { 'app.json': '{"pages":["home"]}', 'home.json': '{}' },
        names: [], errors: ['home.wxml'], legacy: 1 },
      { files: { 'app.json': '{"pages":["bad","good"]}', 'bad.json': '{"schema":false}', 'bad.wxml': '<view />', 'good.ink': good },
        names: ['good'], errors: ['bad.json'], legacy: 2 },
      { files: { 'app.json': '{"pages":["bad","good"]}', 'bad.ink': '<script def>{"schema":false}</script><page><view /></page>', 'good.ink': good },
        names: ['good'], errors: ['bad.ink'], legacy: 2 },
      { files: { 'app.json': '{"pages":["home"]}', 'home.ink': '<page><view /></page>' },
        names: ['home'], errors: [], legacy: 1 },
      { files: { 'app.json': '{"pages":["home"]}', 'home.json': '{}', 'home.wxml': '<view class="card" />', 'home.wxss': '.card {width:160px;height:80px}' },
        names: ['home'], errors: [], legacy: 1, width: 160 },
    ];
    const results = [];
    for (const test of cases) {
      const { data } = await AIX.pack(Object.entries(test.files).map(([path, source]) => ({
        path, data: new TextEncoder().encode(source),
      })), { buildId: 'metadata', optimize: false });
      const reader = await AIX.From(data);
      const raw = new AixReaderWasm(data);
      let widgetError;
      try { reader.getWidgets(); }
      catch (error) { widgetError = String(error); }
      results.push({
        report: reader.resolveMetadata(), rawReport: raw.resolve_metadata(),
        legacyPages: reader.getPages().map((page) => page.name), legacyTools: reader.getTools(),
        widgetError, title: reader.getTitle(),
        expected: { ...test, files: undefined },
      });
      raw.free();
    }
    return results;
  });
  for (const { report, rawReport, legacyPages, legacyTools, widgetError, title, expected } of results) {
    assert.deepEqual(report, rawReport);
    assert.deepEqual(report.pages.map((page) => page.name), expected.names);
    assert.deepEqual(report.tools.map((tool) => tool.function.name), expected.names);
    assert.deepEqual(report.diagnostics.map((diagnostic) => diagnostic.path), expected.errors);
    assert(report.diagnostics.every((diagnostic) => diagnostic.message.length > 0));
    assert.equal(legacyPages.length, expected.legacy);
    assert.equal(legacyTools.length, expected.legacy);
    if (expected.targets) {
      assert.deepEqual(legacyPages, expected.names);
      assert.deepEqual(legacyTools, report.tools);
      assert.deepEqual(report.tools.map((tool) => tool.target), expected.targets);
    }
    if (expected.widgetError) assert.match(widgetError, /Invalid app.json/);
    if (expected.title) assert.equal(title, expected.title);
    if (expected.width) assert.equal(report.pages[0].size.width, expected.width);
  }
  console.log(`ok - ${results.length} browser SDK / raw WASM metadata resolution cases`);
};
