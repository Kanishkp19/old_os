import Foundation
import Security
import Darwin

final class PinnedSession: NSObject, URLSessionDelegate, URLSessionTaskDelegate {
    private let ca: SecCertificate?, identity: SecIdentity?, origin: URL, bootstrap: Bool
    init(origin: URL, ca: SecCertificate? = nil, identity: SecIdentity? = nil, bootstrap: Bool = false) {
        self.origin = origin; self.ca = ca; self.identity = identity; self.bootstrap = bootstrap
    }
    lazy var session: URLSession = {
        let config = URLSessionConfiguration.ephemeral
        config.tlsMinimumSupportedProtocolVersion = .TLSv13
        config.timeoutIntervalForRequest = 30; config.timeoutIntervalForResource = 3600
        config.allowsCellularAccess = false; config.allowsExpensiveNetworkAccess = false
        config.urlCache = nil; config.httpCookieStorage = nil
        config.httpShouldSetCookies = false
        return URLSession(configuration: config, delegate: self, delegateQueue: nil)
    }()
    func close() { session.invalidateAndCancel() }
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) { completionHandler(nil) }
    func urlSession(_ session: URLSession, didReceive challenge: URLAuthenticationChallenge,
                    completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void) {
        guard challenge.protectionSpace.host == origin.host, challenge.protectionSpace.port == origin.port,
              challenge.previousFailureCount == 0 else { completionHandler(.cancelAuthenticationChallenge, nil); return }
        switch challenge.protectionSpace.authenticationMethod {
        case NSURLAuthenticationMethodServerTrust:
            guard let trust = challenge.protectionSpace.serverTrust else { completionHandler(.cancelAuthenticationChallenge, nil); return }
            if bootstrap {
                // This separate session sends only GET /pair/ca, no credentials.
                // Its untrusted response is authenticated against QR before use.
                completionHandler(.useCredential, URLCredential(trust: trust)); return
            }
            guard let ca else { completionHandler(.cancelAuthenticationChallenge, nil); return }
            SecTrustSetAnchorCertificates(trust, [ca] as CFArray); SecTrustSetAnchorCertificatesOnly(trust, true)
            SecTrustSetNetworkFetchAllowed(trust, false)
            SecTrustSetPolicies(trust, SecPolicyCreateSSL(true, "homehub.local" as CFString))
            guard SecTrustEvaluateWithError(trust, nil) else { completionHandler(.cancelAuthenticationChallenge, nil); return }
            completionHandler(.useCredential, URLCredential(trust: trust))
        case NSURLAuthenticationMethodClientCertificate:
            guard !bootstrap, let identity else { completionHandler(.cancelAuthenticationChallenge, nil); return }
            completionHandler(.useCredential, URLCredential(identity: identity, certificates: nil, persistence: .forSession))
        default: completionHandler(.cancelAuthenticationChallenge, nil)
        }
    }
}

private actor DownloadRegistry {
    private var keys = Set<String>()
    func acquire(_ key: String) -> Bool { keys.insert(key).inserted }
    func release(_ key: String) { keys.remove(key) }
}

