import Foundation
import Photos

actor PhotoBackup {
    static func discover(trust: HubTrust, store: QueueStore, engine: UploadEngine) async throws {
        let status = await PHPhotoLibrary.requestAuthorization(for: .readWrite)
        guard status == .authorized || status == .limited else { throw HubError.invalid("photos_permission_required") }
        let paired = try await HubClient(trust).renewedIfDue()
        let api = try HubClient(paired)
        let source = try await api.decode(BackupSource.self, "/v1/backup/sources", method: "POST", json: ["kind": "camera_roll", "label": "iPhone Photos"])
        let known = Set(try store.all().filter { $0.hubId == paired.hubId && $0.state != .failedPerm }.map(\.clientItemId))
        let assets = PHAsset.fetchAssets(with: nil)
        var unavailable = false, staged = 0
        for index in 0..<assets.count {
            try Task.checkCancellation()
            let asset = assets.object(at: index)
            let version = Int64((asset.modificationDate ?? asset.creationDate ?? .distantPast).timeIntervalSince1970 * 1000)
            for (resourceIndex, resource) in PHAssetResource.assetResources(for: asset).enumerated() {
                // Every original resource is copied, including Live Photo video
                // and adjustment sidecars. Never request Photos deletion.
                let clientId = "photos:\(asset.localIdentifier):\(version):\(resource.type.rawValue):\(resourceIndex)"
                if known.contains(clientId) { continue }
                let temp = store.directory.appendingPathComponent(UUID().uuidString + ".export")
                defer { try? FileManager.default.removeItem(at: temp) }
                let options = PHAssetResourceRequestOptions(); options.isNetworkAccessAllowed = false
                do {
                    try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
                        PHAssetResourceManager.default().writeData(for: resource, toFile: temp, options: options) { error in
                            if let error { continuation.resume(throwing: error) } else { continuation.resume() }
                        }
                    }
                } catch { unavailable = true; continue }
                try Task.checkCancellation()
                let hash = try Blake3.fileHex(temp)
                guard let size = try FileManager.default.attributesOfItem(atPath: temp.path)[.size] as? NSNumber else { throw HubError.invalid("source_unavailable") }
                let taken = asset.creationDate.map { Int64($0.timeIntervalSince1970 * 1000) }
                var diffItem: [String: Any] = ["client_item_id": clientId, "size": size.uint64Value, "hash": hash]
                if let taken { diffItem["taken_at"] = taken }
                _ = try await api.decode(BackupDiff.self, "/v1/backup/sources/\(try Endpoint.path(source.id))/diff", method: "POST", json: [diffItem])
                // Even deduplicated resources use transfer create to validate
                // the file receipt and persist a durable local history row.
                try store.enqueue(url: temp, hubId: paired.hubId, clientItemId: clientId, kind: "backup", sourceId: source.id,
                    takenAt: taken, displayName: resource.originalFilename)
                staged += 1
                // Send in small batches so a whole library does not require a
                // second full local copy before its first item reaches Home.
                if staged % 8 == 0 { try await engine.pump(TrustStore.load() ?? paired) }
            }
        }
        try await engine.pump(TrustStore.load() ?? paired)
        if unavailable { throw HubError.invalid("photos_not_local") }
    }
}
