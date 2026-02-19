const path = require('path');
const fs = require('fs');
const { spawnSync } = require('child_process');

const repoRoot = path.resolve(__dirname, '..');
const manifestPath = path.join(repoRoot, 'Cargo.toml');

function firstExistingDir(candidates) {
  for (const candidate of candidates) {
    if (!candidate || typeof candidate !== 'string') {
      continue;
    }
    const resolved = path.resolve(candidate);
    if (fs.existsSync(resolved) && fs.statSync(resolved).isDirectory()) {
      return resolved;
    }
  }
  return process.cwd();
}

// Package managers may launch scripts from repo root while preserving
// invocation context in env vars. Prefer those so server-relative paths
// resolve from where the command was run.
const desiredCwd = firstExistingDir([
  process.env.INIT_CWD, // npm-compatible runners
  process.env.npm_config_local_prefix, // bun/npm compatibility var
  process.env.PWD, // shell invocation directory
  process.cwd(),
]);

const args = process.argv.slice(2);
if (args[0] === '--') {
  args.shift();
}

const build = spawnSync(
  'cargo',
  ['build', '--manifest-path', manifestPath, '--bin', 'converge-server'],
  {
    stdio: 'inherit',
    cwd: repoRoot,
    env: process.env,
  }
);
if (typeof build.status !== 'number' || build.status !== 0) {
  process.exit(typeof build.status === 'number' ? build.status : 1);
}

const exeName = process.platform === 'win32' ? 'converge-server.exe' : 'converge-server';
const exePath = path.join(repoRoot, 'target', 'debug', exeName);

const run = spawnSync(exePath, args, {
  stdio: 'inherit',
  cwd: desiredCwd,
  env: process.env,
});

process.exit(typeof run.status === 'number' ? run.status : 1);
