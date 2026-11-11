// Prepare local build tools and publish the Mac package in the same version
// Windows may already have published. No app, recording, or model is opened.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
if (process.platform !== 'darwin') throw Error('Esegui questo comando sul Mac.');
const tools = path.join(root, '.tools');
const env = { ...process.env };
if (!env.TAURI_SIGNING_PRIVATE_KEY && !['parlo.key', 'presentio.key'].some(key => fs.existsSync(path.join(tools, 'updater', key)))) {
  throw Error('Prima configurazione: copia privatamente in .tools/updater la chiave aggiornamenti già usata su Windows.');
}
function run(command, args, capture = false) {
  const result = spawnSync(command, args, { cwd: root, env, stdio: capture ? 'pipe' : 'inherit', encoding: 'utf8' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw Error(capture ? result.stderr.trim() : `${path.basename(command)} si è fermato (codice ${result.status}).`);
  return result.stdout?.trim() || '';
}

async function githubCLI() {
  const local = path.join(tools, 'gh-local/bin/gh');
  if (fs.existsSync(local)) return local;
  const installed = spawnSync('gh', ['--version'], { stdio: 'ignore' });
  if (!installed.error && installed.status === 0) return 'gh';
  console.log('Preparazione locale di GitHub CLI…');
  const response = await fetch('https://api.github.com/repos/cli/cli/releases/latest');
  if (!response.ok) throw Error(`Download GitHub CLI: HTTP ${response.status}`);
  const release = await response.json();
  if (!/^v\d+\.\d+\.\d+$/.test(release.tag_name)) throw Error('Versione GitHub CLI non valida.');
  const version = release.tag_name.slice(1);
  const name = `gh_${version}_macOS_${process.arch === 'arm64' ? 'arm64' : 'amd64'}.zip`;
  const prefix = `https://github.com/cli/cli/releases/download/${release.tag_name}/`;
  async function download(assetName) {
    const asset = release.assets.find(asset => asset.name === assetName);
    if (!asset || !asset.browser_download_url.startsWith(prefix)) throw Error('Pacchetto GitHub CLI non disponibile.');
    const result = await fetch(asset.browser_download_url);
    if (!result.ok || !result.url.startsWith('https://')) throw Error('Download GitHub CLI non riuscito.');
    const data = Buffer.from(await result.arrayBuffer());
    if (data.length !== asset.size) throw Error('Download GitHub CLI incompleto.');
    return data;
  }
  const checksums = (await download(`gh_${version}_checksums.txt`)).toString('utf8');
  const expected = checksums.split(/\r?\n/).map(line => line.trim().split(/\s+/)).find(parts => parts[1] === name)?.[0];
  const bytes = await download(name);
  if (!expected || createHash('sha256').update(bytes).digest('hex') !== expected) throw Error('Verifica GitHub CLI fallita.');
  const archive = path.join(tools, 'downloads', name);
  fs.writeFileSync(archive, bytes);
  const extracted = path.join(tools, `gh-${version}-${process.arch}`);
  const python = path.join(tools, 'mac-build-venv/bin/python');
  const unpack = 'import pathlib,sys,zipfile\nwith zipfile.ZipFile(sys.argv[1]) as z:\n for name in z.namelist():\n  path=pathlib.PurePosixPath(name)\n  if path.is_absolute() or ".." in path.parts: raise ValueError("Percorso ZIP non valido")\n z.extractall(sys.argv[2])';
  run(python, ['-c', unpack, archive, extracted]);
  const options = [path.join(extracted, 'bin/gh'), path.join(extracted, name.slice(0, -4), 'bin/gh')];
  const binary = options.find(file => fs.existsSync(file));
  if (!binary) throw Error('Il pacchetto GitHub CLI non contiene il programma atteso.');
  fs.mkdirSync(path.dirname(local), { recursive: true });
  fs.copyFileSync(binary, local);
  fs.chmodSync(local, 0o755);
  return local;
}

env.PARLO_PREPARE_ONLY = '1';
run(process.execPath, ['scripts/mac-test.mjs']);
delete env.PARLO_PREPARE_ONLY;
const gh = await githubCLI();
env.PATH = `${path.join(tools, 'gh-local/bin')}:${env.PATH || ''}`;
if (spawnSync(gh, ['auth', 'status'], { cwd: root, env, stdio: 'ignore' }).status !== 0) {
  console.log('Collega una volta il tuo account GitHub per leggere il progetto e caricare le release.');
  run(gh, ['auth', 'login', '--hostname', 'github.com', '--git-protocol', 'https', '--web']);
}
if (run('git', ['status', '--porcelain', '--untracked-files=normal'], true)) throw Error('La cartella contiene modifiche locali. Salvale prima di recuperare e pubblicare la versione condivisa.');
if (run('git', ['branch', '--show-current'], true) !== 'main') throw Error('Passa al ramo main del progetto prima di pubblicare.');
run('git', ['pull', '--ff-only', 'origin', 'main']);
run(process.execPath, ['scripts/local-release.mjs', 'ship']);
