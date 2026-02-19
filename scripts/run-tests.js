const { spawnSync } = require('child_process');

function run(cmd, args, opts) {
  const res = spawnSync(cmd, args, { stdio: 'inherit', ...opts });
  if (typeof res.status === 'number') process.exit(res.status);
  process.exit(1);
}

function hasNextest() {
  const res = spawnSync('cargo', ['nextest', '--version'], { stdio: 'ignore' });
  return typeof res.status === 'number' && res.status === 0;
}

const args = process.argv.slice(2);

function mapArgsForNextest(argv) {
  // Package-manager test runners often forward `-q` to the script.
  // `cargo test -q` accepts it, but nextest does not.
  // Map to nextest's equivalent `--cargo-quiet`.
  const out = [];
  let passthrough = false;
  for (const a of argv) {
    if (a === '--') {
      passthrough = true;
      out.push(a);
      continue;
    }
    if (!passthrough && (a === '-q' || a === '--quiet')) {
      out.push('--cargo-quiet');
      continue;
    }
    out.push(a);
  }
  return out;
}

if (process.env.FORCE_CARGO_TEST === '1') {
  run('cargo', ['test', ...args]);
}

if (hasNextest()) {
  run('cargo', ['nextest', 'run', ...mapArgsForNextest(args)]);
} else {
  run('cargo', ['test', ...args]);
}
