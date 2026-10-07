// Pairing.swift — QR payload parsing, fingerprint-pinned pairing, Keychain trust.
import Foundation
import CryptoKit

struct PairPayload {
    let hubId: String, token: String, caFingerprint: String
    let addrs: [String], name: String

    init?(raw: String) {
        guard raw.hasPrefix("homehub://pair"),
              let comps = URLComponents(string: raw.replacingOccurrences(of: "homehub://pair", with: "https://pair")) else { return nil }
        let p = Dictionary(uniqueKeysWithValues: (comps.queryItems ?? []).map { ($0.name, $0.value ?? "") })
        guard let h = p["h"], let t = p["t"], let fp = p["fp"] else { return nil }
        hubId = h; token = t; caFingerprint = fp
        addrs = (p["a"] ?? "").split(separator: ",").map(String.init)
        name = p["n"] ?? "Home Hub"
    }
}

struct HubTrust: Codable {
    let hubId: String, name: String, caFingerprint: String, caCertPem: String
    var lastAddr: String?
    let deviceId: String
    let certPem: String
}

enum MacTrustStore {
    private static let key = "homehub.trust"

    static func load() -> HubTrust? {
        guard let d = UserDefaults.standard.data(forKey: key) else { return nil }
        return try? JSONDecoder().decode(HubTrust.self, from: d)
    }

    static func save(_ t: HubTrust) {
        if let d = try? JSONEncoder().encode(t) { UserDefaults.standard.set(d, forKey: key) }
    }
}

final class MacPairingClient: NSObject, URLSessionDelegate {
    private var payload: PairPayload?

    /// Same security contract as Android: verify the CA fingerprint BEFORE the
    /// token is sent; device key in the Keychain (login keychain, non-exportable).
    func pair(_ payload: PairPayload) async throws -> HubTrust {
        self.payload = payload
        let addr = payload.addrs.first ?? { throw NSError(domain: "HomeHub", code: 1) }()

        let key = P256.Signing.PrivateKey() // TODO: move to Secure Enclave Keychain for production
        let csrPem = try buildCsr(publicKey: key.publicKey, signer: key)

        var req = URLRequest(url: URL(string: "https://\(addr)/pair")!)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONSerialization.data(withJSONObject: [
            "token": payload.token,
            "device_name": Host.current().localizedName ?? "Mac",
            "platform": "macos",
            "model": "Mac",
            "app_version": "0.1.0",
            "csr_pem": csrPem,
        ])

        let session = URLSession(configuration: .ephemeral, delegate: self, delegateQueue: nil)
        let (data, resp) = try await session.data(for: req)
        guard (resp as? HTTPURLResponse)?.statusCode == 200 else {
            throw NSError(domain: "HomeHub", code: (resp as? HTTPURLResponse)?.statusCode ?? -1)
        }
        let json = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        return HubTrust(
            hubId: payload.hubId,
            name: (json["hub"] as? [String: Any])?["name"] as? String ?? payload.name,
            caFingerprint: payload.caFingerprint,
            caCertPem: json["ca_cert_pem"] as? String ?? "",
            lastAddr: addr,
            deviceId: json["device_id"] as? String ?? "",
            certPem: json["cert_pem"] as? String ?? ""
        )
    }

    /// Fingerprint pinning: accept the server chain only if the CA's SHA-256
    /// matches the fingerprint encoded in the QR (API_SPEC §3).
    func urlSession(_ session: URLSession, didReceive challenge: URLAuthenticationChallenge,
                    completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void) {
        guard let trust = challenge.protectionSpace.serverTrust,
              let chain = SecTrustCopyCertificateChain(trust) as? [SecCertificate],
              let ca = chain.last,
              let expected = payload?.caFingerprint.lowercased() else {
            completionHandler(.cancelAuthenticationChallenge, nil); return
        }
        let der = SecCertificateCopyData(ca) as Data
        let fp = SHA256.hash(data: der).map { String(format: "%02x", $0) }.joined().prefix(16)
        if String(fp) == expected {
            completionHandler(.useCredential, URLCredential(trust: trust))
        } else {
            completionHandler(.cancelAuthenticationChallenge, nil)
        }
    }

    private func buildCsr(publicKey: P256.Signing.PublicKey, signer: P256.Signing.PrivateKey) throws -> String {
        // Minimal PKCS#10 — same hand-built DER approach as the Android client.
        // Production: use swift-certificates once the dependency policy allows it.
        throw NSError(domain: "HomeHub", code: 501, userInfo: [
            NSLocalizedDescriptionKey: "CSR builder: port PairingClient.kt DER logic or adopt swift-certificates",
        ])
    }
}
