# Windows validation backlog

- UNVERIFIED: QR-pinned pairing against a rogue Hub on a Windows LAN. Scan a current QR, route its advertised address to a different Hub, and confirm Android and `hh-tools` reject it before token submission; then pair with the intended Hub.
- UNVERIFIED: Revoke a device during API, SSE, remote control and screen sessions on Windows; confirm requests fail immediately and only that device's live sessions close.
- UNVERIFIED: Renew a near-expiry Android device certificate against Windows, lose the renewal response, retry with the old identity, then verify activation revokes only the old certificate.
- UNVERIFIED: Interrupt Windows transfer finalization before and after the write-through move; restart and confirm exactly one verified library file and a committed database row.
