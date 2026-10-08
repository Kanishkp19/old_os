package com.homehub.net

/** A resumed response must continue the same object at the exact local offset. */
object DownloadIntegrity {
    fun rangeStart(header: String?, total: Long): Long {
        val match = Regex("bytes ([0-9]+)-([0-9]+)/([0-9]+)").matchEntire(header.orEmpty())
            ?: throw java.io.IOException("Invalid content range")
        val start = match.groupValues[1].toLong()
        val end = match.groupValues[2].toLong()
        require(start <= end && end < total && match.groupValues[3].toLong() == total)
        return start
    }
}
