import XCTest
import Security
#if os(macOS)
@testable import HomeHubMac
#else
@testable import HomeHub
#endif

final class HomeHubTests: XCTestCase {
    func testBlake3PublishedVectors() {
        XCTAssertEqual(Blake3.hex(Data()), "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262")
        XCTAssertEqual(Blake3.hex(Data("abc".utf8)), "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85")
    }
    func testPublishedTreeVectors() {
        // BLAKE3-team/BLAKE3 test_vectors/test_vectors.json (0..250 repeat).
        let vectors = [
            1024: "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af71c",
            1025: "d00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444f4",
            2048: "e776b6028c7cd22a4d0ba182a8bf62205d2ef576467e838ed6f2529b85fba24a9"
        ]
        for (size, expected) in vectors {
            XCTAssertEqual(Blake3.hex(Data((0..<size).map { UInt8($0 % 251) })), expected)
        }
    }
    func testFullFingerprintOverridesLegacyPrefix() {
        let full = String(repeating: "b", count: 64)
        let raw = "homehub://pair?h=hub&t=token&fp=0123456789abcdef&fp_sha256=\(full)&a=192.168.1.2:47802"
        XCTAssertEqual(PairPayload(raw: raw)?.caFingerprint, full)
        XCTAssertNil(PairPayload(raw: raw.replacingOccurrences(of: full, with: "invalid")))
    }
    func testBlake3IncrementalTreeBoundaries() {
        for size in [1,63,64,65,1023,1024,1025,2048,4097,4 * 1024 * 1024 + 1] {
            let data = Data((0..<size).map { UInt8($0 % 251) })
            var incremental = Blake3.Hasher()
            for start in stride(from: 0, to: size, by: 73) { incremental.update(data.subdata(in: start..<min(size, start + 73))) }
            XCTAssertEqual(incremental.hex(), Blake3.hex(data), "length \(size)")
        }
    }
    func testPairPayloadRejectsDuplicateAndNonLocalDestinations() {
        let valid = "homehub://pair?h=hub&t=token&fp=0123456789abcdef&a=192.168.1.2:47802"
        XCTAssertNotNil(PairPayload(raw: valid))
        XCTAssertNil(PairPayload(raw: valid + "&t=other"))
        XCTAssertNil(PairPayload(raw: valid.replacingOccurrences(of: "192.168.1.2", with: "8.8.8.8")))
        XCTAssertNil(PairPayload(raw: valid.replacingOccurrences(of: "fp=0123456789abcdef", with: "fp=invalid")))
        XCTAssertNil(Endpoint.parse("home.local@public.example:443", port: 47800))
        XCTAssertNil(Endpoint.parse("192.168.1.2/secret", port: 47800))
        XCTAssertNotNil(Endpoint.parse("[fd12::42]:47802", port: 47802))
    }
    func testQueueStagesBeforeProviderSourceDisappearsAndSurvivesRestart() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: dir) }
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let source = dir.appendingPathComponent("provider.txt")
        let data = Data("keep my original".utf8); try data.write(to: source)
        let queue = try QueueStore(directory: dir.appendingPathComponent("queue"))
        let item = try queue.enqueue(url: source, hubId: "hub")
        XCTAssertEqual(try Data(contentsOf: source), data)
        try FileManager.default.removeItem(at: source)
        let restored = try QueueStore(directory: queue.directory).all()
        XCTAssertEqual(restored.count, 1)
        XCTAssertEqual(try Data(contentsOf: queue.staged(item)), data)
        var done = restored[0]; done.state = .done; try queue.save(done)
        XCTAssertEqual(try queue.all().count, 1)
        XCTAssertTrue(try queue.pending().isEmpty)
    }
    func testCorruptQueueIsReportedInsteadOfDiscarded() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: dir) }
        let queue = try QueueStore(directory: dir)
        try Data("broken".utf8).write(to: dir.appendingPathComponent("broken.json"))
        XCTAssertThrowsError(try queue.all())
    }
    func testCSRSignatureIsVerifiableByPersistentKey() throws {
        let tag = "com.homehub.test." + UUID().uuidString
        defer { IdentityStore.removeUnpairedKey(tag: tag) }
        let key = try IdentityStore.create(tag: tag)
        let pem = try IdentityStore.csr(key: IdentityStore.key(tag: tag))
        let base64 = pem.components(separatedBy: .newlines).filter { !$0.hasPrefix("-----") }.joined()
        let bytes = Array(try XCTUnwrap(Data(base64Encoded: base64)))
        func field(_ offset: Int) throws -> (Range<Int>, Range<Int>) {
            guard offset + 2 <= bytes.count else { throw HubError.security }
            var position = offset + 2, size = Int(bytes[offset + 1])
            if size & 0x80 != 0 {
                let count = size & 0x7f; guard count > 0, count <= 4, position + count <= bytes.count else { throw HubError.security }
                size = 0
                for byte in bytes[position..<position + count] { size = (size << 8) | Int(byte) }
                position += count
            }
            guard position + size <= bytes.count else { throw HubError.security }
            return (offset..<position + size, position..<position + size)
        }
        let root = try field(0), info = try field(root.1.lowerBound), algorithm = try field(info.0.upperBound), signature = try field(algorithm.0.upperBound)
        XCTAssertEqual(bytes[signature.1.lowerBound], 0)
        let publicKey = try XCTUnwrap(SecKeyCopyPublicKey(key))
        XCTAssertTrue(SecKeyVerifySignature(publicKey, .ecdsaSignatureMessageX962SHA256,
            Data(bytes[info.0]) as CFData, Data(bytes[(signature.1.lowerBound + 1)..<signature.1.upperBound]) as CFData, nil))
    }
}
