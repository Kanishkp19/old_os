# Home Hub macOS companion

Open `HomeHubMac.xcodeproj` in Xcode 16+ on macOS 14+. It includes an application target, XCTest target and shared scheme. Select a signing team in Xcode; no credentials or team IDs are committed. Regenerate source membership/projects with `python3 apple-shared/generate-projects.py` from the repository root. The generated project is supplied, so XcodeGen is not required.

Implemented workflows:
- Menu-bar pairing by pasted Hub QR payload; CA pin before token, permanent Keychain key/CSR, pinned TLS 1.3 client identity, certificate renewal and Bonjour address rediscovery.
- Drag/drop or file picker → private durable staging → send-later queue → chunk resume/BLAKE3 verification → retained transfer receipts and explicit retry. A network change or 30-second resident retry loop resumes waiting work.
- Native library search, cursor pagination and save-copy downloads. Saving never overwrites an existing user file and checks fresh whole-file hash/size before install.
- Basic relay: send a library file to a recipient device ID shown by the Hub. Receive the Hub's targeted inbox automatically, persist receipt before download, verify the private destination before acknowledging and retain it under app support `HomeHub/Received/<hub-id>/<delivery-id>/content/<safe-original-name>`. **Received files** opens that folder; each delivery folder also includes a JSON receipt retaining the original name. App Support is inside the application sandbox container in signed builds.
- Screen viewing uses one bundled fixed HTML page in a nonpersistent WKWebView. Navigation and native messages are restricted to that main-frame file. The native bridge exposes only create/heartbeat/stop of the screen session, sends signaling using Keychain credentials outside the page, validates LAN-only host ICE candidates and rejects synthetic `loopback` transport. No loopback listener, Hub credential or general request bridge exists. Session heartbeat is 15 seconds; close/disconnect requests DELETE. Requires a running Windows helper, device remote permission and a gathered real answer from `/v1/screen/view`.

All user-visible client copy is EN/HI localized. macOS capabilities are limited to outbound networking and user-selected read/write files; no resident Electron process is added.

Validation is deferred: project generation succeeded but no Xcode build/tests/checks have been run. Later gate must cover both ARM64/Intel Macs, sandbox Keychain identities and renewal, Bonjour IP changes, 2 GiB/chunk interruption, power-loss queue recovery, target-only relay/duplicate acknowledgment, and WKWebView receive/close/lease behavior against the physical Windows helper. See `../apple-shared/README.md` for trust/persistence details and protocol limitations.
