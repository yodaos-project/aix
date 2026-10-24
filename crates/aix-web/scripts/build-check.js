const path = require('node:path');

const root = path.resolve(__dirname, '..');

/** Build the same filesystem-free check adapter for both consumers. */
async function buildCheck(outfile, format = 'esm') {
  const { build } = require(require.resolve('esbuild', { paths: [root, path.resolve(root, '../../packages/cli')] }));
  await build({
    entryPoints: [path.join(root, 'ts/check/index.ts')],
    outfile,
    bundle: true,
    format,
    platform: 'browser',
    target: 'es2020',
    minifySyntax: true,
    legalComments: 'eof',
  });
}

module.exports = { buildCheck };
