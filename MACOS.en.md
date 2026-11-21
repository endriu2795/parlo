# PARLO on Mac

[Italiano](MACOS.md)

PARLO runs on macOS 13 or later, on Apple Silicon or Intel. Build the Mac
version from source; an official installer is not yet available.

## Build

The [Mac build guide](DEVELOPMENT.md#macos-apple-silicon-or-intel) contains the
commands to prepare dependencies, build, and open **PARLO.app**.

## First launch

1. Open **PARLO.app**. If you use local transcription, prepare the model in
   **Settings → Recording**.
2. On the first recording, grant **Microphone** and **Screen Recording** in
   **Privacy & Security**. PARLO captures system audio through ScreenCaptureKit,
   which requires Screen Recording permission even for audio-only recording.
3. Reopen PARLO if macOS asks you to. If the permissions stay tied to an older
   build, use **Reset permissions and restart** in **Settings → Recording**.

## Test kit

If you use the test ZIP kit:

1. Extract the ZIP and open **Prepara PARLO.command**.
2. If Finder blocks it, follow the instructions in **PRIMO AVVIO MAC.html**.
3. If preparation fails, consult `Diagnostica-PARLO-Mac.zip`.

For subsequent launches, use **Avvia PARLO.command**.

## Testing status

Testing on physical Macs is still to be completed. Planned checks cover
startup, permissions, microphone and system audio recording, device
switching, local transcription, and installer signing.

The official package is planned to use a Developer ID and notarization. See
the [Tauri macOS signing guide](https://v2.tauri.app/distribute/sign/macos/)
for the steps.
