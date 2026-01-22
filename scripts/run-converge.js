const path = require('path');
const { spawnSync } = require('child_process');

const repoRoot = path.resolve(__dirname, '..');
const manifestPath = path.join(repoRoot, 'Cargo.toml');

// npm/pnpm set INIT_CWD to the directory the user invoked the command from.
// We want `converge` to discover `.converge` relative to that location.
const desiredCwd = process.env.INIT_CWD || process.cwd();

const args = process.argv.slice(2);
if (args[0] === '--') {
  args.shift();
}

const build = spawnSync('cargo', ['build', '--manifest-path', manifestPath, '--bin', 'converge'], {
  stdio: 'inherit',
  cwd: repoRoot,
  env: process.env,
});
if (typeof build.status !== 'number' || build.status !== 0) {
  process.exit(typeof build.status === 'number' ? build.status : 1);
}

const exeName = process.platform === 'win32' ? 'converge.exe' : 'converge';
const exePath = path.join(repoRoot, 'target', 'debug', exeName);

const run = spawnSync(exePath, args, {
  stdio: 'inherit',
  cwd: desiredCwd,
  env: process.env,
});

process.exit(typeof run.status === 'number' ? run.status : 1);
