# PARLO on Mac

[Italiano](MACOS.md)

PARLO requires macOS 13 or later, on Apple Silicon or Intel. It records the
microphone through the Mac's audio devices and system audio through
ScreenCaptureKit. There is no official installer yet: the app is built from
source.

## Build

Follow the steps in the [Mac build guide](DEVELOPMENT.md#macos-apple-silicon-or-intel).
It explains how to open the folder in Terminal and find the app in Finder.

## First launch

1. Open **PARLO.app** and prepare the model in **Settings → Recording**.
2. On the first recording, grant **Microphone** and **Screen Recording** in
   **Privacy & Security**. The second permission is also needed to capture
   system audio only.
3. Reopen PARLO if macOS asks you to. If the permissions stay tied to an older
   build, use **Reset permissions and restart** in **Settings → Recording**.

## Test kit

If you received the test ZIP, extract it and open **Prepara PARLO.command**.
If Finder blocks it, open **PRIMO AVVIO MAC.html** and follow the kit's
instructions. If preparation fails after starting, check
`Diagnostica-PARLO-Mac.zip`. To reopen the app, use **Avvia PARLO.command**.

## Testing status

The Mac version has not yet been thoroughly tested on physical hardware.
Still to be verified: startup, permissions, microphone and system audio
recording, device switching, local transcription, and installer signing.
Reports and contributions are welcome. Distributing a DMG requires a
Developer ID and notarization.
[macOS signing with Tauri](https://v2.tauri.app/distribute/sign/macos/)
