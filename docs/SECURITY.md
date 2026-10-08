# Home Hub — Security Model

Home Hub stores people's most personal data (photos, documents, family files). Security is a core feature, not an add-on.

---

## 1. Security goals

1. Only paired, non-revoked devices can access the Hub.
2. All LAN traffic is encrypted and mutually authenticated.
3. The Hub is **not reachable from the internet by default**.
4. A lost phone can be cut off instantly.
5. Corruption and tampering are detectable (hash verification).
6. Compromise of one device does not expose Hub admin or the CA key.

## 2. Assets

| Asset | Sensitivity |
|---|---|
| User files (photos, docs, IDs) | Critical |
| Hub CA private key | Critical |
| Device private keys (client keystore) | High |
| `hub.db` (metadata, GPS, filenames) | High |
| Pairing tokens | High (short-lived) |
| Logs / audit log | Medium |

## 3. Threat model

| # | Threat | Actor | Mitigation |
|---|---|---|---|
| T1 | Eavesdropping on Wi-Fi | Neighbor, café attacker | TLS 1.3 everywhere |
| T2 | Rogue device joins | Guest on Wi-Fi | mTLS; pairing needs on-screen QR/code from Hub |
| T3 | Rogue Hub impersonation | Evil twin on LAN | Clients pin CA fingerprint from QR; TOFU only via QR |
| T4 | QR/token theft | Shoulder surfer, screenshot | 5-min TTL, single use, 5-attempt lockout, optional on-Hub confirm |
| T5 | Lost/stolen phone | Thief | Remove Device → immediate revoke; certs short-lived (1 yr) + revocation checked per handshake |
| T6 | Malware on laptop | Local malware | Least-privilege service account; data dir ACLs; recommend BitLocker; signed updates |
| T7 | Remote control abuse | Paired-but-compromised device | `remote` scope separate, off by default, tray indicator, audit log, rate limits |
| T8 | Path traversal / file API abuse | Malicious client | Server-side path canonicalization, root jail, never trust client paths |
| T9 | Resource exhaustion | Malicious/buggy client | Quotas, concurrency caps, request limits, idle TTLs |
| T10 | Supply chain / malicious update | Attacker | Signed manifests (pinned Ed25519), reproducible builds goal, dependency auditing (`cargo-deny`, `cargo-audit`) |
| T11 | Disk theft | Physical | Full-disk encryption (BitLocker) recommended; app-level at-rest encryption future |
| T12 | Silent data corruption / bit rot | Aging drives | BLAKE3 verification on write + scrub + second copy |
| T13 | Internet exposure via router port-forward / UPnP | Misconfiguration | Hub never uses UPnP; binds only to LAN; warns if reachable on public IP (self-check) |
| T14 | Downgrade/TLS misconfig | Active attacker | TLS 1.3 only, no fallback |

Out of scope (v1): nation-state attackers, compromised Windows kernel, malicious admin on the Hub PC (they own the data anyway).

## 4. Cryptography

| Purpose | Choice |
|---|---|
| Channel | TLS 1.3, rustls; ciphers AES-256-GCM / ChaCha20-Poly1305 |
| Hub CA | ECDSA P-256 (or Ed25519 where supported by client stacks); generated on first run |
| Device keys | Generated on device in secure hardware (Android Keystore/StrongBox, Apple Secure Enclave/Keychain), non-exportable where possible |
| Device certs | 1-year validity, auto-renew at <30 days, CN=device_id, SAN URI `homehub:device:<id>` |
| Integrity | BLAKE3 (chunk + tree root) |
| Token hashing | SHA-256 of 128-bit random token at rest |
| Update signing | Ed25519, owner-configured public key stored in protected Hub settings; installer SHA-256 in the signed manifest |
| Randomness | OS CSPRNG only |

### Key storage (Windows)
- CA private key encrypted with **DPAPI (machine scope)** and ACL'd to the service account; never written to `hub.db` plaintext.
- Optional: user passphrase-wrapped backup of CA key for Hub migration (exported only on explicit action).

## 5. Pairing security

- Window opens only when user clicks **Pair a device** (tray/dashboard). Closes after success or 5 minutes.
- Token 128-bit random; stored hashed; burned on use.
- CA fingerprint in QR defeats LAN MITM (client verifies chain before sending token).
- Manual code mode (6-digit, 2-min TTL) **requires** explicit on-Hub confirmation prompt because digits are low-entropy; max 3 attempts.
- All pairing events recorded in `audit_log`.

