import SwiftUI
import Network
import AppKit

@main
@MainActor
struct HomeHubMacApp: App {
    @StateObject private var model = AppModel()
    var body: some Scene {
        MenuBarExtra("Home Hub", systemImage: model.isReachable ? "house.fill" : "house") { PopoverView().environmentObject(model) }.menuBarExtraStyle(.window)
        Window("Home Hub", id: "library") { LibraryView().environmentObject(model).frame(minWidth: 650, minHeight: 450) }
        Window("Screen", id: "screen") {
            if let trust = model.trust { ScreenView(trust: trust).environmentObject(model).frame(minWidth: 800, minHeight: 500) }
        }
    }
}
@MainActor final class AppModel: ObservableObject {
    @Published var trust = TrustStore.load()
    @Published var isReachable = false
    @Published var items: [QueueItem] = []
    @Published var files: [HubFile] = []
    @Published var error: String?
    @Published var query = ""
    @Published var cursor: String?
    @Published var inboxCount = 0
    @Published var busy = false
    private var store: QueueStore?, engine: UploadEngine?, relay: RelayReceiver?
    private let discovery = BonjourDiscovery(), path = NWPathMonitor()
    private var loop: Task<Void, Never>?
    init() {
        do { let store = try QueueStore(); self.store = store; engine = UploadEngine(store: store); relay = try RelayReceiver(); refresh() }
        catch { self.error = error.localizedDescription }
        discovery.onHubFound = { [weak self] id, host, port, fp in
            Task { @MainActor in
                guard let self, var trust = self.trust, trust.hubId == id, trust.caFingerprint.hasPrefix(fp), !fp.isEmpty,
                      Endpoint.parse("\(host):\(port)", port: 47800) != nil else { return }
                trust.lastAddr = "\(host):\(port)"
                do { _ = try await HubClient(trust).request("/v1/info"); guard var current = TrustStore.load(), current.hubId == trust.hubId else { return }; current.lastAddr = trust.lastAddr; try TrustStore.save(current); self.trust = current; self.isReachable = true }
                catch { self.isReachable = false }
            }
        }
        discovery.start()
        path.pathUpdateHandler = { [weak self] _ in Task { @MainActor in await self?.send() } }; path.start(queue: DispatchQueue(label: "homehub.network"))
        loop = Task { [weak self] in
            while !Task.isCancelled {
                await self?.send()
                do { try await Task.sleep(for: .seconds(30)) } catch { return }
            }
        }
    }
    func refresh() { do { items = try store?.all() ?? [] } catch { self.error = error.localizedDescription } }
    func pair(_ raw: String) async {
        guard let payload = PairPayload(raw: raw) else { error = NSLocalizedString("invalid_code", comment: ""); return }
        busy = true; defer { busy = false }
        do { trust = try await PairingClient().pair(payload, deviceName: Host.current().localizedName ?? "Mac", platform: "macos"); error = nil; await send() }
        catch { self.error = error.localizedDescription }
    }
    func enqueue(urls: [URL], target: String? = nil) {
        guard let store, let trust else { error = NSLocalizedString("connect_first", comment: ""); return }
        Task {
            do {
                try await Task.detached { for url in urls { try store.enqueue(url: url, hubId: trust.hubId, targetDeviceId: target) } }.value
                refresh(); await send()
            } catch { self.error = error.localizedDescription; refresh() }
        }
    }
    func send() async {
        guard let trust, let engine else { return }
        do {
            let api = try HubClient(trust); _ = try await api.request("/v1/info"); isReachable = true
            try await engine.pump(trust); self.trust = TrustStore.load(); refresh()
            if let current = self.trust { inboxCount = try await relay?.receive(current) ?? 0 }
        } catch is CancellationError {} catch { isReachable = false; self.error = error.localizedDescription }
    }
    func browse(more: Bool = false) async {
        guard let trust else { return }
        do {
            var parts = URLComponents(); parts.queryItems = [URLQueryItem(name: "q", value: query), URLQueryItem(name: "limit", value: "100")]
            if more, let cursor { parts.queryItems?.append(URLQueryItem(name: "cursor", value: cursor)) }
            let page = try await HubClient(trust).decode(FilePage.self, "/v1/files" + (parts.string ?? ""))
            if more { files.append(contentsOf: page.items) } else { files = page.items }; cursor = page.next_cursor
        } catch { self.error = error.localizedDescription }
    }
    func download(_ file: HubFile) {
        let panel = NSSavePanel(); panel.nameFieldStringValue = file.name; panel.canCreateDirectories = true
        guard panel.runModal() == .OK, let url = panel.url, let trust else { return }
        Task {
            let access = url.startAccessingSecurityScopedResource(); defer { if access { url.stopAccessingSecurityScopedResource() } }
            do { try await HubClient(trust).download(file, to: url) } catch { self.error = error.localizedDescription }
        }
    }
    func relayFile(_ file: HubFile, target: String) async {
        guard let trust else { return }
        do { _ = try await HubClient(trust).request("/v1/files/\(try Endpoint.path(file.id))/relay", method: "POST", json: ["target_device_id": target]) }
        catch { self.error = error.localizedDescription }
    }
    func retry(_ item: QueueItem) { do { try store?.retry(item); refresh(); Task { await send() } } catch { self.error = error.localizedDescription } }
}
