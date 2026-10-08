import Foundation

actor UploadEngine {
    private let store: QueueStore
    private var running = false
    init(store: QueueStore) { self.store = store }
    func pump(_ initialTrust: HubTrust) async throws {
        guard !running else { return }; running = true; defer { running = false }
        let initial = try HubClient(initialTrust)
        let trust = try await initial.renewedIfDue()
        let api = try HubClient(trust)
        for var item in try store.pending() where item.hubId == trust.hubId {
            try Task.checkCancellation()
            if let next = item.nextAttemptAt, next > Date() { continue }
            do {
                item.state = .connecting; try store.save(item)
                let source = try store.staged(item)
                item.rootHash = try Blake3.fileHex(source)
                try store.save(item)
                guard let hash = item.rootHash else { throw HubError.invalid("queue_invalid") }
                var json: [String: Any] = ["name": item.name, "size": item.size, "kind": item.kind,
                    "chunk_size": 4 * 1024 * 1024, "client_item_id": item.clientItemId, "root_hash": hash]
                if let sourceId = item.backupSourceId { json["backup_source_id"] = sourceId }
                if let target = item.targetDeviceId { json["target_device_id"] = target }
                if let taken = item.takenAt { json["taken_at"] = taken }
                let created = try await api.decode(TransferCreated.self, "/v1/transfers", method: "POST", json: json)
                guard (1...8 * 1024 * 1024).contains(created.chunk_size), created.chunk_count == max(1, Int((item.size + UInt64(created.chunk_size) - 1) / UInt64(created.chunk_size))) else { throw HubError.invalid("queue_invalid") }
                item.transferId = created.transfer_id; try store.save(item)
                if created.already_exists {
                    guard let id = created.existing_file_id else { throw HubError.invalid("verification_failed") }
                    let file = try await api.decode(HubFile.self, "/v1/files/\(try Endpoint.path(id))")
                    guard file.hash.lowercased() == hash, file.size == item.size else { throw HubError.invalid("verification_failed") }
                    item.fileId = id
                } else {
                    let transfer = try Endpoint.path(created.transfer_id)
                    let status = try await api.decode(TransferStatus.self, "/v1/transfers/\(transfer)")
                    let handle = try FileHandle(forReadingFrom: source); defer { try? handle.close() }
                    item.state = .uploading; try store.save(item)
                    for index in 0..<created.chunk_count where !status.have.contains(index) {
                        try Task.checkCancellation()
                        try handle.seek(toOffset: UInt64(index) * UInt64(created.chunk_size))
                        let bytes = try handle.read(upToCount: created.chunk_size) ?? Data()
                        let expected = min(UInt64(created.chunk_size), item.size - UInt64(index) * UInt64(created.chunk_size))
                        guard UInt64(bytes.count) == expected else { throw HubError.invalid("source_changed") }
                        _ = try await api.request("/v1/transfers/\(transfer)/chunks/\(index)", method: "PUT", data: bytes, hash: Blake3.hex(bytes))
                    }
                    item.state = .verifying; try store.save(item)
                    let completed = try await api.decode(TransferComplete.self, "/v1/transfers/\(transfer)/complete", method: "POST", json: ["root_hash": hash])
                    guard completed.verified, completed.hash.lowercased() == hash, completed.size == item.size else { throw HubError.invalid("verification_failed") }
                    item.fileId = completed.file_id
                }
                item.state = .done; item.lastError = nil; item.nextAttemptAt = nil; try store.save(item)
                // Only this private queue copy is removed; source originals stay.
                try? FileManager.default.removeItem(at: source)
            } catch is CancellationError {
                item.state = .queued; try store.save(item); throw CancellationError()
            } catch {
                item.attempts += 1
                let transient = (error as? HubError)?.retryable ?? true
                item.state = transient ? .failedRetry : .failedPerm
                if let hubError = error as? HubError, case .http(410, _, _) = hubError { item.transferId = nil; item.clientItemId = UUID().uuidString; item.state = .failedRetry }
                item.nextAttemptAt = Date().addingTimeInterval(min(3600, 15 * pow(2, Double(min(item.attempts, 8)))))
                item.lastError = error.localizedDescription; try store.save(item)
            }
        }
    }
}
