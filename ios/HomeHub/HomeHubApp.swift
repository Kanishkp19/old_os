// HomeHubApp.swift — iOS MVP skeleton (M3, IMPLEMENTATION_PLAN §Phase 2).
//
// Scope for the MVP: pair via QR, queue-and-send-later via share extension,
// photo backup. Reuses the exact hub protocol (API_SPEC v1) — the server does
// not know or care which client platform is talking.
//
// Project layout (open with Xcode 16+, iOS 17+):
//   ios/HomeHub/           app target
//   ios/HomeHubShare/      share extension target (enqueues into App Group)
//
// NOTE: this is a bring-up skeleton, not a shipping app — the pairing and
// upload flows are implemented, gallery and remote are intentionally absent.

import SwiftUI

@main
struct HomeHubApp: App {
    @StateObject private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(model)
        }
    }
}

final class AppModel: ObservableObject {
    @Published var trust: HubTrust?
    let queue = UploadQueue()

    init() {
        trust = TrustStore.load()
    }
}