final class HubClient {
    private static let downloads = DownloadRegistry()
    let trust: HubTrust
    private let connection: PinnedSession
    init(_ trust: HubTrust) throws {
        guard let origin = Endpoint.parse(trust.lastAddr, port: 47800) else { throw HubError.security }
        self.trust = trust
        connection = PinnedSession(origin: origin, ca: try IdentityStore.certificate(trust.caCertPem), identity: try IdentityStore.identity(tag: trust.keyTag))
    }
    deinit { connection.close() }
    func request(_ path: String, method: String = "GET", json: Any? = nil, data: Data? = nil, hash: String? = nil) async throws -> Data {
        guard path.hasPrefix("/v1/"), let origin = Endpoint.parse(trust.lastAddr, port: 47800),
              let url = URL(string: path, relativeTo: origin)?.absoluteURL, url.host == origin.host, url.port == origin.port else { throw HubError.security }
        var request = URLRequest(url: url); request.httpMethod = method
        if let json { request.httpBody = try JSONSerialization.data(withJSONObject: json); request.setValue("application/json", forHTTPHeaderField: "Content-Type") }
        if let data { request.httpBody = data; request.setValue("application/octet-stream", forHTTPHeaderField: "Content-Type") }
        if let hash { request.setValue(hash, forHTTPHeaderField: "X-Chunk-Hash") }
        try Task.checkCancellation()
        let (bytes, response) = try await connection.session.bytes(for: request)
        var body = Data()
        for try await byte in bytes {
            guard body.count < 8 * 1024 * 1024 else { throw HubError.invalid("response_too_large") }
            body.append(byte)
        }
        try Self.validate(response, body: body); return body
    }
    static func validate(_ response: URLResponse, body: Data) throws {
        guard let http = response as? HTTPURLResponse else { throw HubError.unavailable }
        if !(200...299).contains(http.statusCode) {
            let error = try? JSONDecoder().decode(ErrorReply.self, from: body)
            throw HubError.http(http.statusCode, error?.error.code ?? "request_failed", error?.error.retryable ?? (http.statusCode >= 500 || http.statusCode == 429))
        }
    }
    func decode<T: Decodable>(_ type: T.Type, _ path: String, method: String = "GET", json: Any? = nil) async throws -> T {
        try JSONDecoder().decode(type, from: await request(path, method: method, json: json))
    }
    func download(_ file: HubFile, to destination: URL) async throws {
        guard let origin = Endpoint.parse(trust.lastAddr, port: 47800), file.hash.count == 64,
              file.hash.allSatisfy({ $0.isASCII && $0.isHexDigit }) else { throw HubError.security }
        let id = try Endpoint.path(file.id)
        let hub = try Endpoint.path(trust.hubId)
        let cacheKey = hub + "_" + id + "_" + file.hash
        guard await Self.downloads.acquire(cacheKey) else { throw HubError.invalid("download_busy") }
        defer { Task { await Self.downloads.release(cacheKey) } }
        let cache = try FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
            .appendingPathComponent("HomeHub/downloads", isDirectory: true)
        try FileManager.default.createDirectory(at: cache, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let partial = cache.appendingPathComponent("\(hub)_\(id)_\(file.hash).partial")
        if !FileManager.default.fileExists(atPath: partial.path) {
            guard FileManager.default.createFile(atPath: partial.path, contents: Data(), attributes: [.posixPermissions: 0o600]) else { throw HubError.invalid("source_unavailable") }
        }
        let output = try FileHandle(forWritingTo: partial); defer { try? output.close() }
        let blockSize: UInt64 = 4 * 1024 * 1024
        let present = (try FileManager.default.attributesOfItem(atPath: partial.path)[.size] as? NSNumber)?.uint64Value ?? 0
        // Interrupted appends are discarded back to the last full range. A
        // complete final range is retained and still checked before install.
        var offset = present == file.size ? present : min(file.size, present / blockSize * blockSize)
        try output.truncate(atOffset: offset); try output.seek(toOffset: offset)
        while offset < file.size {
            try Task.checkCancellation()
            let end = min(file.size, offset + blockSize) - 1
            var request = URLRequest(url: origin.appendingPathComponent("v1/files/\(id)/content"))
            request.setValue("bytes=\(offset)-\(end)", forHTTPHeaderField: "Range")
            let (temp, response) = try await connection.session.download(for: request)
            defer { try? FileManager.default.removeItem(at: temp) }
            try Self.validate(response, body: Data())
            guard let http = response as? HTTPURLResponse,
                  http.statusCode == 206,
                  http.value(forHTTPHeaderField: "Content-Range") == "bytes \(offset)-\(end)/\(file.size)",
                  (try FileManager.default.attributesOfItem(atPath: temp.path)[.size] as? NSNumber)?.uint64Value == end - offset + 1 else { throw HubError.invalid("verification_failed") }
            let input = try FileHandle(forReadingFrom: temp); defer { try? input.close() }
            while let bytes = try input.read(upToCount: 1024 * 1024), !bytes.isEmpty { try Task.checkCancellation(); try output.write(contentsOf: bytes) }
            try output.synchronize(); offset = end + 1
        }
        try output.synchronize()
        let descriptor = open(cache.path, O_RDONLY)
        guard descriptor >= 0 else { throw HubError.invalid("source_unavailable") }
        defer { close(descriptor) }
        guard fsync(descriptor) == 0 else { throw HubError.invalid("source_unavailable") }
        guard try Blake3.fileHex(partial) == file.hash.lowercased() else {
            try output.truncate(atOffset: 0); try output.synchronize()
            throw HubError.invalid("verification_failed")
        }
        // Install only after complete verification. A sibling copy gives the
        // final rename atomicity on the destination volume, without overwrite.
        let ready = destination.deletingLastPathComponent().appendingPathComponent(".homehub-" + UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: ready) }
        try FileManager.default.copyItem(at: partial, to: ready)
        let handle = try FileHandle(forWritingTo: ready); try handle.synchronize(); try handle.close()
        try FileManager.default.moveItem(at: ready, to: destination)
        let destinationDirectory = open(destination.deletingLastPathComponent().path, O_RDONLY)
        guard destinationDirectory >= 0 else { throw HubError.invalid("source_unavailable") }
        defer { close(destinationDirectory) }
        guard fsync(destinationDirectory) == 0 else { throw HubError.invalid("source_unavailable") }
        try? FileManager.default.removeItem(at: partial)
    }
    func renewedIfDue() async throws -> HubTrust {
        guard trust.certExpiresAt - Int64(Date().timeIntervalSince1970 * 1000) < 30 * 86400 * 1000 else { return trust }
        struct Renew: Decodable { let cert_pem: String, cert_expires_at: Int64 }
        let csr = try IdentityStore.csr(key: IdentityStore.key(tag: trust.keyTag))
        let reply = try await decode(Renew.self, "/v1/certs/renew", method: "POST", json: ["csr_pem": csr])
        try IdentityStore.install(reply.cert_pem, tag: trust.keyTag, ca: IdentityStore.certificate(trust.caCertPem))
        var updated = trust; updated.certPem = reply.cert_pem; updated.certExpiresAt = reply.cert_expires_at
        try TrustStore.save(updated); return updated
    }
}

