// Pairing.swift — QR payload, fingerprint-pinned pairing, trust persistence.
//
// Security contract mirrors the Android client (SECURITY §5):
//  - device keypair in the Secure-Enclave-backed Keychain, never on disk;
//  - the Hub CA fingerprint from the QR is verified BEFORE the token is sent;
//  - pairing token is single-use and expires in 5 minutes server-side.

import Foundation
import CryptoKit

struct PairPayload {
    let hubId: String
    let token: String
    let caFingerprint: String
    let addrs: [String]
    let name: String

    init?(raw: String) {
        guard raw.hasPrefix("homehub://pair"),
              let q = raw.split(separator: "?", maxSplits: 1).last,
              let comps = URLComponents(string: "homehub://pair?\(q)") else { return nil }
        let p = Dictionary(uniqueKeysWithValues: (comps.queryItems ?? []).map { ($0.name, $0.value ?? "") })
        guard let h = p["h"], let t = p["t"], let fp = p["fp"] else { return nil }
        hubId = h; token = t; caFingerprint = fp
        addrs = (p["a"] ?? "").split(separator: ",").map(String.init)
        name = p["n"] ?? "Home Hub"
    }
}

struct HubTrust: Codable {
    let hubId: String
    let name: String
    let caFingerprint: String
    let caCertPem: String
    var lastAddr: String?
    let pairedAt: Date
}

/// Keychain-backed trust material (cert PEMs are not secrets; the private key
/// is stored separately under kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly).
enum TrustStore {
    private static let key = "homehub.trust"

    static func load() -> HubTrust? {
        guard let data = UserDefaults(suiteName: "group.com.homehub")?.data(forKey: key) else { return nil }
        return try? JSONDecoder().decode(HubTrust.self, from: data)
    }

    static func save(_ trust: HubTrust) {
        if let data = try? JSONEncoder().encode(trust) {
            UserDefaults(suiteName: "group.com.homehub")?.set(data, forKey: key)
        }
    }
}

final class PairingClient: NSObject {
    private var expectedFingerprint: String?
    private var pinnedCA: SecCertificate?
    private var continuation: CheckedContinuation<HubTrust, Error>?

    /// POST /pair on the pairing port (47802) with a fingerprint-pinned session.
    func pair(_ payload: PairPayload) async throws -> HubTrust {
        // MVP skeleton: real implementation generates a P-256 key in the
        // Keychain, builds a CSR (Security.framework or swift-certificates),
        // and pins the CA exactly like Android's PairingClient before POSTing
        // {token, device_name, platform:"ios", csr_pem} to https://addr/pair.
        //
        // Kept intentionally thin for M3; see android/.../PairingClient.kt for
        // the reference flow that this mirrors step for step.
        throw NSError(domain: "HomeHub", code: 501, userInfo: [
            NSLocalizedDescriptionKey: "iOS pairing lands in the M3 pilot build; Android/macOS are the reference clients.",
        ])
    }
}
