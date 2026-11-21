# Strumenti per documenti e media

Il JSON `tools` contiene i percorsi di Python, FFmpeg e dell’helper, con le
versioni delle librerie disponibili. Usa questi percorsi per eseguire gli
strumenti nella cartella di lavoro.

L’helper offre i comandi `extract`, `validate`, `render-pdf`, `slideshow` e
`ffmpeg`. Per usarlo, esegui `tools.python` con `-I -X utf8`, seguito da
`tools.helper` e dal comando. `--help` elenca i parametri. Puoi anche usare
direttamente le librerie incluse.

In PowerShell, anteponi `&` ai percorsi eseguibili tra virgolette. Salva
script e librerie aggiuntive nella cartella di lavoro.

`python-pptx` crea file PPTX senza Office. Con MoviePy 2.x importa da
`moviepy`. Rendering dei file Office, ricalcolo Excel, OCR e generazione di
riprese video AI richiedono strumenti dedicati.
