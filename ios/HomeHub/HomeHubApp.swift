import SwiftUI
import BackgroundTasks
import UIKit

@main
@MainActor
struct HomeHubApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var model = AppModel.shared
    @Environment(\.scenePhase) private var phase
    var body: some Scene {
        WindowGroup { ContentView().environmentObject(model).onChange(of: phase, initial: true) { _, phase in model.setActive(phase == .active) } }
    }
}
final class AppDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication, didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: "com.homehub.backup", using: nil) { task in
            guard let task = task as? BGProcessingTask else { task.setTaskCompleted(success: false); return }
            let work = Task { @MainActor in
                await AppModel.shared.backgroundRun(task)
            }
            task.expirationHandler = { work.cancel() }
        }
        return true
    }
}

@MainActor final class AppModel: ObservableObject {
    static let shared = AppModel()
    @Published var trust: HubTrust? = TrustStore.load()
    @Published var items: [QueueItem] = []
    @Published var error: String?
    @Published var busy = false
    @Published var backupEnabled = UserDefaults.standard.bool(forKey: "backupEnabled")
    private var foreground: Task<Void, Never>?
    private var backupWork: Task<Void, Never>?
    private let discovery = BonjourDiscovery()
    private var store: QueueStore?
    private var engine: UploadEngine?
    init() {
        do { let store = try QueueStore(); self.store = store; engine = UploadEngine(store: store); refresh(); if let trust { UserDefaults(suiteName: "group.com.homehub")?.set(trust.hubId, forKey: "hubId") } }
        catch { self.error = error.localizedDescription }
        discovery.onHubFound = { [weak self] id, host, port, fp in
            Task { @MainActor in
                guard let self, var trust = self.trust, trust.hubId == id, !fp.isEmpty, trust.caFingerprint.hasPrefix(fp),
                      Endpoint.parse("\(host):\(port)", port: 47800) != nil else { return }
                trust.lastAddr = "\(host):\(port)"
                do {
                    _ = try await HubClient(trust).request("/v1/info")
                    guard var current = TrustStore.load(), current.hubId == id else { return }
                    current.lastAddr = trust.lastAddr; try TrustStore.save(current); self.trust = current
                } catch { self.error = error.localizedDescription }
            }
        }
        discovery.start()
    }
    func refresh() { do { items = try store?.all() ?? [] } catch { self.error = error.localizedDescription } }
    func pair(_ raw: String) async {
        guard let payload = PairPayload(raw: raw) else { error = NSLocalizedString("invalid_code", comment: ""); return }
        busy = true; defer { busy = false }
        do { trust = try await PairingClient().pair(payload, deviceName: UIDevice.current.name, platform: "ios"); UserDefaults(suiteName: "group.com.homehub")?.set(trust?.hubId, forKey: "hubId"); error = nil; setActive(true) }
        catch { self.error = error.localizedDescription }
    }
    func setActive(_ active: Bool) {
        foreground?.cancel(); foreground = nil
        if active {
            foreground = Task {
                while !Task.isCancelled {
                    await send()
                    do { try await Task.sleep(for: .seconds(30)) } catch { return }
                }
            }
        } else { backupWork?.cancel(); scheduleBackground() }
    }
    func send(force: Bool = false) async {
        guard !busy || force else { return }
        guard let trust, let engine else { refresh(); return }
        do { try await engine.pump(trust); self.trust = TrustStore.load(); refresh() }
        catch is CancellationError {} catch { self.error = error.localizedDescription }
    }
    func backupNow() {
        guard backupWork == nil else { return }
        backupWork = Task {
            busy = true; defer { busy = false; backupWork = nil }
            do { try await discoverPhotos() }
            catch is CancellationError { return } catch { self.error = error.localizedDescription }
            await send(force: true)
        }
    }
    private func discoverPhotos() async throws {
        guard let trust, let store, let engine else { throw HubError.invalid("connect_first") }
        try await PhotoBackup.discover(trust: trust, store: store, engine: engine)
        refresh()
    }
    func setBackupEnabled(_ enabled: Bool) { backupEnabled = enabled; UserDefaults.standard.set(enabled, forKey: "backupEnabled"); scheduleBackground() }
    func retry(_ item: QueueItem) { do { try store?.retry(item); refresh(); Task { await send() } } catch { self.error = error.localizedDescription } }
    func scheduleBackground() {
        let identifier = "com.homehub.backup"
        BGTaskScheduler.shared.cancel(taskRequestWithIdentifier: identifier)
        guard backupEnabled || items.contains(where: { $0.state != .done && $0.state != .failedPerm }) else { return }
        let request = BGProcessingTaskRequest(identifier: identifier)
        // Do not let an Internet-connectivity requirement exclude offline LANs.
        // URLSession still restricts transfers to non-cellular local endpoints.
        request.requiresNetworkConnectivity = false; request.requiresExternalPower = true
        request.earliestBeginDate = Date().addingTimeInterval(15 * 60)
        do { try BGTaskScheduler.shared.submit(request) } catch { self.error = error.localizedDescription }
    }
    func backgroundRun(_ task: BGProcessingTask) async {
        var success = false
        defer { refresh(); scheduleBackground(); task.setTaskCompleted(success: success) }
        do {
            if backupEnabled { try await discoverPhotos() }
            try Task.checkCancellation()
            if let trust, let engine { try await engine.pump(trust) }
            success = !Task.isCancelled
        } catch is CancellationError {} catch { self.error = error.localizedDescription }
    }
}
