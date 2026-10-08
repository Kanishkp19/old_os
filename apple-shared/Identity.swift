import Foundation
import Security
import CryptoKit

// One device-only Keychain identity per pairing. No private key export or
// password/defaults persistence. Share extension only writes staged queue files.
enum TrustStore {
    private static let service = "com.homehub.trust.v1"
    static func load() -> HubTrust? {
        var result: CFTypeRef?
        guard SecItemCopyMatching([kSecClass: kSecClassGenericPassword, kSecAttrService: service,
            kSecAttrAccount: "current", kSecReturnData: true, kSecMatchLimit: kSecMatchLimitOne,
            kSecUseDataProtectionKeychain: true] as CFDictionary, &result) == errSecSuccess,
            let data = result as? Data else { return nil }
        return try? JSONDecoder().decode(HubTrust.self, from: data)
    }
    static func save(_ trust: HubTrust) throws {
        let data = try JSONEncoder().encode(trust)
        let q: [CFString: Any] = [kSecClass: kSecClassGenericPassword, kSecAttrService: service,
            kSecAttrAccount: "current", kSecUseDataProtectionKeychain: true]
        let status = SecItemUpdate(q as CFDictionary, [kSecValueData: data] as CFDictionary)
        if status == errSecItemNotFound {
            var add = q; add[kSecValueData] = data; add[kSecAttrAccessible] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            guard SecItemAdd(add as CFDictionary, nil) == errSecSuccess else { throw HubError.security }
        } else if status != errSecSuccess { throw HubError.security }
    }
}

