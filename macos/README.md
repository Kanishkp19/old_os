# Home Hub for macOS (M1 menu-bar client)

A SwiftUI menu-bar app: pair once, then drag files onto the menu-bar icon.
Runs on macOS 14+, no admin rights needed (the Hub is a Windows machine;
this app is only a client).

## Building

Open this folder in Xcode 16+ and create an App target named `HomeHubMac`
(macOS 14, SwiftUI lifecycle), adding every file under `HomeHubMac/`.
No third-party dependencies; `Network.framework` + `CryptoKit` only.

## What works vs what's staged

| Piece | Status |
|---|---|
| Bonjour discovery (`_homehub._tcp`) | Implemented (`Discovery.swift`) |
| QR payload parse + fingerprint pinning | Implemented (`Pairing.swift`) |
| PKCS#10 CSR in Swift | Stub — port the DER builder from `android/.../PairingClient.kt` or adopt swift-certificates |
| Chunked upload pump | Skeleton — reference flow documented in `Queue.swift`, mirroring `hh-tools fake_client` |
| Drag-drop queue + menu-bar UI | Implemented (`PopoverView.swift`) |

The Android app is the most complete client; use it as the behavior reference.
