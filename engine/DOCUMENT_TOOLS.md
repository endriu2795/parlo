# Strumenti per documenti e media

Nel JSON `tools` trovi i percorsi di Python, FFmpeg e dell'helper, oltre alle
versioni delle librerie incluse. Scegli gli strumenti in base alla richiesta.

L'helper è facoltativo. Offre `extract`, `validate`, `render-pdf`, `slideshow`
e `ffmpeg`. Per usarlo, esegui `tools.python` con `-I -X utf8`, seguito da
`tools.helper` e dal comando. Consulta `--help` per i parametri.
Su PowerShell avvia i percorsi tra virgolette con `&`. Tieni script e librerie
aggiuntive nella cartella di lavoro.

`python-pptx` non richiede Office per creare PPTX. MoviePy 2.x si importa da
`moviepy`. Per il rendering dei file Office, il ricalcolo Excel, l'OCR e la
generazione di riprese video AI servono strumenti dedicati.
