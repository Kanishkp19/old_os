import UIKit
import UniformTypeIdentifiers

final class ShareViewController: UIViewController {
    private var started = false
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        guard !started else { return }; started = true
        let spinner = UIActivityIndicatorView(style: .large); spinner.center = view.center; view.addSubview(spinner); spinner.startAnimating()
        Task {
            do {
                let store = try QueueStore()
                // Extension intentionally has no Keychain identity entitlement.
                // The selected hub ID is non-secret App Group configuration.
                guard let hubId = UserDefaults(suiteName: "group.com.homehub")?.string(forKey: "hubId"), !hubId.isEmpty else { throw HubError.invalid("connect_first") }
                var count = 0
                for item in extensionContext?.inputItems as? [NSExtensionItem] ?? [] {
                    for provider in item.attachments ?? [] {
                        guard let type = provider.registeredTypeIdentifiers.first(where: { UTType($0)?.conforms(to: .data) == true }) else { continue }
                        // Provider URLs can disappear as soon as this completion
                        // returns. Stage DURING the callback, then persist before
                        // dismissing the sheet; bookmarks are not sufficient.
                        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
                            provider.loadFileRepresentation(forTypeIdentifier: type) { url, error in
                                guard let url else { continuation.resume(throwing: error ?? HubError.invalid("source_unavailable")); return }
                                do { try store.enqueue(url: url, hubId: hubId); continuation.resume() }
                                catch { continuation.resume(throwing: error) }
                            }
                        }
                        count += 1
                    }
                }
                guard count > 0 else { throw HubError.invalid("choose_file") }
                extensionContext?.completeRequest(returningItems: nil)
            } catch {
                spinner.stopAnimating()
                let alert = UIAlertController(title: NSLocalizedString("send_failed", comment: ""), message: error.localizedDescription, preferredStyle: .alert)
                alert.addAction(UIAlertAction(title: NSLocalizedString("close", comment: ""), style: .default) { _ in self.extensionContext?.cancelRequest(withError: error) })
                present(alert, animated: true)
            }
        }
    }
}
