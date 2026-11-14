// Windows QA, Node 22.13+, local Whisper runtime and a default audio output.
// PowerShell: $env:TAURI_CONFIG = '{"identifier":"app.parlo.desktop.qa","plugins":{"updater":null}}'
// cargo build --manifest-path src-tauri/Cargo.toml --bin callnotes
// node scripts/desktop-qa.cjs
const assert = require('node:assert/strict');
const { spawn } = require('node:child_process');
const { mkdtempSync, mkdirSync, readFileSync, realpathSync, writeFileSync, renameSync, rmSync } = require('node:fs');
const { createServer: createHttpServer } = require('node:http');
const { dirname, join, resolve } = require('node:path');
const { randomUUID } = require('node:crypto');
const { DatabaseSync } = require('node:sqlite');
const { setTimeout: delay } = require('node:timers/promises');
const { chromium } = require('playwright');

const project = resolve(__dirname, '..');
const engine = join(project, '.tools', 'engine');
const qa = join(project, '.tools', 'qa');
const binary = join(project, 'src-tauri', 'target', 'debug', 'callnotes.exe');
const timestamp = '2026-01-01T10:00:00Z';
const qaIdentifier = 'app.parlo.desktop.qa';
const recordingOnly = process.argv.includes('--recording-only');
const chatOnly = process.argv.includes('--chat-only');

function writeShared(folder, id, publisher, title, text) {
  const metadata = { id, publisher_id: publisher, publisher: 'Autore QA', title,
    created_at: timestamp, published_at: new Date().toISOString(), duration_ms: 5000 };
  const path = join(folder, `parlo-${id}.md`);
  const partial = `${path}.part`;
  writeFileSync(partial, `<!-- PARLO-SHARE-V1 ${Buffer.from(JSON.stringify(metadata)).toString('base64')} -->\n\n**[00:00] Speaker**\n\n${text}\n`);
  renameSync(partial, path);
}

