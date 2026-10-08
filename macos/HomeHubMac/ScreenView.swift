import SwiftUI
import WebKit

struct ScreenView: View {
    let trust: HubTrust
    @State private var error: String?
    var body: some View {
        VStack {
            ScreenCanvas(trust: trust, error: $error)
            if let error { Text(error).foregroundStyle(.red).padding() }
        }
    }
}
private struct ScreenCanvas: NSViewRepresentable {
    let trust: HubTrust
    @Binding var error: String?
    func makeCoordinator() -> Bridge { Bridge(trust: trust, error: $error) }
    func makeNSView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.mediaTypesRequiringUserActionForPlayback = []
        configuration.userContentController.add(context.coordinator, name: "screen")
        let view = WKWebView(frame: .zero, configuration: configuration)
        context.coordinator.view = view; view.navigationDelegate = context.coordinator
        if let url = Bundle.main.url(forResource: "viewer", withExtension: "html") {
            context.coordinator.allowed = url
            view.loadFileURL(url, allowingReadAccessTo: url)
        } else { error = NSLocalizedString("screen_unavailable", comment: "") }
        return view
    }
    func updateNSView(_ view: WKWebView, context: Context) {}
    static func dismantleNSView(_ view: WKWebView, coordinator: Bridge) {
        view.configuration.userContentController.removeScriptMessageHandler(forName: "screen")
        coordinator.closed = true; coordinator.stop()
        view.stopLoading(); view.navigationDelegate = nil
    }
    @MainActor final class Bridge: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
        let trust: HubTrust
        var error: Binding<String?>, session: String?, allowed: URL?, closed = false, starting = false
        weak var view: WKWebView?
        init(trust: HubTrust, error: Binding<String?>) { self.trust = trust; self.error = error }
        func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
            decisionHandler(action.request.url == allowed && action.targetFrame?.isMainFrame == true ? .allow : .cancel)
        }
        func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { self.error.wrappedValue = error.localizedDescription; stop() }
        func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { error.wrappedValue = NSLocalizedString("screen_unavailable", comment: ""); stop() }
        func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
            guard !closed, message.frameInfo.isMainFrame, message.frameInfo.request.url == allowed,
                  let body = message.body as? [String: Any], let action = body["action"] as? String else { return }
            switch action {
            case "offer":
                guard !starting, session == nil, let sdp = body["sdp"] as? String, Self.localSDP(sdp) else { return }
                starting = true
                Task {
                    do {
                        struct Answer: Decodable { let answer_sdp: String, session_id: String, transport: String? }
                        let answer = try await HubClient(trust).decode(Answer.self, "/v1/screen/view", method: "POST", json: ["offer_sdp": sdp, "direction": "view"])
                        session = try Endpoint.path(answer.session_id)
                        guard !closed, Self.localSDP(answer.answer_sdp), answer.transport != "loopback" else { stop(); throw HubError.invalid("screen_unavailable") }
                        _ = try await view?.callAsyncJavaScript("window.acceptAnswer(answer)", arguments: ["answer": answer.answer_sdp], in: nil, contentWorld: .page)
                    } catch { self.error.wrappedValue = error.localizedDescription; stop() }
                }
            case "heartbeat":
                guard let session else { return }
                Task { do { _ = try await HubClient(trust).request("/v1/screen/\(session)", method: "PATCH", json: [:]) } catch { self.error.wrappedValue = error.localizedDescription; stop() } }
            case "stop": stop()
            case "error": error.wrappedValue = NSLocalizedString("screen_unavailable", comment: ""); stop()
            default: break
            }
        }
        func stop() {
            guard let session else { return }; self.session = nil
            Task { do { _ = try await HubClient(trust).request("/v1/screen/\(session)", method: "DELETE") } catch { self.error.wrappedValue = error.localizedDescription } }
        }
        static func localSDP(_ sdp: String) -> Bool {
            guard !sdp.isEmpty, sdp.utf8.count <= 512 * 1024 else { return false }
            for line in sdp.components(separatedBy: .newlines) where line.hasPrefix("a=candidate:") {
                let values = line.split(whereSeparator: { $0 == " " || $0 == "\t" })
                guard values.count >= 8, values[6] == "typ", values[7] == "host" else { return false }
                let host = String(values[4])
                let address = host.contains(":") ? "[\(host)]:47800" : "\(host):47800"
                guard Endpoint.parse(address, port: 47800) != nil else { return false }
            }
            return sdp.contains("a=fingerprint:") && sdp.contains("m=video")
        }
    }
}
