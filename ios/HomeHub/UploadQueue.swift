// UploadQueue.swift — send-later queue shared with the share extension via
// the App Group container (same on-disk shape as BACKEND_SCHEMA §9).
import Foundation

struct QueueItem: Codable, Identifiable {
    var id: String
    var hubId: String
    var sourceBookmark: Data     // security-scoped bookmark for the source URL
    var clientItemId: String
    var name: String
    var size: Int64
    var mime: String?
    var kind: String             // send | backup
    var rootHash: String?        // computed lazily
    var transferId: String?
    var state: State
    var attempts: Int
    var nextAttemptAt: Date?
    var lastError: String?

    enum State: String, Codable {
        case queued, connecting, uploading, verifying, done
        case failedRetry = "failed_retry"
        case failedPerm = "failed_perm"
    }
}

final class UploadQueue {
    private let storeURL: URL = {
        let dir = FileManager.default
            .containerURL(forSecurityApplicationGroupIdentifier: "group.com.homehub")!
            .appendingPathComponent("queue", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("queue.json")
    }()

    func pending() -> [QueueItem] {
        guard let data = try? Data(contentsOf: storeURL),
              let items = try? JSONDecoder().decode([QueueItem].self, from: data) else { return [] }
        return items.filter { $0.state != .done && $0.state != .failedPerm }
    }

    /// Actual uploading uses a background URLSession with the pinned-CA
    /// delegate; the chunk protocol is identical to hh-tools fake_client.rs.
    func enqueue(_ item: QueueItem) {
        var items = pending()
        items.append(item)
        if let data = try? JSONEncoder().encode(items) {
            try? data.write(to: storeURL, options: .atomic)
        }
    }
}