enum IdentityStore {
    static func create(tag: String) throws -> SecKey {
        var error: Unmanaged<CFError>?
        let attrs: [CFString: Any] = [kSecAttrKeyType: kSecAttrKeyTypeECSECPrimeRandom, kSecAttrKeySizeInBits: 256,
            kSecUseDataProtectionKeychain: true,
            kSecPrivateKeyAttrs: [kSecAttrIsPermanent: true, kSecAttrApplicationTag: Data(tag.utf8),
                kSecAttrAccessible: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly]]
        guard let key = SecKeyCreateRandomKey(attrs as CFDictionary, &error) else { throw HubError.security }
        return key
    }
    static func key(tag: String) throws -> SecKey {
        var item: CFTypeRef?
        guard SecItemCopyMatching([kSecClass: kSecClassKey, kSecAttrApplicationTag: Data(tag.utf8),
            kSecAttrKeyType: kSecAttrKeyTypeECSECPrimeRandom, kSecReturnRef: true,
            kSecUseDataProtectionKeychain: true] as CFDictionary, &item) == errSecSuccess, let item else { throw HubError.security }
        return item as! SecKey
    }
    static func removeUnpairedKey(tag: String) {
        SecItemDelete([kSecClass: kSecClassKey, kSecAttrApplicationTag: Data(tag.utf8),
            kSecUseDataProtectionKeychain: true] as CFDictionary)
    }
    static func certificate(_ pem: String) throws -> SecCertificate {
        let body = pem.components(separatedBy: .newlines).filter { !$0.hasPrefix("-----") }.joined()
        guard let d = Data(base64Encoded: body), let c = SecCertificateCreateWithData(nil, d as CFData) else { throw HubError.security }
        return c
    }
    static func fingerprint(_ cert: SecCertificate) -> String {
        SHA256.hash(data: SecCertificateCopyData(cert) as Data).map { String(format: "%02x", $0) }.joined()
    }
    static func install(_ pem: String, tag: String, ca: SecCertificate) throws {
        let certificate = try certificate(pem)
        guard let publicKey = SecCertificateCopyKey(certificate), let ownPublic = SecKeyCopyPublicKey(try key(tag: tag)),
              let received = SecKeyCopyExternalRepresentation(publicKey, nil) as Data?,
              let expected = SecKeyCopyExternalRepresentation(ownPublic, nil) as Data?, received == expected else { throw HubError.security }
        var trust: SecTrust?
        guard SecTrustCreateWithCertificates(certificate, SecPolicyCreateBasicX509(), &trust) == errSecSuccess, let trust else { throw HubError.security }
        SecTrustSetAnchorCertificates(trust, [ca] as CFArray); SecTrustSetAnchorCertificatesOnly(trust, true)
        SecTrustSetNetworkFetchAllowed(trust, false)
        guard SecTrustEvaluateWithError(trust, nil) else { throw HubError.security }
        let status = SecItemAdd([kSecClass: kSecClassCertificate, kSecValueRef: certificate,
            kSecAttrLabel: tag, kSecUseDataProtectionKeychain: true] as CFDictionary, nil)
        guard status == errSecSuccess || status == errSecDuplicateItem else { throw HubError.security }
    }
    static func identity(tag: String) throws -> SecIdentity {
        var result: CFTypeRef?
        guard SecItemCopyMatching([kSecClass: kSecClassIdentity, kSecAttrLabel: tag,
            kSecReturnRef: true, kSecMatchLimit: kSecMatchLimitAll, kSecUseDataProtectionKeychain: true] as CFDictionary, &result) == errSecSuccess,
              let candidates = result as? [SecIdentity] else { throw HubError.security }
        // Renewal leaves the prior cert installed until the new cert is durable.
        // Select the identity matching the current durable leaf, not an old cert.
        if let saved = TrustStore.load(), saved.keyTag == tag {
            let der = SecCertificateCopyData(try certificate(saved.certPem)) as Data
            for candidate in candidates {
                var cert: SecCertificate?
                if SecIdentityCopyCertificate(candidate, &cert) == errSecSuccess, let cert, SecCertificateCopyData(cert) as Data == der { return candidate }
            }
            throw HubError.security
        }
        guard let first = candidates.first else { throw HubError.security }; return first
    }
    static func csr(key: SecKey) throws -> String {
        guard let pub = SecKeyCopyPublicKey(key), let bytes = SecKeyCopyExternalRepresentation(pub, nil) as Data? else { throw HubError.security }
        // PKCS#10: version 0, subject CN=HomeHub, P-256 SPKI, empty attributes.
        let subject = DER.seq(DER.set(DER.seq(DER.oid([0x55,0x04,0x03]) + DER.wrap(0x0c, Data("HomeHub".utf8)))))
        let algorithm = DER.seq(DER.oid([0x2a,0x86,0x48,0xce,0x3d,0x02,0x01]) + DER.oid([0x2a,0x86,0x48,0xce,0x3d,0x03,0x01,0x07]))
        let info = DER.seq(DER.wrap(0x02, Data([0])) + subject + DER.seq(algorithm + DER.bits(bytes)) + DER.wrap(0xa0, Data()))
        var error: Unmanaged<CFError>?
        guard let sig = SecKeyCreateSignature(key, .ecdsaSignatureMessageX962SHA256, info as CFData, &error) as Data? else { throw HubError.security }
        let csr = DER.seq(info + DER.seq(DER.oid([0x2a,0x86,0x48,0xce,0x3d,0x04,0x03,0x02])) + DER.bits(sig))
        return "-----BEGIN CERTIFICATE REQUEST-----\n" + csr.base64EncodedString(options: [.lineLength64Characters, .endLineWithLineFeed]) + "\n-----END CERTIFICATE REQUEST-----\n"
    }
}
private enum DER {
    static func wrap(_ tag: UInt8, _ data: Data) -> Data {
        var len = data.count; var length = [UInt8]()
        if len < 128 { length = [UInt8(len)] } else {
            while len > 0 { length.insert(UInt8(len & 255), at: 0); len >>= 8 }
            length.insert(0x80 | UInt8(length.count), at: 0)
        }
        return Data([tag] + length) + data
    }
    static func seq(_ d: Data) -> Data { wrap(0x30, d) }
    static func set(_ d: Data) -> Data { wrap(0x31, d) }
    static func oid(_ d: [UInt8]) -> Data { wrap(0x06, Data(d)) }
    static func bits(_ d: Data) -> Data { wrap(0x03, Data([0]) + d) }
}
