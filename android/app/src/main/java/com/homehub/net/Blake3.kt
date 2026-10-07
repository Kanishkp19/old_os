package com.homehub.net

/**
 * BLAKE3 (single-keyed, hash mode) — pure Kotlin reference implementation
 * used for bring-up and unit tests. Production builds should link the
 * official BLAKE3 JNI bindings (see build.gradle.kts note); both must agree
 * with the hub's `blake3` crate, chunk for chunk and on the tree root.
 *
 * Implements the full Merkle-tree mode so 4 MiB chunking on the hub and the
 * client produce identical root hashes.
 */
object Blake3 {
    private val IV = intArrayOf(0x6A09E667, -0x4498517B, 0x3C6EF372, -0x5AB00AC6, 0x510E527F, -0x64FA9774, 0x1F83D9AB, 0x5BE0CD19)
    private const val BLOCK_LEN = 64
    private const val CHUNK_LEN = 1024
    private const val CHUNK_START = 1
    private const val CHUNK_END = 2
    private const val PARENT = 4
    private const val ROOT = 8

    private val MSG_PERMUTATION = intArrayOf(2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8)

    private fun rotr(x: Int, n: Int) = (x ushr n) or (x shl (32 - n))

    private class Compressor {
        val state = IntArray(16)
        fun g(a: Int, b: Int, c: Int, d: Int, mx: Int, my: Int) {
            state[a] = state[a] + state[b] + mx
            state[d] = rotr(state[d] xor state[a], 16)
            state[c] = state[c] + state[d]
            state[b] = rotr(state[b] xor state[c], 12)
            state[a] = state[a] + state[b] + my
            state[d] = rotr(state[d] xor state[a], 8)
            state[c] = state[c] + state[d]
            state[b] = rotr(state[b] xor state[c], 7)
        }

        fun compress(cv: IntArray, block: IntArray, counter: Long, blockLen: Int, flags: Int): IntArray {
            state[0] = cv[0]; state[1] = cv[1]; state[2] = cv[2]; state[3] = cv[3]
            state[4] = cv[4]; state[5] = cv[5]; state[6] = cv[6]; state[7] = cv[7]
            state[8] = IV[0]; state[9] = IV[1]; state[10] = IV[2]; state[11] = IV[3]
            state[12] = counter.toInt(); state[13] = (counter ushr 32).toInt()
            state[14] = blockLen; state[15] = flags
            var m = block
            repeat(7) { round ->
                g(0, 4, 8, 12, m[0], m[1]); g(1, 5, 9, 13, m[2], m[3])
                g(2, 6, 10, 14, m[4], m[5]); g(3, 7, 11, 15, m[6], m[7])
                g(0, 5, 10, 15, m[8], m[9]); g(1, 6, 11, 12, m[10], m[11])
                g(2, 7, 8, 13, m[12], m[13]); g(3, 4, 9, 14, m[14], m[15])
                if (round < 6) {
                    val permuted = IntArray(16)
                    for (i in 0 until 16) permuted[i] = m[MSG_PERMUTATION[i]]
                    m = permuted
                }
            }
            val out = IntArray(16)
            for (i in 0 until 8) out[i] = state[i] xor state[i + 8]
            for (i in 0 until 8) out[i + 8] = state[i + 8] xor cv[i]
            return out
        }
    }

    private fun bytesToWords(bytes: ByteArray): IntArray {
        val words = IntArray(16)
        for (i in 0 until 16) {
            var w = 0
            for (j in 0 until 4) {
                val idx = i * 4 + j
                if (idx < bytes.size) w = w or ((bytes[idx].toInt() and 0xFF) shl (8 * j))
            }
            words[i] = w
        }
        return words
    }

    private class Output(val inputCv: IntArray, val blockWords: IntArray, val counter: Long, val blockLen: Int, val flags: Int) {
        fun chainingValue(): IntArray =
            Compressor().compress(inputCv, blockWords, counter, blockLen, flags).copyOfRange(0, 8)

        fun rootBytes(): ByteArray {
            val c = Compressor()
            val words = c.compress(inputCv, blockWords, counter, blockLen, flags or ROOT)
            val out = ByteArray(32)
            for (i in 0 until 8) {
                out[i * 4] = (words[i] and 0xFF).toByte()
                out[i * 4 + 1] = (words[i] ushr 8 and 0xFF).toByte()
                out[i * 4 + 2] = (words[i] ushr 16 and 0xFF).toByte()
                out[i * 4 + 3] = (words[i] ushr 24).toByte()
            }
            return out
        }
    }

