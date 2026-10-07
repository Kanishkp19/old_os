// ContentView.swift — home + pairing entry.
import SwiftUI

struct ContentView: View {
    @EnvironmentObject var model: AppModel

    var body: some View {
        NavigationStack {
            if model.trust == nil {
                PairView()
            } else {
                HomeView()
            }
        }
    }
}

struct HomeView: View {
    @EnvironmentObject var model: AppModel

    var body: some View {
        List {
            Section {
                Label(model.trust?.name ?? "Home", systemImage: "house.fill")
                Text("Share to Home Hub from any app; items send when you're on home Wi-Fi.")
                    .foregroundStyle(.secondary)
            }
            Section("Waiting to send") {
                ForEach(model.queue.pending()) { item in
                    VStack(alignment: .leading) {
                        Text(item.name).lineLimit(1)
                        Text(item.state.rawValue).font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
        }
        .navigationTitle("Home Hub")
    }
}

/// QR scan → verify CA fingerprint → exchange token for device cert (T3).
struct PairView: View {
    @EnvironmentObject var model: AppModel
    @State private var error: String?

    var body: some View {
        VStack(spacing: 24) {
            Text("Connect to your Home").font(.title2)
            Text("Open Home Hub on your computer, choose “Add a device”, and scan the code it shows.")
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
            QRScannerView { raw in
                guard let payload = PairPayload(raw: raw) else {
                    error = "Not a Home Hub code"; return
                }
                Task {
                    do {
                        let trust = try await PairingClient().pair(payload)
                        TrustStore.save(trust)
                        await MainActor.run { model.trust = trust }
                    } catch {
                        await MainActor.run { self.error = error.localizedDescription }
                    }
                }
            }
            .frame(maxHeight: 320)
            .clipShape(RoundedRectangle(cornerRadius: 12))
            if let error { Text(error).foregroundStyle(.red) }
        }
        .padding(24)
    }
}
