# PARLO

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="src/assets/parlo-lockup-white.svg">
  <img src="src/assets/parlo-lockup-indigo.svg" width="200" alt="PARLO">
</picture>

[English](README.en.md)

PARLO è un’app desktop gratuita per **registrare le call e ritrovarle nel tempo**.
Avvia la registrazione del microfono e dell’audio del computer con una
scorciatoia da tastiera configurabile, anche mentre usi un’altra app.
A fine call, la trascrizione entra nel tuo archivio
locale. Puoi cercare le parole pronunciate, aggiungere tag e filtrare per tag
o data.

## Altre funzioni

- Importa file audio e video da trascrivere.
- Trascrivi interamente sul computer oppure in cloud con la tua chiave per
  un servizio supportato.
- Aggiungi note e screenshot alla call ed esporta la trascrizione in Markdown.
- Scegli un assistente AI locale o cloud per fare domande sulla call e creare
  documenti, presentazioni e slideshow a partire dalle fonti che scegli.
- Genera audio da un testo con un motore locale o un servizio di sintesi
  vocale online.

## Installare e iniziare

- **Windows x64.** Scarica il file `.exe`. Per ora l’installer è privo di
  firma Authenticode. Windows può mostrare un avviso o bloccarlo in base alle
  impostazioni di sicurezza. [Releases ufficiali](https://github.com/endriu2795/parlo/releases)
- **macOS 13+.** Per ora puoi compilare l’app dai sorgenti su Apple Silicon o
  Intel. [Istruzioni per macOS](SVILUPPO.md#mac-apple-silicon-o-intel)

Scegli le sorgenti audio e avvia una registrazione. Per trascrivere, prepara
il modello locale oppure configura un servizio cloud supportato. Scegli
l’assistente per la chat nelle impostazioni.

Gli installer includono Python e gli strumenti per documenti e media.
I servizi esterni opzionali possono prevedere costi e quote propri.

Per il collaudo e i limiti su Mac, vedi [lo stato della build macOS](MACOS.md).

## Guide

- [Uso dell'app, assistenti, voce e dati](USO.md).
- [Compilare il codice e creare il proprio installer su Windows o Mac](SVILUPPO.md).
- [Riutilizzare un template PowerPoint in tutte le chat](USO.md#slide-post-call-con-il-tuo-template).
- [Licenze e componenti di terze parti](THIRD_PARTY_NOTICES.md).

## Dati e servizi

L’archivio resta sul tuo computer. PARLO lo salva senza cifratura. Con la
trascrizione locale, l’audio viene elaborato sul dispositivo. Se scegli un
servizio cloud, la voce online o uno strumento web, i dati necessari
all’operazione vengono inviati al servizio scelto. [Guida d’uso e servizi supportati](USO.md)

## Licenza

Il codice originale di PARLO è distribuito sotto
**GNU General Public License versione 3 (GPL-3.0-only)**: puoi usarlo,
modificarlo e redistribuirlo rispettandone le condizioni. Il testo integrale
è in [LICENSE](LICENSE). Il software viene fornito senza garanzia.

I componenti, modelli e servizi di terzi conservano le proprie licenze e
condizioni; non vengono rilicenziati dalla GPL del progetto.
Attribuzioni e riferimenti sono in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