    private class ChunkState(var chunkCounter: Long) {
        val cv = IV.copyOf()
        var block = ByteArray(BLOCK_LEN)
        var blockLen = 0
        var blocksCompressed = 0
        var flags = CHUNK_START

        fun update(input: ByteArray) {
            var off = 0
            while (off < input.size) {
                if (blockLen == BLOCK_LEN) {
                    val out = Compressor().compress(cv, bytesToWords(block), chunkCounter, BLOCK_LEN, flags).copyOfRange(0, 8)
                    out.copyInto(cv)
                    blocksCompressed++
                    block = ByteArray(BLOCK_LEN)
                    blockLen = 0
                    flags = 0
                }
                val want = BLOCK_LEN - blockLen
                val take = minOf(want, input.size - off)
                input.copyInto(block, blockLen, off, off + take)
                blockLen += take
                off += take
            }
        }

        fun output(): Output = Output(cv, bytesToWords(block), chunkCounter, blockLen, flags or CHUNK_END)
    }

    /** Hash `input` in hash mode; returns the 32-byte tree root. */
    fun hash(input: ByteArray): ByteArray {
        // Fold every non-final chunk into a CV stack of subtree roots
        // (merging while the chunk count is even), then merge the final
        // chunk's Output down the stack so the ROOT flag is applied once.
        val cvStack = ArrayDeque<IntArray>()
        var chunkCounter = 0L
        var off = 0
        var lastOutput: Output? = null
        do {
            val chunk = ChunkState(chunkCounter)
            val end = minOf(off + CHUNK_LEN, input.size)
            if (end > off) chunk.update(input.copyOfRange(off, end))
            lastOutput = chunk.output()
            off = end
            chunkCounter++
            if (off < input.size) {
                var cv = lastOutput!!.chainingValue()
                var total = chunkCounter
                while (total and 1L == 0L) {
                    val left = cvStack.removeLast()
                    cv = parentOutput(left, cv).chainingValue()
                    total = total shr 1
                }
                cvStack.addLast(cv)
            }
        } while (off < input.size)

        var output = lastOutput!!
        while (cvStack.isNotEmpty()) {
            val left = cvStack.removeLast()
            output = parentOutput(left, output.chainingValue())
        }
        return output.rootBytes()
    }

    private fun parentOutput(leftCv: IntArray, rightCv: IntArray): Output {
        val blockWords = IntArray(16)
        leftCv.copyInto(blockWords, 0, 0, 8)
        rightCv.copyInto(blockWords, 8, 0, 8)
        return Output(IV, blockWords, 0, BLOCK_LEN, PARENT)
    }

    fun hashHex(input: ByteArray): String = hash(input).joinToString("") { "%02x".format(it) }

    /**
     * Incremental hasher for large files: feed buffers of any size with
     * [update], finish with [digest]. Produces the same root as [hash].
     */
    class Hasher {
        private val cvStack = ArrayDeque<IntArray>()
        private var chunk = ChunkState(0)
        private var chunkCounter = 0L
        private var finalized = false

        fun update(input: ByteArray, offset: Int = 0, length: Int = input.size - offset) {
            check(!finalized) { "hasher already finalized" }
            var off = offset
            val end = offset + length
            while (off < end) {
                val take = minOf(CHUNK_LEN - chunk.blockLenTotal(), end - off)
                if (take <= 0) { startNextChunk(); continue }
                chunk.update(input.copyOfRange(off, off + take))
                off += take
                if (chunk.blockLenTotal() == CHUNK_LEN && off < end) startNextChunk()
            }
        }

        private fun ChunkState.blockLenTotal(): Int = blocksCompressed * BLOCK_LEN + blockLen

        private fun startNextChunk() {
            var cv = chunk.output().chainingValue()
            var total = chunkCounter + 1
            while (total and 1L == 0L) {
                val left = cvStack.removeLast()
                cv = parentOutput(left, cv).chainingValue()
                total = total shr 1
            }
            cvStack.addLast(cv)
            chunkCounter++
            chunk = ChunkState(chunkCounter)
        }

        fun digest(): ByteArray {
            check(!finalized) { "hasher already finalized" }
            finalized = true
            var output = chunk.output()
            while (cvStack.isNotEmpty()) {
                val left = cvStack.removeLast()
                output = parentOutput(left, output.chainingValue())
            }
            return output.rootBytes()
        }

        fun digestHex(): String = digest().joinToString("") { "%02x".format(it) }
    }
}
