package com.homehub.queue

import android.content.Context
import android.net.Uri
import android.provider.MediaStore
import android.provider.OpenableColumns
import com.homehub.net.Blake3
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import java.security.MessageDigest

/** Source generation identity is distinct from the persistent queue row ID. */
data class SourceMeta(val name: String, val size: Long, val mime: String?, val modified: Long?, val takenAt: Long?)
object SourceReader {
    fun meta(context: Context, uri: Uri): SourceMeta {
        if (uri.scheme == "file") {
            val file = java.io.File(requireNotNull(uri.path))
            require(file.isFile) { "This file is no longer available" }
            return SourceMeta(file.name, file.length(), null, file.lastModified(), null)
        }
        var name = "file"; var size = -1L; var modified: Long? = null; var taken: Long? = null
        context.contentResolver.query(uri, null, null, null, null)?.use { c ->
            require(c.moveToFirst()) { "This file is no longer available" }
            fun value(column: String): Long? = c.getColumnIndex(column).takeIf { it >= 0 && !c.isNull(it) }?.let(c::getLong)
            c.getColumnIndex(OpenableColumns.DISPLAY_NAME).takeIf { it >= 0 }?.let { name = c.getString(it) ?: name }
            size = value(OpenableColumns.SIZE) ?: -1
            modified = value(MediaStore.MediaColumns.DATE_MODIFIED)?.times(1000)
            taken = value(MediaStore.Images.ImageColumns.DATE_TAKEN)
        }
        if (size < 0) size = context.contentResolver.openFileDescriptor(uri, "r")?.use { it.statSize } ?: -1
        return SourceMeta(name, size, context.contentResolver.getType(uri), modified, taken)
    }
    fun identity(uri: Uri, meta: SourceMeta): String = MessageDigest.getInstance("SHA-256")
        .digest("${uri}|${meta.size}|${meta.modified ?: "unknown"}".toByteArray())
        .joinToString("") { "%02x".format(it) }
    suspend fun hash(context: Context, uri: Uri): Pair<String, Long> {
        val hasher = Blake3.Hasher(); var size = 0L
        context.contentResolver.openInputStream(uri)?.use { input ->
            val buffer = ByteArray(256 * 1024)
            while (true) {
                currentCoroutineContext().ensureActive()
                val n = input.read(buffer)
                if (n < 0) break
                if (n > 0) { hasher.update(buffer, 0, n); size += n }
            }
        } ?: error("This file is no longer available")
        return hasher.digestHex() to size
    }
}