async function run() {
  assert.ok(readFileSync(binary).includes(qaIdentifier), 'Compila il binario di sviluppo con l’identificatore QA indicato in testa allo script.');
  mkdirSync(qa, { recursive: true });
  const root = mkdtempSync(join(qa, 'desktop-test-'));
  assert.equal(dirname(realpathSync(root)), realpathSync(qa));
  const folder = join(root, 'shared');
  mkdirSync(folder);
  const db = new DatabaseSync(join(root, 'archive.sqlite'));
  const peer = randomUUID();
  const sharedId = randomUUID();
  const requests = [];
  const assistant = createHttpServer(async (request, response) => {
    if (request.method !== 'POST' || request.url !== '/v1/chat/completions') {
      response.writeHead(404).end();
      return;
    }
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    let payload;
    try { payload = JSON.parse(Buffer.concat(chunks)); }
    catch { response.writeHead(400).end(); return; }
    if (payload.model !== 'qa') { response.writeHead(400).end(); return; }
    requests.push(payload);
    response.writeHead(200, { 'content-type': 'text/event-stream' });
    const delta = { content: `Risposta desktop QA ${requests.length}` };
    response.end(`data: ${JSON.stringify({ choices: [{ delta, finish_reason: null }] })}\n\ndata: [DONE]\n\n`);
  });
  await new Promise(resolve => assistant.listen(0, '127.0.0.1', resolve));
  const settings = {
    retention_months: 0, keep_audio: false, live_transcription: false, language: 'en',
    microphone: false, system_audio: true, completion_notifications: false,
    sharing: { folder, name: 'PARLO QA', read_shared: true },
    assistant_provider: 'custom', custom_chat: { model: 'qa', url: `http://127.0.0.1:${assistant.address().port}/v1` },
  };
  db.exec('CREATE TABLE settings (id INTEGER PRIMARY KEY, json TEXT NOT NULL)');
  db.prepare('INSERT INTO settings VALUES (1, ?)').run(JSON.stringify(settings));
  // Fill the isolated archive before starting PARLO. The app applies its migrations.
  db.exec('CREATE TABLE calls (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at TEXT NOT NULL, duration_ms INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL, error TEXT)');
  const insert = db.prepare("INSERT INTO calls VALUES (?, ?, ?, 45000, 'ready', NULL)");
  db.exec('BEGIN');
  for (let index = 0; index < 480; index++) insert.run(randomUUID(), `Call QA ${index}`, new Date(Date.parse(timestamp) - index * 86400000).toISOString());
  db.exec('COMMIT');
  writeShared(folder, sharedId, peer, 'Condivisa QA', 'Testo condiviso iniziale');

  let app;
  let vite;
  let browser;
  let playback;
  let appError = '';
  try {
    const { createServer } = await import('vite');
    vite = await createServer({ server: { host: '127.0.0.1', port: 1420, strictPort: true } });
    await vite.listen();
    app = spawn(binary, [], {
      cwd: project, windowsHide: true,
      env: { ...process.env, CALLNOTES_DATA_DIR: root, CALLNOTES_ENGINE_DIR: engine,
        HF_HUB_OFFLINE: '1', TRANSFORMERS_OFFLINE: '1',
        WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=19241 --remote-debugging-address=127.0.0.1' },
    });
    app.stderr.on('data', data => { appError = (appError + data).slice(-2000); });
    app.once('error', error => { appError = error.message; });
    const started = Date.now();
    while (!browser && Date.now() - started < 30000) {
      if (app.exitCode !== null || !app.pid) throw Error(`PARLO non è partito (${app.exitCode}). ${appError}`);
      browser = await chromium.connectOverCDP('http://127.0.0.1:19241', { timeout: 1200 }).catch(() => undefined);
      if (!browser) await delay(350);
    }
    assert.ok(browser, `WebView2 non raggiungibile. ${appError}`);
    const page = await waitUntil(() => browser.contexts().flatMap(context => context.pages())
      .find(candidate => candidate.url() === 'http://127.0.0.1:1420/'), 15000, 'Finestra principale');
    page.setDefaultTimeout(12000);
    const button = (it, en) => page.getByRole('button', { name: new RegExp(`${it}|${en}`, 'i') }).first();
    if (!chatOnly) await page.locator('.record-button:enabled').first().waitFor({ timeout: 45000 });

    if (!recordingOnly) {

      const list = page.locator('.call-list');
      if (!chatOnly) {
        const spacer = list.locator('.virtual-archive-rows');
        for (let count = 20; count <= 460; count += 10) {
          const height = await spacer.evaluate(element => element.offsetHeight);
          await list.locator('.load-more:enabled').evaluate(element => element.click());
          await page.waitForFunction(previous => document.querySelector('.virtual-archive-rows')?.offsetHeight > previous, height, { timeout: 10000 });
        }
        const profile = await list.evaluate(async element => {
          const elements = () => element.querySelectorAll('article').length;
          const frames = [];
          let peak = 0;
          let last = await new Promise(requestAnimationFrame);
          for (let i = 1; i <= 90; i++) {
            element.scrollTop = (element.scrollHeight - element.clientHeight) * i / 110;
            const now = await new Promise(requestAnimationFrame);
            frames.push(now - last);
            last = now;
            peak = Math.max(peak, elements());
          }
          frames.sort((a, b) => a - b);
          return { peak, frameP95Ms: Number(frames[85].toFixed(1)), over25ms: frames.filter(frame => frame > 25).length };
        });
        assert.ok(profile.peak < 40);
        console.log('Scroll WebView2', profile);

        const search = page.locator('.archive-search input');
        await search.fill('Call QA 479');
        await list.getByText('Call QA 479', { exact: true }).waitFor();
        await search.fill('');
      }
      await button('Condivise', 'Shared').click();
      await list.getByText('Condivisa QA', { exact: true }).click();
      await page.getByText('Testo condiviso iniziale', { exact: true }).waitFor();
      writeShared(folder, sharedId, peer, 'Condivisa QA aggiornata', 'Testo condiviso aggiornato');
      await page.getByText('Testo condiviso aggiornato', { exact: true }).waitFor({ timeout: 42000 });
      console.log('Aggiornamento della cartella condivisa rilevato da PARLO.');
      const composer = page.locator('.assistant-panel textarea').first();
      await composer.fill('Domanda desktop QA');
      await button('Invia messaggio', 'Send message').click();
      await page.getByText('Risposta desktop QA 1', { exact: true }).waitFor();
      assert.match(JSON.stringify(requests[0].messages), /Testo condiviso aggiornato/);
      assert.equal(requests[0].tools, undefined);

      const sources = button('File e fonti della chat', 'Chat files and sources');
      await sources.click();
      await page.getByRole('checkbox', { name: /^(Usa Trascrizione|Use Transcript)$/i }).uncheck();
      await sources.click();
      await composer.fill('Domanda senza trascrizione');
      await button('Invia messaggio', 'Send message').click();
      await page.getByText('Risposta desktop QA 2', { exact: true }).waitFor();
      assert.doesNotMatch(JSON.stringify(requests[1].messages), /Testo condiviso aggiornato|Trascrizione di riferimento/);
      assert.equal(requests[1].tools, undefined);
      console.log('Assistente HTTP locale verificato con trascrizione inclusa ed esclusa.');
    }

    if (chatOnly) return;

    // Play only speech from the public benchmark through the Windows output.
    const sample = readFileSync(join(engine, 'benchmark.wav'));
    assert.equal(sample.toString('ascii', 0, 4), 'RIFF');
    const speech = join(root, 'public-sample.wav');
    writeFileSync(speech, sample);
    await page.locator('.record-button').first().click();
    try {
      await page.locator('.record-button.recording').first().waitFor({ timeout: 5000 });
    } catch (error) {
      const alerts = await page.getByRole('alert').allTextContents();
      if (alerts.length) throw Error(alerts.join('\n'));
      throw error;
    }
    playback = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command',
      '$player=[System.Media.SoundPlayer]::new($env:PARLO_QA_WAV); try { $player.PlayLooping(); Start-Sleep -Seconds 12 } finally { $player.Stop(); $player.Dispose() }'],
    { windowsHide: true, env: { ...process.env, PARLO_QA_WAV: speech } });
    await new Promise((resolve, reject) => {
      playback.once('error', reject);
      playback.once('exit', code => code === 0 ? resolve() : reject(Error(`Riproduzione uscita con codice ${code}`)));
    });
    await page.locator('.record-button.recording').first().click();
    await page.locator('.processing-cancel').click({ timeout: 12000 });
    const recording = await waitUntil(() => db.prepare("SELECT id, error FROM calls WHERE origin='recording' AND status='failed'").get(), 20000, 'Annullamento');
    assert.match(recording.error, /Elaborazione interrotta/);
    await button('Riprova la trascrizione', 'Retry transcription').click();
    await waitUntil(() => db.prepare('SELECT status FROM calls WHERE id=?').get(recording.id)?.status === 'ready', 120000, 'Trascrizione dopo la riprova');
    assert.ok(db.prepare('SELECT count(*) AS count FROM segments WHERE call_id=?').get(recording.id).count > 0);
    console.log('Registrazione, annullamento e riprova verificati nella UI desktop.');
  } finally {
    if (playback && playback.exitCode === null) playback.kill();
    if (browser) await browser.close().catch(() => {});
    if (app?.pid && app.exitCode === null && app.signalCode === null) {
      const exited = new Promise(resolve => app.once('exit', resolve));
      app.kill();
      await Promise.race([exited, delay(5000)]);
    }
    if (vite) await vite.close();
    assistant.close();
    db.close();
    if (appError) console.error(appError);
    if (dirname(realpathSync(root)) === realpathSync(qa)) {
      rmSync(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 300 });
    }
  }
}

async function waitUntil(read, timeout, label) {
  const deadline = Date.now() + timeout;
  do {
    const value = read();
    if (value) return value;
    await delay(150);
  } while (Date.now() < deadline);
  throw Error(`${label}: tempo scaduto`);
}

run().catch(error => { console.error(error); process.exitCode = 1; });
