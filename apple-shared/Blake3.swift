import Foundation

// Dependency-free unkeyed BLAKE3, incremental whole-file and chunk hashing.
// Uses wrapping UInt32 arithmetic; memory stays bounded to one 1 KiB block
// and a logarithmic CV stack. Wire digest is 32 little-endian output bytes.
enum Blake3 {
    private static let iv: [UInt32] = [0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19]
    private static let permutation = [2,6,3,10,7,0,4,13,1,11,12,5,9,14,15,8]
    private static func rotate(_ x: UInt32, _ n: UInt32) -> UInt32 { (x >> n) | (x << (32 - n)) }
    private static func g(_ s: inout [UInt32], _ a: Int, _ b: Int, _ c: Int, _ d: Int, _ x: UInt32, _ y: UInt32) {
        s[a] = s[a] &+ s[b] &+ x; s[d] = rotate(s[d] ^ s[a], 16)
        s[c] = s[c] &+ s[d]; s[b] = rotate(s[b] ^ s[c], 12)
        s[a] = s[a] &+ s[b] &+ y; s[d] = rotate(s[d] ^ s[a], 8)
        s[c] = s[c] &+ s[d]; s[b] = rotate(s[b] ^ s[c], 7)
    }
    private static func compress(_ cv: [UInt32], _ block: [UInt32], _ counter: UInt64, _ length: UInt32, _ flags: UInt32) -> [UInt32] {
        var s = cv + Array(iv.prefix(4)) + [UInt32(truncatingIfNeeded: counter), UInt32(truncatingIfNeeded: counter >> 32), length, flags]
        var m = block
        for round in 0..<7 {
            g(&s,0,4,8,12,m[0],m[1]); g(&s,1,5,9,13,m[2],m[3]); g(&s,2,6,10,14,m[4],m[5]); g(&s,3,7,11,15,m[6],m[7])
            g(&s,0,5,10,15,m[8],m[9]); g(&s,1,6,11,12,m[10],m[11]); g(&s,2,7,8,13,m[12],m[13]); g(&s,3,4,9,14,m[14],m[15])
            if round < 6 { m = permutation.map { m[$0] } }
        }
        return (0..<8).map { s[$0] ^ s[$0 + 8] } + (0..<8).map { s[$0 + 8] ^ cv[$0] }
    }
    private static func words(_ bytes: [UInt8]) -> [UInt32] {
        var result = [UInt32](repeating: 0, count: 16)
        for (i, b) in bytes.enumerated() { result[i / 4] |= UInt32(b) << UInt32(8 * (i % 4)) }
        return result
    }
    private struct Output {
        let cv: [UInt32], block: [UInt32], counter: UInt64, length: UInt32, flags: UInt32
        func chain() -> [UInt32] { Array(compress(cv, block, counter, length, flags).prefix(8)) }
        func digest() -> Data {
            let w = compress(cv, block, 0, length, flags | 8)
            return Data(w.prefix(8).flatMap { word in (0..<4).map { UInt8(truncatingIfNeeded: word >> UInt32(8 * $0)) } })
        }
    }
    private static func parent(_ left: [UInt32], _ right: [UInt32]) -> Output { Output(cv: iv, block: left + right, counter: 0, length: 64, flags: 4) }
    struct Hasher {
        private var cv = iv, stack = [[UInt32]](), block = [UInt8](), blocks = 0
        private var counter: UInt64 = 0
        private var output: Output { Output(cv: cv, block: words(block), counter: counter, length: UInt32(block.count), flags: (blocks == 0 ? 1 : 0) | 2) }
        mutating func update(_ data: Data) {
            for byte in data {
                if blocks * 64 + block.count == 1024 {
                    var chain = output.chain(), total = counter + 1
                    while total & 1 == 0 { chain = parent(stack.removeLast(), chain).chain(); total >>= 1 }
                    stack.append(chain); counter += 1; cv = iv; block.removeAll(keepingCapacity: true); blocks = 0
                }
                if block.count == 64 {
                    cv = Array(compress(cv, words(block), counter, 64, blocks == 0 ? 1 : 0).prefix(8)); blocks += 1
                    block.removeAll(keepingCapacity: true)
                }
                block.append(byte)
            }
        }
        func digest() -> Data {
            var result = output
            for left in stack.reversed() { result = parent(left, result.chain()) }
            return result.digest()
        }
        func hex() -> String { digest().map { String(format: "%02x", $0) }.joined() }
    }
    static func hex(_ data: Data) -> String { var h = Hasher(); h.update(data); return h.hex() }
    static func fileHex(_ url: URL) throws -> String {
        let file = try FileHandle(forReadingFrom: url); defer { try? file.close() }
        var h = Hasher()
        while let bytes = try file.read(upToCount: 1024 * 1024), !bytes.isEmpty { try Task.checkCancellation(); h.update(bytes) }
        return h.hex()
    }
}
