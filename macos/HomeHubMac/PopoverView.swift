// PopoverView.swift — the menu-bar popover: status, drop zone, queue list.
import SwiftUI
import UniformTypeIdentifiers

struct PopoverView: View {
    @EnvironmentObject var model: AppModel
    @State private var isDropTarget = false
    @State private var pairInput = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let trust = model.trust {
                HStack {
                    Image(systemName: model.isReachable ? "checkmark.circle.fill" : "moon.zzz.fill")
                        .foregroundStyle(model.isReachable ? .green : .secondary)
                    VStack(alignment: .leading) {
                        Text(trust.name).font(.headline)
                        Text(model.isReachable ? "Home is nearby" : "Home is out of reach — items will send later")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }

                Text("Drop files here to send them Home")
                    .frame(maxWidth: .infinity, minHeight: 80)
                    .background(isDropTarget ? Color.accentColor.opacity(0.2) : Color.secondary.opacity(0.1))
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                    .onDrop(of: [.fileURL], isTargeted: $isDropTarget) { providers in
                        for p in providers {
                            _ = p.loadObject(ofClass: URL.self) { url, _ in
                                if let url { DispatchQueue.main.async { model.enqueue(urls: [url]) } }
                            }
                        }
                        return true
                    }

                if !model.queueItems.isEmpty {
                    List(model.queueItems) { item in
                        HStack {
                            Text(item.name).lineLimit(1)
                            Spacer()
                            Text(item.state).font(.caption).foregroundStyle(.secondary)
                        }
                    }
                    .frame(maxHeight: 160)
                }
            } else {
                Text("Connect to your Home").font(.headline)
                Text("On your computer running Home Hub, open the dashboard, choose “Add a device”, and paste the code text here.")
                    .font(.caption).foregroundStyle(.secondary)
                TextField("homehub://pair?…", text: $pairInput)
                Button("Connect") {
                    guard let payload = PairPayload(raw: pairInput) else { return }
                    Task {
                        if let trust = try? await MacPairingClient().pair(payload) {
                            MacTrustStore.save(trust)
                            await MainActor.run { model.trust = trust }
                        }
                    }
                }
                .disabled(!pairInput.hasPrefix("homehub://pair"))
            }
        }
        .padding(16)
        .frame(width: 320)
    }
}
