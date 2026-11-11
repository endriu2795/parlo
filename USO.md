# Usare PARLO

App desktop per registrare call, trascriverle sul computer e lavorare sui contenuti con un assistente AI.

## Cosa fa

- Registra microfono e audio del computer, con un controllo compatto e scorciatoie configurabili.
- Importa un file audio o video dall’archivio: l’app estrae l’audio e lo trascrive localmente. Il file originale resta nella posizione scelta; l’audio estratto segue la preferenza di conservazione delle registrazioni.
- Trascrive localmente oppure tramite Groq cloud, con timestamp ed etichette delle sorgenti audio. La modalità locale offre anche l’anteprima opzionale durante la registrazione.
- Aggiunge note, screenshot di un’area selezionata o dello schermo intero e annotazioni alla call.
- Organizza le registrazioni in un archivio ricercabile, con filtro per date ed esportazione Markdown.
- Permette di discutere trascrizioni e materiali con **Claude Code o Codex**, usando il proprio account e istruzioni personalizzabili, oppure con **Ollama** come opzione esplicita.
- Collega file e cartelle alla singola chat oppure li condivide fra tutte le chat; le menzioni **@** permettono di richiamarli come reference.
- Mostra i file generati dall’assistente nella call: puoi scaricarli, trovarli sul disco e citarli nelle richieste successive.

Per ottenere un quiz JSON, un documento Word, una presentazione, un Excel, un PDF o uno slideshow MP4, chiedilo in chat e indica eventuali template fra le fonti. I pacchetti dell’app includono un ambiente privato per documenti e montaggio video, separato dalla trascrizione: non servono Python, Office o FFmpeg installati sul computer. Il controllo dei file Office è strutturale; il rendering Office, il ricalcolo delle formule e l’OCR non sono inclusi.

In **Impostazioni → Voce** puoi abilitare il TTS: Edge è gratuito e online, senza chiave API; ElevenLabs usa la tua chiave e i crediti del tuo account. La chiave è salvata nel Gestore credenziali Windows o nel Portachiavi macOS e non viene passata al modello o agli script. I testi da leggere vengono inviati al servizio scelto. La chat ottiene MP3, sottotitoli SRT e tempi delle parole con durata audio effettiva, utilizzabili per sincronizzare un video. Edge non garantisce disponibilità o utilizzo illimitato. Non serve abilitare i connettori esterni per usare la voce integrata.

Puoi anche scegliere **Piper locale**: il catalogo include Serena Standard (circa 64 MB) e Serena Alta qualità (circa 114 MB). Scarica esplicitamente la voce dalle impostazioni; i file vengono verificati con SHA-256. Dopo il download la generazione è offline, senza chiavi o account, con due thread CPU per il modello. Il processo conserva un solo modello tra le scene della sessione. Ricevi MP3 e durata reale; queste voci non producono tempi delle parole o SRT. Il motore è GPL-3.0-or-later e le voci Serena sono dichiarate CC-BY-4.0: riferimenti e attribuzioni sono in [PIPER-LICENSES.md](engine/PIPER-LICENSES.md).

Come alternativa locale più espressiva puoi selezionare **Qwen3-TTS 1.7B CustomVoice**, opzionale e separato dal modello chat di Ollama. Inizialmente disponibile su Windows x64: la preparazione richiede almeno 16 GB di RAM e 15 GB liberi; per la modalità GPU consigliata servono NVIDIA, almeno 8 GB di VRAM e driver compatibile con CUDA 12.8. Ollama può occupare la stessa VRAM. La CPU è selezionabile esplicitamente ma molto lenta, con 32 GB di RAM consigliati. Le soglie sono requisiti dell’integrazione, senza garanzia di prestazioni. **Scarica e prepara** installa i componenti in un ambiente privato e scarica circa 4,5 GB di modello verificato; niente viene installato selezionando soltanto il servizio. Dopo la preparazione funziona offline e offre nove voci multilingue, lingua e indicazioni di tono, MP3 e durata reale; non fornisce tempi delle parole o SRT. Non passa automaticamente a CPU o cloud in caso di errore. Licenze e revisione sono in [QWEN-LICENSES.md](engine/QWEN-LICENSES.md).

