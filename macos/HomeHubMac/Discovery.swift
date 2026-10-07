// Discovery.swift — Bonjour browse for _homehub._tcp (API_SPEC §2).
import Foundation
import Network

final class BonjourDiscovery: NSObject, NetServiceBrowserDelegate, NetServiceDelegate {
    var onHubFound: ((String, Int) -> Void)?
    private let browser = NetServiceBrowser()

    func start() {
        browser.delegate = self
        browser.searchForServices(ofType: "_homehub._tcp", inDomain: "local.")
    }

    func netServiceBrowser(_ browser: NetServiceBrowser, didFind service: NetService, moreComing: Bool) {
        service.delegate = self
        service.resolve(withTimeout: 5)
    }

    func netServiceDidResolveAddress(_ sender: NetService) {
        guard let host = sender.hostName else { return }
        onHubFound?(host, sender.port)
    }
}