final class PairingClient {
    func pair(_ payload: PairPayload, deviceName: String, platform: String) async throws -> HubTrust {
        var lastError: Error = HubError.unavailable
        // Try another QR address only during CA bootstrap, never after sending
        // a one-use token: an interrupted successful claim must be re-paired.
        for address in payload.addrs {
            guard let origin = Endpoint.parse(address, port: 47802) else { continue }
            let bootstrap = PinnedSession(origin: origin, bootstrap: true)
            let caPEM: String
            do {
                let (bytes, response) = try await bootstrap.session.bytes(from: origin.appendingPathComponent("pair/ca"))
                var data = Data()
                for try await byte in bytes { guard data.count < 16384 else { throw HubError.security }; data.append(byte) }
                try HubClient.validate(response, body: data)
                struct CA: Decodable { let ca_cert_pem: String }
                caPEM = try JSONDecoder().decode(CA.self, from: data).ca_cert_pem
                let certificate = try IdentityStore.certificate(caPEM)
                guard IdentityStore.fingerprint(certificate).hasPrefix(payload.caFingerprint) else { throw HubError.security }
                bootstrap.close()
            } catch { bootstrap.close(); lastError = error; continue }
            let ca = try IdentityStore.certificate(caPEM)
            let tag = "com.homehub.identity." + UUID().uuidString
            let key = try IdentityStore.create(tag: tag)
            let pinned = PinnedSession(origin: origin, ca: ca)
            defer { pinned.close() }
            do {
                var request = URLRequest(url: origin.appendingPathComponent("pair")); request.httpMethod = "POST"
                request.setValue("application/json", forHTTPHeaderField: "Content-Type")
                request.httpBody = try JSONSerialization.data(withJSONObject: ["token": payload.token, "device_name": deviceName,
                    "platform": platform, "model": platform, "app_version": "0.1.0", "csr_pem": IdentityStore.csr(key: key)])
                let (bytes, response) = try await pinned.session.bytes(for: request)
                var data = Data()
                for try await byte in bytes { guard data.count < 65536 else { throw HubError.security }; data.append(byte) }
                try HubClient.validate(response, body: data)
                let reply = try JSONDecoder().decode(PairReply.self, from: data)
                guard reply.hub.id == payload.hubId,
                      IdentityStore.fingerprint(try IdentityStore.certificate(reply.ca_cert_pem)) == IdentityStore.fingerprint(ca) else { throw HubError.security }
                try IdentityStore.install(reply.cert_pem, tag: tag, ca: ca)
                let trust = HubTrust(hubId: reply.hub.id, name: reply.hub.name, caFingerprint: IdentityStore.fingerprint(ca), caCertPem: caPEM,
                    deviceId: reply.device_id, keyTag: tag, certPem: reply.cert_pem, certExpiresAt: reply.cert_expires_at, lastAddr: address, scopes: reply.scopes)
                try TrustStore.save(trust); return trust
            } catch { IdentityStore.removeUnpairedKey(tag: tag); throw error }
        }
        throw lastError
    }
}
