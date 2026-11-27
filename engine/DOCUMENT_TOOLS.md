# Strumenti per documenti e media

Il JSON `tools` contiene i percorsi di Python e dell’helper, con le
versioni delle librerie disponibili. Usa questi percorsi per eseguire gli
strumenti nella cartella di lavoro.

L’helper offre i comandi `extract`, `validate`, `render-pdf`, `slideshow` e
`ffmpeg`. Per usarlo, esegui `tools.python` con `-I -X utf8`, seguito da
`tools.helper` e dal comando. `--help` elenca i parametri.

Per convertire o tagliare un video, estrarne l’audio o aggiungere una traccia,
usa il sottocomando `ffmpeg -i <file> --output <risultato>`. Le opzioni
`--start` e `--duration` usano secondi. `--audio` indica una traccia separata.
Per gli slideshow, crea il manifest nella cartella della call. Gli strumenti
multimediali leggono i file della call e le fonti condivise abilitate, e salvano
i risultati nella cartella della call.

In PowerShell, anteponi `&` ai percorsi eseguibili tra virgolette. Salva
script e librerie aggiuntive nella cartella di lavoro.

`python-pptx` crea file PPTX senza Office. FFmpeg legge i fotogrammi e verifica
i video MP4. Rendering dei file Office, ricalcolo Excel, OCR e generazione di
riprese video AI richiedono strumenti dedicati.