Gli strumenti voce esposti alla chat seguono esclusivamente il servizio attivo nelle impostazioni: istruzioni, parametri e cataloghi sono separati. Con Piper vengono elencate solo le voci già preparate; per la generazione vengono usati motore e voce configurati. `generate_speech` riceve testo e output; `generate_speech_from_file` riceve il percorso del copione UTF-8 e l’output. Gli errori nativi del servizio voce sono mostrati nell’attività della chat. Non viene imposto uno stop in base al solo numero di errori.

I selettori di voci e modelli mostrano nomi leggibili, lingua e scelta attiva. La ricerca non modifica la selezione. Edge ed ElevenLabs possono caricare il catalogo completo dal rispettivo servizio; gli ID personalizzati rimangono disponibili in una sezione separata.

## Slide post-call con il tuo template

1. In una chat, apri **Fonti → Aggiungi file condivisi** e seleziona il tuo template `.pptx`. Il file sarà disponibile tra le fonti di tutte le chat.
2. Dopo una call, apri la sua chat e verifica che il template sia spuntato. Digita `@` e selezionalo nella richiesta.

> Crea una nuova presentazione PPTX dalla trascrizione. Usa @Template.pptx come riferimento per layout, colori e tipografia. Includi le decisioni e i prossimi passi emersi nella call.

## Come iniziare

1. In **Releases** della repo GitHub ufficiale, scegli l’installer disponibile per il tuo sistema.
2. Scegli le sorgenti audio e avvia una registrazione. Al termine trovi la trascrizione nell’archivio.
3. Per usare la chat, scegli Claude Code o Codex in **Impostazioni → Assistente** e collega l’account. Aggiungi i materiali dal pannello **File e fonti**.

Gli aggiornamenti si controllano in **Impostazioni → Generali → Aggiornamenti**.

