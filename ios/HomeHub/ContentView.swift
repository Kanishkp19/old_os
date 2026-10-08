import SwiftUI

struct ContentView: View {
    @EnvironmentObject var model: AppModel
    var body: some View {
        NavigationStack {
            Group { if model.trust == nil { PairView() } else { HomeView() } }
                .navigationTitle("Home Hub")
                .overlay(alignment: .bottom) {
                    if let error = model.error { Text(error).font(.caption).foregroundStyle(.red).padding().background(.regularMaterial) }
                }
        }
    }
}
struct HomeView: View {
    @EnvironmentObject var model: AppModel
    var body: some View {
        List {
            Section {
                Label(model.trust?.name ?? "Home Hub", systemImage: "house.fill")
                Text("share_hint").foregroundStyle(.secondary)
                Button("send_now") { Task { await model.send() } }
            }
            Section("photos") {
                Button("backup_now") { model.backupNow() }.disabled(model.busy)
                Toggle("background_backup", isOn: Binding(get: { model.backupEnabled }, set: model.setBackupEnabled))
                Text("background_limits").font(.caption).foregroundStyle(.secondary)
                Text("originals_preserved").font(.caption)
            }
            Section("queue") {
                ForEach(model.items) { item in
                    VStack(alignment: .leading) {
                        Text(item.name).lineLimit(1)
                        Text(LocalizedStringKey("state_" + item.state.rawValue)).font(.caption).foregroundStyle(.secondary)
                        if let error = item.lastError { Text(error).font(.caption).foregroundStyle(.red) }
                        if item.state == .failedPerm || item.state == .failedRetry { Button("retry") { model.retry(item) } }
                    }
                }
            }
        }.refreshable { await model.send() }
    }
}
struct PairView: View {
    @EnvironmentObject var model: AppModel
    @State private var text = ""
    var body: some View {
        VStack(spacing: 16) {
            Text("connect_home").font(.title2)
            Text("scan_hint").multilineTextAlignment(.center).foregroundStyle(.secondary)
            QRScannerView { raw in if !model.busy { Task { await model.pair(raw) } } }
                .frame(maxHeight: 300).clipShape(RoundedRectangle(cornerRadius: 12))
            TextField("pairing_text", text: $text).textInputAutocapitalization(.never).autocorrectionDisabled()
            Button("connect") { Task { await model.pair(text) } }.disabled(model.busy || text.isEmpty)
            if model.busy { ProgressView() }
        }.padding(24)
    }
}
