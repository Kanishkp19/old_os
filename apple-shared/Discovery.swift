import Foundation

final class BonjourDiscovery: NSObject, NetServiceBrowserDelegate, NetServiceDelegate {
    var onHubFound: ((String, String, Int, String) -> Void)?
    private let browser = NetServiceBrowser()
    private var services = [NetService]()
    func start() { browser.delegate = self; browser.searchForServices(ofType: "_homehub._tcp", inDomain: "local.") }
    func netServiceBrowser(_ browser: NetServiceBrowser, didFind service: NetService, moreComing: Bool) {
        services.append(service); service.delegate = self; service.resolve(withTimeout: 5)
    }
    func netServiceBrowser(_ browser: NetServiceBrowser, didRemove service: NetService, moreComing: Bool) { services.removeAll { $0 == service } }
    func netServiceDidResolveAddress(_ sender: NetService) {
        guard let host = sender.hostName, let data = sender.txtRecordData() else { return }
        let txt = NetService.dictionary(fromTXTRecord: data)
        guard let idData = txt["id"], let id = String(data: idData, encoding: .utf8),
              let fpData = txt["fp"], let fp = String(data: fpData, encoding: .utf8) else { return }
        onHubFound?(id, host, sender.port, fp.lowercased())
    }
}
