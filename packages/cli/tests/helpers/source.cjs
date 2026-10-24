const path = require('node:path');
const { buildSync } = require('esbuild');

// Unit tests exercise exported source APIs with the same bundling and WASM
// adjacency as the CLI. Inline maps also let c8 merge these executions with the
// command integration tests and Worker rather than reporting generated code.
function source(relative, external = []) {
  const root = path.resolve(__dirname, '../..');
  const outfile = path.join(root, 'dist', `test-${relative.replace(/\W/g, '-')}.cjs`);
  buildSync({
    entryPoints: [path.join(root, 'src', relative)], outfile,
    bundle: true, platform: 'node', format: 'cjs', target: 'node20',
    sourcemap: 'inline', external: ['ignore', 'ws', ...external],
  });
  delete require.cache[outfile];
  return { ...require(outfile) };
}

module.exports = { source };