## 6. Authorization

| Scope | Grants |
|---|---|
| `files` | Browse, download, rename, trash |
| `transfer` | Upload/download sessions |
| `photos` | Gallery, backup, duplicates |
| `remote` | Input, power, media (off by default; user opt-in per device) |
| `admin` | Device management, settings, second-copy (typically only the first/owner device) |

Dashboard (loopback) uses Windows user session; it is the only way to grant `admin` to additional devices (or via first device owner confirm).

The installer records the chosen owner account SID in `%ProgramData%\HomeHub\authorized-owner.sid` under SYSTEM/Administrators ACLs. The service gives only that SID read access to the local dashboard token. The Tauri trusted Home Hub view and external browser tabs use separate WebView2 profiles and capabilities. Website content cannot call privileged commands or receive the Hub token. The desktop app stores restorable tab addresses only in its per-user database; restoration requires a user action and does not restore website permissions or download grants. Clearing website data removes that saved session and browser-history rows. A paired certificate is rechecked against device status, scopes and serial on requests and active connections. The session helper accepts only the installed service identity on a bounded local pipe; screen sessions have an exact owner and expire on disconnect or missed heartbeats.

## 7. Network hardening

- Bind API to LAN interfaces only; dashboard to 127.0.0.1.
- Windows Firewall inbound rules scoped to **Private** profile + local subnet.
- No UPnP/NAT-PMP; no port-forward guidance.
- Public-IP self-check: if Hub detects inbound reachability from non-RFC1918 source (attempted connections), log + alert + refuse non-LAN source IPs (explicit source-IP allowlist: RFC1918, link-local, ULA).
- Remote-from-outside-home is **future opt-in** via user-owned overlay (e.g., WireGuard); never default.

## 8. Data protection

- Files written as-is (no proprietary container) so users can always read their data with any tool.
- At rest: recommend BitLocker in setup; surface status (read via WMI) in Hardware/Security panel.
- Temp `.part` files inside the library volume (same-volume atomic rename), cleaned on startup.
- Trash retention then secure delete is best-effort (SSD limitations noted).
- Logs must never contain tokens, file contents, or full file paths of user media by default (log IDs and sizes).
- GPS EXIF stored but not displayed unless user enables; never leaves Hub.

## 9. Privacy

- No telemetry by default. Optional diagnostics are user-triggered export bundles, scrubbed.
- No accounts, no cloud identity, no analytics SDKs in mobile apps (verify with dependency audit).
- Update checks (if enabled) send only version + platform.

## 10. Secure development checklist (per feature)

- [ ] All client input validated; sizes/length bounded
- [ ] Path handling via canonical join + root-prefix check; reject `..`, reserved Windows names (`CON`, `NUL`…), ADS (`:`), trailing dots/spaces
- [ ] AuthN/AuthZ enforced on every route (default-deny middleware)
- [ ] No secrets in logs
- [ ] Errors don't leak internal paths
- [ ] Rate limits and concurrency caps applied
- [ ] Fuzz tests for parsers (chunk headers, QR payload, WebSocket frames)
- [ ] `cargo audit` / `cargo deny` clean
- [ ] Threat-model delta reviewed

## 11. Incident handling

| Event | Response |
|---|---|
| Device lost | Remove Device (instant); optional "rotate CA" for severe cases (re-pairs all devices) |
| Suspected Hub compromise | Dashboard: Pause sharing → revoke all → review `audit_log` → re-pair |
| Integrity mismatch found | Alert, quarantine file, attempt repair from second copy, notify user |
| Vulnerability report | `SECURITY.md` contact (security@<domain>), 90-day coordinated disclosure; signed patch release |

## 12. Security testing plan
- Unit tests for cert validation, revocation, token lifecycle
- Integration: unpaired/revoked/expired cert connections must fail
- Pentest-style tests: path traversal corpus, oversize bodies, slowloris, chunk replay, token brute force
- Third-party review before Phase 1 pilot invitation and before Phase 4 OS release

## 13. Disclosure
Report privately to `security@<domain>` (placeholder). Do not open public issues for vulnerabilities.
