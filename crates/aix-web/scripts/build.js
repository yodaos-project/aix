const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
const { writeEnvironment } = require('./build-check');

const rootDir = path.resolve(__dirname, '..');
const distDir = path.join(rootDir, 'dist');
const cargoPath = path.join(rootDir, 'Cargo.toml');
const packagePath = path.join(rootDir, 'package.json');

// 1. Get version from Cargo.toml
console.log('Reading version from Cargo.toml...');
const cargoContent = fs.readFileSync(cargoPath, 'utf-8');
const versionMatch = cargoContent.match(/^version\s*=\s*"([^"]+)"/m);
if (!versionMatch) {
  throw new Error('Could not find version in Cargo.toml');
}
const version = versionMatch[1];
console.log(`Detected version: ${version}`);

// 2. Clean dist (but keep the dist directory itself because wasm-pack outputs to its subdirectory)
if (fs.existsSync(distDir)) {
  fs.rmSync(distDir, { recursive: true, force: true });
}
fs.mkdirSync(distDir, { recursive: true });

// 3. Build WASM (outputs directly to dist/pkg)
console.log('Building WASM to dist/pkg...');
execSync('npm run build:wasm', { cwd: rootDir, stdio: 'inherit' });

// Remove .gitignore in dist/pkg, otherwise npm publish will ignore this directory
const pkgGitignore = path.join(distDir, 'pkg', '.gitignore');
if (fs.existsSync(pkgGitignore)) {
  fs.unlinkSync(pkgGitignore);
  console.log('Removed dist/pkg/.gitignore');
}

const wasmPackagePath = path.join(distDir, 'pkg', 'package.json');
if (fs.existsSync(wasmPackagePath)) {
  const wasmPackage = JSON.parse(fs.readFileSync(wasmPackagePath, 'utf-8'));
  wasmPackage.type = 'module';
  fs.writeFileSync(wasmPackagePath, JSON.stringify(wasmPackage, null, 2));
}

// 4. Generate the checker environment before type-checking the SDK, then
// bundle the optional compiler so consumers do not need a Node filesystem.
console.log('Building shared checker...');
writeEnvironment();

// Build TS (outputs to dist/)
console.log('Building TS to dist/...');
execSync('npm run build:ts', { cwd: rootDir, stdio: 'inherit' });
execSync('node -e "require(\'./scripts/build-check\').buildCheck(\'dist/check/index.js\')"', { cwd: rootDir, stdio: 'inherit' });
// These implementation modules are now inside the standalone checker bundle.
for (const name of ['typescript', 'results', 'environment.generated']) {
  for (const extension of ['js', 'd.ts']) {
    fs.rmSync(path.join(distDir, 'check', `${name}.${extension}`), { force: true });
  }
}

// 5. Fix import paths
console.log('Fixing import paths in dist/index.js...');
const distIndexJs = path.join(distDir, 'index.js');
const distIndexDts = path.join(distDir, 'index.d.ts');

[distIndexJs, distIndexDts].forEach((file) => {
  if (fs.existsSync(file)) {
    let content = fs.readFileSync(file, 'utf-8');
    content = content.replace(/\.\.\/dist\/pkg\//g, './pkg/');
    fs.writeFileSync(file, content);
  }
});

// 6. Generate package.json for publishing
console.log('Generating dist/package.json...');
const packageJson = JSON.parse(fs.readFileSync(packagePath, 'utf-8'));
const publishPackageJson = {
  name: packageJson.name,
  version: version,
  description: packageJson.description || 'Ink AIX Web Library',
  type: 'module',
  main: 'index.js',
  types: 'index.d.ts',
  files: ['*'],
  dependencies: packageJson.dependencies,
  author: packageJson.author,
  license: packageJson.license,
  repository: packageJson.repository,
  publishConfig: {
    access: 'public',
    registry: 'https://registry.npmjs.com/',
  },
};
fs.writeFileSync(path.join(distDir, 'package.json'), JSON.stringify(publishPackageJson, null, 2));

// 7. Generate jsr.json for publishing
console.log('Generating dist/jsr.json...');
const jsrJson = {
  name: packageJson.name,
  version: version,
  exports: './index.js',
};
fs.writeFileSync(path.join(distDir, 'jsr.json'), JSON.stringify(jsrJson, null, 2));

// 8. Copy README.md to dist
console.log('Copying README.md to dist...');
const readmePath = path.join(rootDir, 'README.md');
if (fs.existsSync(readmePath)) {
  fs.copyFileSync(readmePath, path.join(distDir, 'README.md'));
}

console.log('Build completed successfully! Output is in dist/');
