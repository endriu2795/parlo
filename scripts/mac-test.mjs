// No global npm, Python, Homebrew or Rust installation is required.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {Readable, Transform} from 'node:stream';
import {pipeline} from 'node:stream/promises';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const tools = path.join(root, '.tools');
if (process.platform !== 'darwin') throw Error('Questo kit richiede macOS.');
const arm = process.arch === 'arm64';
const platform = arm ? 'mac-arm64' : 'mac-x64';
const host = arm ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin';
process.env.RUSTUP_HOME = path.join(tools, 'rustup');
process.env.CARGO_HOME = path.join(tools, 'cargo');
process.env.npm_config_cache = path.join(tools, 'npm-cache');
process.env.PIP_CACHE_DIR = path.join(tools, 'pip-cache');
delete process.env.PYTHONHOME;
delete process.env.PYTHONPATH;

function run(binary, args) {
  const result = spawnSync(binary, args, {cwd:root, env:process.env, stdio:'inherit'});
  if (result.error) throw result.error;
  if (result.status !== 0) throw Error(`${path.basename(binary)} ha interrotto la preparazione (codice ${result.status}).`);
}
async function valid(file, sha, size) {
  if (!fs.existsSync(file) || (size && fs.statSync(file).size !== size)) return false;
  const hash = createHash('sha256');
  for await (const chunk of fs.createReadStream(file)) hash.update(chunk);
  return hash.digest('hex') === sha;
}
async function download(url, file, sha, size) {
  if (await valid(file, sha, size)) return;
  const response = await fetch(url, {signal:AbortSignal.timeout(15*60*1000)});
  if (!response.ok || !response.url.startsWith('https://')) throw Error(`Download non riuscito: ${response.status}`);
  const partial = file + '.part';
  const hash = createHash('sha256');
  let count = 0;
  try {
    await pipeline(Readable.fromWeb(response.body), new Transform({transform(chunk, _, callback) {
      count += chunk.length;
      if (size && count > size) return callback(Error('Dimensione download inattesa.'));
      hash.update(chunk); callback(null, chunk);
    }}), fs.createWriteStream(partial));
    if ((size && count !== size) || hash.digest('hex') !== sha) throw Error('Verifica di integrità del download non riuscita.');
    fs.renameSync(partial, file);
  } finally { fs.rmSync(partial, {force:true}); }
}

const manifest = JSON.parse(fs.readFileSync(path.join(root, 'engine/python-runtime.json'), 'utf8'));
const artifact = manifest.artifacts.find(a => a.platform === platform);
const pythonFolder = path.join(tools, `build-python-${manifest.version}-${platform}`);
const python = path.join(pythonFolder, 'python/bin/python3');
if (!fs.existsSync(python)) {
  console.log('Preparazione del Python privato per la compilazione…');
  const archive = path.join(tools, 'downloads', artifact.file);
  await download(artifact.url, archive, artifact.sha256, artifact.size);
  fs.mkdirSync(pythonFolder, {recursive:true});
  run('/usr/bin/tar', ['-xzf', archive, '-C', pythonFolder]);
}
run(python, ['-I', '-c', 'import sys,ssl,venv; print("Python locale:",sys.version.split()[0])']);
const venv = path.join(tools, 'mac-build-venv');
if (!fs.existsSync(path.join(venv, 'bin/python'))) run(python, ['-I', '-m', 'venv', venv]);
const buildPython = path.join(venv, 'bin/python');
run(buildPython, ['-m', 'pip', 'install', '--disable-pip-version-check', 'cmake==3.31.6']);
process.env.PATH = `${path.join(venv,'bin')}:${path.join(tools,'cargo/bin')}:${process.env.PATH}`;

const rustup = path.join(tools, 'cargo/bin/rustup');
if (!fs.existsSync(rustup)) {
  console.log('Preparazione del compilatore Rust locale…');
  const init = path.join(tools, 'downloads', `rustup-init-${host}`);
  const sha = arm ? '20ef5516c31b1ac2290084199ba77dbbcaa1406c45c1d978ca68558ef5964ef5' : '9c331076f62b4d0edeae63d9d1c9442d5fe39b37b05025ec8d41c5ed35486496';
  await download(`https://static.rust-lang.org/rustup/archive/1.28.2/${host}/rustup-init`, init, sha);
  fs.chmodSync(init, 0o755);
  run(init, ['-y', '--no-modify-path', '--profile', 'minimal', '--default-host', host, '--default-toolchain', '1.99.0']);
} else {
  run(rustup, ['toolchain', 'install', '1.99.0', '--profile', 'minimal']);
  run(rustup, ['default', '1.99.0']);
}
if (process.env.PARLO_PREPARE_ONLY === '1') process.exit(0);
console.log('Preparazione interfaccia e compilazione. La prima volta può richiedere diversi minuti…');
run('npm', ['ci', '--no-audit', '--no-fund']);
run(buildPython, ['scripts/build-mac.py', '--test']);
const app = path.join(root, 'src-tauri/target/debug/bundle/macos/PARLO.app');
if (!fs.existsSync(app)) throw Error('La compilazione non ha prodotto l’app attesa.');
fs.writeFileSync(path.join(root, 'test-results/esito.json'), JSON.stringify({platform:process.platform, architecture:process.arch, node:process.version, build:'debug', completed:new Date().toISOString()}, null, 2));
console.log('PARLO è pronto per la prova. Si apre l’app; il Finder mostra dove si trova.');
run('/usr/bin/open', ['-R', app]);
run('/usr/bin/open', [app]);
