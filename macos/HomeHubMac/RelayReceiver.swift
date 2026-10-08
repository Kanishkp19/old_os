import Foundation
import Darwin

actor RelayReceiver {
    struct Delivery: Codable, Identifiable {
        let id: String, file_id: String, name: String, size: UInt64, hash: String, source_device_id: String, created_at: Int64
        var status: String
    }
    struct Inbox: Decodable { let items: [Delivery] }
    private let root: URL
    private var running = false
    init() throws {
        root = try FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
            .appendingPathComponent("HomeHub/Received", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
    }
    func receive(_ trust: HubTrust) async throws -> Int {
        guard !running else { return 0 }; running = true; defer { running = false }
        let api = try HubClient(trust), inbox = try await api.decode(Inbox.self, "/v1/relay/inbox")
        for var delivery in inbox.items where delivery.status == "pending" {
            try Task.checkCancellation()
            let id = try Endpoint.path(delivery.id), fileId = try Endpoint.path(delivery.file_id)
            let hubDirectory = root.appendingPathComponent(try Endpoint.path(trust.hubId), isDirectory: true)
            try FileManager.default.createDirectory(at: hubDirectory, withIntermediateDirectories: true)
            let deliveryDirectory = hubDirectory.appendingPathComponent(id, isDirectory: true)
            try FileManager.default.createDirectory(at: deliveryDirectory, withIntermediateDirectories: true)
            // Keep useful extensions while jailing every delivery in its own
            // locally chosen directory. Remote paths never become paths.
            let safe = String(delivery.name.unicodeScalars.filter { !CharacterSet.controlCharacters.contains($0) }
                .map { $0 == "/" || $0 == ":" || $0 == "\\" ? "_" : String($0) }.joined().prefix(180))
            let visible = safe.trimmingCharacters(in: CharacterSet.whitespacesAndNewlines.union(CharacterSet(charactersIn: ".")))
            let name = visible.isEmpty ? "Received file" : visible.lowercased() == "receipt.json" ? "received-receipt.json" : visible
            let contentDirectory = deliveryDirectory.appendingPathComponent("content", isDirectory: true)
            try FileManager.default.createDirectory(at: contentDirectory, withIntermediateDirectories: true)
            let fileURL = contentDirectory.appendingPathComponent(name)
            let receipt = deliveryDirectory.appendingPathComponent("receipt.json")
            try saveReceipt(delivery, to: receipt)
            if !FileManager.default.fileExists(atPath: fileURL.path) {
                let file = try await api.decode(HubFile.self, "/v1/files/\(fileId)")
                guard file.hash == delivery.hash, file.size == delivery.size else { throw HubError.invalid("verification_failed") }
                try await api.download(file, to: fileURL)
            }
            guard try Blake3.fileHex(fileURL) == delivery.hash.lowercased(),
                (try FileManager.default.attributesOfItem(atPath: fileURL.path)[.size] as? NSNumber)?.uint64Value == delivery.size else { throw HubError.invalid("verification_failed") }
            _ = try await api.request("/v1/relay/\(id)/delivered", method: "POST", json: ["hash": delivery.hash])
            delivery.status = "delivered"; try saveReceipt(delivery, to: receipt)
        }
        return inbox.items.filter { $0.status == "pending" }.count
    }
    private func saveReceipt(_ delivery: Delivery, to url: URL) throws {
        try JSONEncoder().encode(delivery).write(to: url, options: .atomic)
        let handle = try FileHandle(forWritingTo: url); try handle.synchronize(); try handle.close()
        let descriptor = open(url.deletingLastPathComponent().path, O_RDONLY)
        guard descriptor >= 0 else { throw HubError.invalid("source_unavailable") }
        defer { close(descriptor) }
        guard fsync(descriptor) == 0 else { throw HubError.invalid("source_unavailable") }
    }
    static func reveal() throws {
        let url = try FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: false)
            .appendingPathComponent("HomeHub/Received")
        #if os(macOS)
        NSWorkspace.shared.open(url)
        #endif
    }
}
#if os(macOS)
import AppKit
#endif
