// Cross-platform npm entry point. Never prints signing keys or tokens.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const env = { ...process.env };
const windows = process.platform === 'win32';
const cargo = path.join(root, '.tools/cargo');
const rustup = path.join(root, '.tools/rustup');
const bin = path.join(cargo, 'bin');
const macVenv = path.join(root, '.tools/mac-build-venv/bin');
const pathKey = Object.keys(env).find(key => key.toLowerCase() === 'path') || 'PATH';
env[pathKey] = [bin, ...(windows ? [] : [macVenv]), env[pathKey] || ''].join(path.delimiter);
if (fs.existsSync(rustup) && fs.existsSync(bin)) {
  env.CARGO_HOME = cargo;
  env.RUSTUP_HOME = rustup;
}
const candidates = windows
  ? [[path.join(root, '.tools/engine/venv/Scripts/python.exe')], ['py', '-3.12'], ['python']]
  : [[path.join(macVenv, 'python')], ['python3.12'], ['python3']];
let python;
for (const command of candidates) {
  const result = spawnSync(command[0], [...command.slice(1), '-c', 'import sys; sys.exit(0 if sys.version_info[:2] == (3, 12) else 1)'], { cwd: root, env, windowsHide: true, stdio: 'ignore' });
  if (!result.error && result.status === 0) { python = command; break; }
}
if (!python) {
  console.error('Serve Python 3.12 per preparare i pacchetti. Installa Python 3.12 oppure usa l’ambiente locale .tools del progetto.');
  process.exit(1);
}
const result = spawnSync(python[0], [...python.slice(1), 'scripts/local-release.py', ...process.argv.slice(2)], { cwd: root, env, windowsHide: true, stdio: 'inherit' });
if (result.error) console.error(result.error.message);
process.exit(result.status ?? 1);
