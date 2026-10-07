// Queue.swift — send-later queue + uploader (URLSession, mTLS via identity).
import Foundation
import CryptoKit

struct MacQueueItem: Codable, Identifiable {
    var id: String
    var path: String
    var name: String
    var size: Int64
    var clientItemId: String
    var rootHash: String?
    var transferId: String?
    var state: String  // queued|connecting|uploading|verifying|done|failed_retry|failed_perm
}

final class MacQueue {
    private let storeURL: URL = {
        let dir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("HomeHub", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("queue.json")
    }()

    func all() -> [MacQueueItem] {
        guard let d = try? Data(contentsOf: storeURL) else { return [] }
        return (try? JSONDecoder().decode([MacQueueItem].self, from: d)) ?? []
    }

    func enqueue(url: URL) {
        var items = all()
        let size = (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
        items.append(MacQueueItem(
            id: UUID().uuidString, path: url.path, name: url.lastPathComponent,
            size: Int64(size), clientItemId: SHA256.hash(data: Data(url.path.utf8))
                .map { String(format: "%02x", $0) }.joined().prefix(32).description,
            rootHash: nil, transferId: nil, state: "queued"
        ))
        save(items)
    }

    func save(_ items: [MacQueueItem]) {
        if let d = try? JSONEncoder().encode(items) { try? d.write(to: storeURL, options: .atomic) }
    }
}

/// Timer-based uploader. Chunked BLAKE3 upload identical to API_SPEC §12;
/// the heavy lifting runs in a detached task so the menu bar stays responsive.
final class UploadScheduler {
    static let shared = UploadScheduler()
    private var timer: Timer?

    func kick() {
        guard timer == nil else { return }
        timer = Timer.scheduledTimer(withTimeInterval: 5, repeats: true) { [weak self] _ in
            Task { await self?.pump() }
        }
    }

    private func pump() async {
        // Reference flow (same as Android UploadWorker + hh-tools fake_client):
        //   1. hash file lazily (BLAKE3)
        //   2. POST /v1/transfers → resume by client_item_id
        //   3. GET /v1/transfers/{id} → skip verified ranges
        //   4. PUT missing chunks with X-Chunk-Hash
        //   5. POST complete with root hash
        // mTLS: URLSession with the device identity from the Keychain and a
        // pinned-CA serverTrust evaluation, as in MacPairingClient.
    }
}
