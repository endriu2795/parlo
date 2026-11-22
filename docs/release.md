# Release Guide (Maintainers)

Steps for publishing official releases on [endriu2795/parlo](https://github.com/endriu2795/parlo). For custom fork installers, see [CONTRIBUTING.md](../CONTRIBUTING.md#installers-and-forks).

## 1. Prepare the version

1. Work from a clean clone of `main` with a GitHub account authorized to publish releases (`gh auth login`).
2. Keep the Tauri updater signing key at `.tools/updater/parlo.key` or set `TAURI_SIGNING_PRIVATE_KEY` in your environment (the matching public key is in `scripts/official-updater.json`).
3. Bump the version, commit, and push to `main`:

```sh
npm run release:version -- <VERSION>
```

## 2. Build & upload Windows x64

Ensure the [Windows prerequisites](../CONTRIBUTING.md#windows-x64) and Vulkan runtime are ready, then run:

```sh
npm run release
```

> Until an Authenticode certificate is configured, mention in the release notes that the Windows installer is unsigned and SmartScreen may warn or block it depending on local policy.

## 3. Build & upload macOS

Once Developer ID signing and notarization are configured, build from the **same commit** as Windows:

```sh
bash scripts/pubblica-mac.command
```

## 4. Publish

The release scripts upload packages to [GitHub Releases](https://github.com/endriu2795/parlo/releases) and update the Tauri updater manifest. Platform builds can be attached to the same release tag separately.

Before publishing binaries, verify all items in [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md#redistributing-installers).
