package com.homehub.screen

import java.net.InetAddress

/** Do not resolve names or accept external/multicast ICE endpoints. No STUN or TURN. */
object LanSdp {
    fun privateAddress(raw: String): Boolean {
        if (!raw.matches(Regex("[0-9a-fA-F:.]+"))) return false
        val address = runCatching { InetAddress.getByName(raw) }.getOrNull() ?: return false
        if (address.isAnyLocalAddress || address.isLoopbackAddress || address.isMulticastAddress) return false
        val bytes = address.address
        return address.isSiteLocalAddress || address.isLinkLocalAddress ||
            (bytes.size == 16 && (bytes[0].toInt() and 0xfe) == 0xfc)
    }
    fun hostOnly(sdp: String): String {
        require(sdp.length <= 128 * 1024)
        var candidates = 0
        val lines = sdp.lineSequence().filter { line ->
            if (!line.startsWith("a=candidate:")) true else {
                val tokens = line.trim().split(Regex("\\s+"))
                val valid = tokens.size >= 8 && tokens[6] == "typ" && tokens[7] == "host" &&
                    privateAddress(tokens[4]) && tokens[5].toIntOrNull()?.let { it in 1..65535 } == true
                if (valid) candidates++
                valid
            }
        }.toList()
        require(candidates > 0) { "No local screen route" }
        return lines.filter { it.isNotEmpty() }.joinToString("\r\n", postfix = "\r\n")
    }
}
