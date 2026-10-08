import SwiftUI
import UniformTypeIdentifiers

struct PopoverView: View {
    @EnvironmentObject var model: AppModel
    @Environment(\.openWindow) private var openWindow
    @State private var pairInput = ""
    @State private var drop = false
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let trust = model.trust {
                Label(trust.name, systemImage: model.isReachable ? "house.fill" : "house")
                Text(model.isReachable ? "home_nearby" : "home_unavailable").font(.caption)
                Text("drop_files").frame(maxWidth: .infinity, minHeight: 70)
                    .background(drop ? Color.accentColor.opacity(0.2) : Color.secondary.opacity(0.1))
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                    .onDrop(of: [.fileURL], isTargeted: $drop) { providers in
                        for provider in providers { _ = provider.loadObject(ofClass: URL.self) { url, _ in if let url { Task { @MainActor in model.enqueue(urls: [url]) } } } }; return true
                    }
                Button("choose_files") { let panel = NSOpenPanel(); panel.allowsMultipleSelection = true; if panel.runModal() == .OK { model.enqueue(urls: panel.urls) } }
                HStack { Button("library") { openWindow(id: "library") }; Button("received") { do { try RelayReceiver.reveal() } catch { model.error = error.localizedDescription } } }
                Button("view_screen") { openWindow(id: "screen") }
                QueueList().frame(maxHeight: 180)
                Button("send_now") { Task { await model.send() } }
            } else {
                Text("connect_home").font(.headline)
                Text("paste_hint").font(.caption).foregroundStyle(.secondary)
                TextField("pairing_text", text: $pairInput)
                Button("connect") { Task { await model.pair(pairInput) } }.disabled(model.busy || pairInput.isEmpty)
            }
            if model.busy { ProgressView() }
            if let error = model.error { Text(error).font(.caption).foregroundStyle(.red) }
        }.padding(16).frame(width: 360)
    }
}
struct QueueList: View {
    @EnvironmentObject var model: AppModel
    var body: some View {
        List(model.items) { item in
            VStack(alignment: .leading) {
                Text(item.name).lineLimit(1)
                Text(LocalizedStringKey("state_" + item.state.rawValue)).font(.caption)
                if let error = item.lastError { Text(error).font(.caption).foregroundStyle(.red) }
                if item.state == .failedRetry || item.state == .failedPerm { Button("retry") { model.retry(item) } }
            }
        }
    }
}
struct LibraryView: View {
    @EnvironmentObject var model: AppModel
    @State private var target = ""
    var body: some View {
        VStack {
            HStack { TextField("search_files", text: $model.query).onSubmit { Task { await model.browse() } }; Button("search") { Task { await model.browse() } } }
            TextField("recipient_device_id", text: $target)
            List(model.files) { file in
                HStack {
                    VStack(alignment: .leading) { Text(file.name); Text(ByteCountFormatter.string(fromByteCount: Int64(clamping: file.size), countStyle: .file)).font(.caption) }
                    Spacer()
                    Button("download") { model.download(file) }
                    Button("relay") { Task { await model.relayFile(file, target: target) } }.disabled(target.isEmpty)
                }
            }
            if model.cursor != nil { Button("load_more") { Task { await model.browse(more: true) } } }
            if let error = model.error { Text(error).foregroundStyle(.red) }
        }.padding().task { await model.browse() }
    }
}
