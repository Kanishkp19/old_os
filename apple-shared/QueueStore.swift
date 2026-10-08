import Foundation
import Darwin

struct QueueItem: Codable, Identifiable {
    enum State: String, Codable { case queued, connecting, uploading, verifying, done, failedRetry, failedPerm }
    let id: String, hubId: String, name: String, size: UInt64
    var clientItemId: String, stagedName: String, kind: String, backupSourceId: String?, targetDeviceId: String?, takenAt: Int64?
    var rootHash: String?, transferId: String?, fileId: String?, state: State = .queued
    var attempts = 0, nextAttemptAt: Date?, lastError: String?
}
struct QueueStore {
    let directory: URL
    init(directory: URL? = nil) throws {
        if let directory { self.directory = directory } else {
            #if os(iOS)
            guard let root = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.com.homehub") else { throw HubError.invalid("shared_storage_unavailable") }
            #else
            let root = try FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true).appendingPathComponent("HomeHub")
            #endif
            self.directory = root.appendingPathComponent("queue", isDirectory: true)
        }
        try FileManager.default.createDirectory(at: self.directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
    }
    func all() throws -> [QueueItem] {
        try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "json" }
            .map { try JSONDecoder().decode(QueueItem.self, from: Data(contentsOf: $0)) }
            .sorted { $0.id < $1.id }
    }
    func pending() throws -> [QueueItem] { try all().filter { $0.state != .done && $0.state != .failedPerm } }
    func staged(_ item: QueueItem) throws -> URL {
        guard item.stagedName == item.id + ".data" else { throw HubError.invalid("queue_invalid") }
        return directory.appendingPathComponent(item.stagedName)
    }
    func save(_ item: QueueItem) throws {
        _ = try Endpoint.path(item.id)
        let data = try JSONEncoder().encode(item)
        let url = directory.appendingPathComponent(item.id + ".json")
        try data.write(to: url, options: .atomic)
        let handle = try FileHandle(forWritingTo: url); try handle.synchronize(); try handle.close()
        try synchronizeDirectory()
        #if os(iOS)
        try FileManager.default.setAttributes([.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication], ofItemAtPath: url.path)
        #endif
    }
    @discardableResult
    func enqueue(url: URL, hubId: String, clientItemId: String? = nil, kind: String = "send", sourceId: String? = nil,
                 targetDeviceId: String? = nil, takenAt: Int64? = nil, displayName: String? = nil) throws -> QueueItem {
        guard !hubId.isEmpty else { throw HubError.invalid("connect_first") }
        if let clientItemId, let existing = try all().first(where: { $0.hubId == hubId && $0.clientItemId == clientItemId && $0.state != .failedPerm }) { return existing }
        let access = url.startAccessingSecurityScopedResource(); defer { if access { url.stopAccessingSecurityScopedResource() } }
        let values = try url.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
        guard values.isRegularFile == true, values.isSymbolicLink != true else { throw HubError.invalid("choose_file") }
        let id = UUID().uuidString
        let stage = directory.appendingPathComponent(id + ".data"), partial = directory.appendingPathComponent(id + ".partial")
        defer { try? FileManager.default.removeItem(at: partial) }
        try FileManager.default.copyItem(at: url, to: partial)
        let handle = try FileHandle(forWritingTo: partial); try handle.synchronize(); try handle.close()
        #if os(iOS)
        try FileManager.default.setAttributes([.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication], ofItemAtPath: partial.path)
        #endif
        try FileManager.default.moveItem(at: partial, to: stage)
        try synchronizeDirectory()
        let size = (try FileManager.default.attributesOfItem(atPath: stage.path)[.size] as? NSNumber)?.uint64Value
        guard let size else { throw HubError.invalid("source_unavailable") }
        let item = QueueItem(id: id, hubId: hubId, name: displayName ?? url.lastPathComponent, size: size, clientItemId: clientItemId ?? id,
            stagedName: id + ".data", kind: kind, backupSourceId: sourceId, targetDeviceId: targetDeviceId, takenAt: takenAt)
        do { try save(item) } catch { try? FileManager.default.removeItem(at: stage); throw error }
        return item
    }
    private func synchronizeDirectory() throws {
        let descriptor = open(directory.path, O_RDONLY)
        guard descriptor >= 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        defer { close(descriptor) }
        guard fsync(descriptor) == 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
    }
    func retry(_ item: QueueItem) throws {
        var updated = item; updated.state = .queued; updated.nextAttemptAt = nil; updated.lastError = nil
        try save(updated)
    }
}
