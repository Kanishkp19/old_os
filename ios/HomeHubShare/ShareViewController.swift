// ShareViewController.swift — share extension: enqueue into the App Group
// queue and return immediately (FR-3.4 send-later; never blocks the sheet).
import UIKit
import UniformTypeIdentifiers

final class ShareViewController: UIViewController {
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        let items = extensionContext?.inputItems as? [NSExtensionItem] ?? []
        Task {
            for item in items {
                for provider in item.attachments ?? [] {
                    if let url = try? await provider.loadItem(forTypeIdentifier: UTType.data.identifier) as? URL {
                        var queue = UploadQueue()
                        queue.enqueue(QueueItem(
                            id: UUID().uuidString,
                            hubId: TrustStore.load()?.hubId ?? "",
                            sourceBookmark: (try? url.bookmarkData()) ?? Data(),
                            clientItemId: url.absoluteString, // stable per source
                            name: url.lastPathComponent,
                            size: (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize).map(Int64.init) ?? 0,
                            mime: nil,
                            kind: "send",
                            rootHash: nil,
                            transferId: nil,
                            state: .queued,
                            attempts: 0,
                            nextAttemptAt: nil,
                            lastError: nil
                        ))
                    }
                }
            }
            self.extensionContext?.completeRequest(returningItems: nil)
        }
    }
}
