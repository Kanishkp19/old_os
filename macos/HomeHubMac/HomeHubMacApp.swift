// HomeHubMacApp.swift — macOS menu-bar client (M1, UI_UX_DESIGN §5).
//
// Lives in the menu bar; the main "window" is a popover. Drag files onto the
// icon or popover to queue them; pairing is a QR shown by the Hub dashboard
// scanned with the phone — on the Mac itself you paste/type the payload or
// use the 6-digit code.
import SwiftUI

@main
struct HomeHubMacApp: App {
    @StateObject private var model = AppModel()

    var body: some Scene {
        MenuBarExtra("Home Hub", systemImage: model.isReachable ? "house.fill" : "house") {
            PopoverView()
                .environmentObject(model)
        }
        .menuBarExtraStyle(.window)
    }
}

final class AppModel: ObservableObject {
    @Published var trust: HubTrust?
    @Published var isReachable = false
    @Published var queueItems: [MacQueueItem] = []

    let queue = MacQueue()
    let discovery = BonjourDiscovery()

    init() {
        trust = MacTrustStore.load()
        queueItems = queue.all()
        discovery.onHubFound = { [weak self] host, port in
            guard let self, var t = self.trust else { return }
            t.lastAddr = "\(host):\(port)"
            MacTrustStore.save(t)
            DispatchQueue.main.async {
                self.trust = t
                self.isReachable = true
            }
        }
        discovery.start()
    }

    func enqueue(urls: [URL]) {
        for url in urls {
            queue.enqueue(url: url)
        }
        queueItems = queue.all()
        UploadScheduler.shared.kick()
    }
}