**Ollama è opzionale e disattivato di default.** In **Impostazioni → Assistente** scegli **Ollama**, installa [Ollama dal sito ufficiale](https://ollama.com/download/), avvialo sulla porta locale 11434 e seleziona un modello già scaricato tramite il normale menu a tendina. Lo stesso selettore è disponibile nella chat. Non vengono installati modelli automaticamente e i modelli cloud di Ollama sono esclusi, compresi gli alias remoti dichiarati dal server. Serve un modello con tool calling e la CLI Claude Code installata: viene riutilizzata per file, shell e conferme senza richiedere un account Claude. Le autorizzazioni e la configurazione della CLI sono separate dagli account cloud. Sono disponibili gli strumenti standard per file, Bash e PowerShell su Windows. Ricerca e apertura delle pagine sono disattivate per impostazione predefinita, anche dopo il passaggio dalla precedente configurazione: si riattivano in **Impostazioni → Assistente → Strumenti web con Ollama**. Attivandole, le query vengono inviate a DuckDuckGo, senza chiavi API, e le pagine richieste vengono contattate; WebFetch invia il solo dominio ad Anthropic (`api.anthropic.com`) per un controllo di sicurezza. Il modello continua a girare sul computer. Può essere temporaneamente limitata dal servizio; non aggira verifiche anti-bot. I connettori degli account cloud sono esclusi. La voce integrata segue invece il servizio TTS scelto. Se il server o il modello non sono disponibili, la richiesta si ferma senza ripiegare sul cloud. Velocità e affidabilità degli strumenti dipendono dal modello, dall’hardware e dal contesto configurato in Ollama; la documentazione di Ollama consiglia almeno 64k di contesto per Claude Code. Per mantenere locali anche trascrizione e voce, scegli la trascrizione sul computer e un servizio voce locale. Le shell e gli altri programmi autorizzati mantengono le proprie capacità di rete: l’interruttore controlla gli strumenti web integrati.

In **Impostazioni → Assistente → Memoria della conversazione Ollama** scegli il contesto: 32k token iniziali (il precedente valore predefinito di 16k viene aggiornato; gli altri valori personalizzati restano invariati), con valori proposti fino al massimo dichiarato dal modello e un campo personalizzato (almeno 8k). Non c’è un tetto fisso di 64k: modelli più capienti possono usare contesti maggiori, se la memoria del computer lo permette. Un contesto maggiore usa più RAM e può rallentare. PARLO crea una configurazione locale riservata che riusa i pesi del modello scelto, senza scaricarli nuovamente né cambiare le impostazioni globali di Ollama. Le configurazioni riservate non compaiono nel selettore. La chat mostra una stima del solo testo, con una riserva per la risposta: non viene aggiunto un costo fisso inventato per gli strumenti. La stima avvisa ma non blocca l’invio e non accorcia la trascrizione nell’app. Non è un conteggio esatto del tokenizer. La CLI riceve il contesto locale configurato e gestisce i propri limiti; una richiesta può essere compattata oppure fallire se il contesto reale non basta. Durante l’esecuzione compaiono i token di input comunicati dal backend, quando disponibili, e l’eventuale compattazione della cronologia eseguita dalla CLI; questa può riassumere dettagli precedenti senza modificare la registrazione originale. Il ragionamento esteso della CLI è disattivato per questo provider. Gli schemi degli strumenti completi occupano parte del contesto: prova almeno 32k se modello e hardware lo permettono. Il contesto non elimina i limiti di affidabilità dei modelli piccoli.

Il prompt comune è modificabile in **Impostazioni → Avanzate**. Il supporto Antigravity è stato rimosso a causa dei tempi di avvio e dei timeout del componente locale.

## Dove restano i dati

Registrazioni e contenuti della call sono salvati localmente. Per impostazione predefinita la trascrizione viene elaborata sul computer. In **Impostazioni → Registrazione → Dove trascrivere** puoi scegliere **Groq cloud**, con Whisper Large V3 Turbo e chiave API personale: dopo lo stop l’audio viene inviato a Groq in parti inferiori al limite di upload del piano gratuito. Si applicano le quote e le condizioni del tuo account; l’abbonamento a Claude o ChatGPT non comprende questo servizio. La chiave resta nel portachiavi del sistema e non viene passata all’assistente. In modalità cloud l’anteprima locale è disattivata e non serve scaricare un modello Whisper. Importazione e video usano comunque gli strumenti multimediali inclusi nell’app.

Durante l’elaborazione finale l’app impedisce lo standby automatico e permette allo schermo di spegnersi. Lo standby richiesto manualmente, la chiusura del portatile o la perdita della rete possono interrompere una richiesta cloud. In caso di errore, l’audio e le parti già completate rimangono disponibili: **Riprova** riutilizza le parti compatibili con audio, lingua e dizionario correnti. Non ci sono reinvii automatici; una richiesta interrotta potrebbe essere già conteggiata dal fornitore. Dopo un riavvio dell’app le elaborazioni interrotte risultano recuperabili dalla call. Per sospendere manualmente il computer è preferibile attendere il completamento.

Quando usi la chat con Claude Code o Codex, i contenuti necessari vengono inviati al servizio AI scelto. Con l’opzione Ollama l’inferenza usa esclusivamente il server sul computer; STT, TTS e le operazioni esterne autorizzate mantengono le rispettive impostazioni.

I file generati nella call seguono la sua conservazione e cancellazione. I file e le cartelle collegati come fonti rimangono nel percorso originale: rimuovere un collegamento dall’app non li cancella. Le copie scaricate altrove rimangono indipendenti.

## Sviluppo

Per compilare e modificare l'app, segui [SVILUPPO.md](SVILUPPO.md).
Licenza del progetto e componenti inclusi: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
