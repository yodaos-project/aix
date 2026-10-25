const assert = require('node:assert/strict');

module.exports = async (page) => {
  const result = await page.evaluate(async () => {
    const { AIX } = window;
    const files = [{ path: 'app.json', data: new TextEncoder().encode('{"pages":[]}') }];
    const builds = [
      await AIX.pack(files, { buildId: 'first', agentId: 'stable-project' }),
      await AIX.packFromSource(files, { buildId: 'second', agentId: 'stable-project' }),
      await AIX.packFromSource(files, { buildId: 'third', agentId: 'stable-project', onProgress() {} }),
      await AIX.packFromFiles([new File(['{"pages":[]}'], 'app.json')], { buildId: 'fourth', agentId: 'stable-project' }),
    ];
    builds.push(await AIX.optimize(builds[0].data));
    const metadata = [];
    for (const { data } of builds) {
      const reader = await AIX.From(data);
      metadata.push({ id: reader.getAgentId(), version: reader.getVersion() });
    }
    const legacy = await AIX.From((await AIX.pack(files, { buildId: 'legacy' })).data);
    const legacyMissing = legacy.getAgentId() === undefined;
    const failures = [];
    for (const agentId of ['', '!!!', 'UPPER', '-bad']) {
      try { await AIX.pack(files, { agentId }); }
      catch (error) { failures.push(String(error)); }
    }
    return { metadata, legacyMissing, failures };
  });
  assert.deepEqual(result.metadata, ['first', 'second', 'third', 'fourth', 'first'].map((version) => ({ id: 'stable-project', version })));
  assert(result.legacyMissing);
  assert.equal(result.failures.length, 4);
  assert(result.failures.every((error) => error.includes('Invalid AIX manifest agent_id')));
  console.log('ok - Web SDK packs stable identity across entry points and preserves it during optimization');
};
