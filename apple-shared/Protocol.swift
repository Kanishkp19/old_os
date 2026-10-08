import Foundation

// Explicit wire names keep both companions aligned with hh-core/types.rs.
struct PairPayload {
    let hubId: String, token: String, caFingerprint: String, addrs: [String], name: String
    init?(raw: String) {
        guard raw.count <= 8192, let c = URLComponents(string: raw), c.scheme == "homehub", c.host == "pair" else { return nil }
        var q = [String: String]()
        for item in c.queryItems ?? [] {
            guard q[item.name] == nil, let value = item.value else { return nil }
            q[item.name] = value
        }
        guard let h = q["h"], !h.isEmpty, let t = q["t"], !t.isEmpty,
              let fp = (q["fp_sha256"] ?? q["fp"])?.lowercased(), [16, 64].contains(fp.count), fp.allSatisfy({ $0.isHexDigit }),
              let addresses = q["a"] else { return nil }
        let a = addresses.split(separator: ",").map(String.init)
        guard !a.isEmpty, a.allSatisfy({ Endpoint.parse($0, port: 47802) != nil }) else { return nil }
        hubId = h; token = t; caFingerprint = fp; addrs = a; name = q["n"] ?? "Home Hub"
    }
}

enum HubError: LocalizedError {
    case invalid(String), http(Int, String, Bool), security, unavailable
    var errorDescription: String? {
        switch self {
        case .invalid(let key): return NSLocalizedString(key, comment: "")
        case .http(_, let code, _):
            let message = NSLocalizedString(code, comment: "")
            return message == code ? NSLocalizedString("request_failed", comment: "") : message
        case .security: return NSLocalizedString("connection_untrusted", comment: "")
        case .unavailable: return NSLocalizedString("home_unavailable", comment: "")
        }
    }
    var retryable: Bool { if case .http(_, _, let retry) = self { return retry }; if case .security = self { return false }; if case .invalid = self { return false }; return true }
}

enum Endpoint {
    // QR/discovery is untrusted. Restrict destinations to local address forms;
    // Bonjour names remain .local and literal IPs must be private/link-local.
    static func parse(_ address: String, port: Int) -> URL? {
        guard !address.contains("@"), !address.contains("/"), !address.contains("?"), !address.contains("#"),
              var c = URLComponents(string: "https://" + address), let host = c.host,
              c.port == nil || (1...65535).contains(c.port!) else { return nil }
        let lower = host.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "[]"))
        let octets = lower.split(separator: ".").compactMap { Int($0) }
        let localV4 = octets.count == 4 && octets.allSatisfy({ (0...255).contains($0) }) &&
            (octets[0] == 10 || (octets[0] == 172 && (16...31).contains(octets[1])) ||
             (octets[0] == 192 && octets[1] == 168) || (octets[0] == 169 && octets[1] == 254))
        let localV6 = lower.contains(":") && (lower.hasPrefix("fc") || lower.hasPrefix("fd") || lower.hasPrefix("fe80:"))
        guard localV4 || localV6 || lower.hasSuffix(".local") || lower.hasSuffix(".local.") else { return nil }
        c.port = port; return c.url
    }
    static func path(_ id: String) throws -> String {
        guard !id.isEmpty, id.allSatisfy({ $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_") }) else { throw HubError.invalid("invalid_identifier") }
        return id
    }
}
struct HubTrust: Codable {
    let hubId: String, name: String, caFingerprint: String, caCertPem: String, deviceId: String, keyTag: String
    var certPem: String, certExpiresAt: Int64, lastAddr: String
    var scopes: [String]
}
struct PairReply: Decodable {
    struct Hub: Decodable { let id: String, name: String }
    let device_id: String, cert_pem: String, ca_cert_pem: String, cert_expires_at: Int64, scopes: [String], hub: Hub
}
struct Bitmap: Codable {
    let encoding: String, ranges: [[Int]]
    func contains(_ i: Int) -> Bool { ranges.contains { $0.count == 2 && i >= $0[0] && i <= $0[1] } }
}
struct TransferCreated: Decodable {
    let transfer_id: String, chunk_size: Int, chunk_count: Int, have: Bitmap, already_exists: Bool, existing_file_id: String?
}
struct TransferStatus: Decodable { let status: String, have: Bitmap }
struct TransferComplete: Decodable { let file_id: String, verified: Bool, hash: String, size: UInt64 }
struct HubFile: Codable, Identifiable { let id: String, name: String, category: String, size: UInt64, hash: String; let mime: String? }
struct FilePage: Decodable { let items: [HubFile], next_cursor: String? }
struct BackupSource: Decodable { let id: String }
struct BackupDiff: Decodable { let needed: [String] }
struct ErrorReply: Decodable {
    struct Body: Decodable { let code: String, retryable: Bool }
    let error: Body
}
