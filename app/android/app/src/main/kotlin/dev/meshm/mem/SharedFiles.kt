package dev.meshm.mem

import android.content.ContentResolver
import android.content.Intent
import android.database.Cursor
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import java.io.File

// Files the user shares with contacts stay where they are: MEM keeps a persisted read
// permission for each picked document (or the picked folder) and reads a file only when a
// contact asks for it.
object SharedFiles {
    // The most files one share holds; deeper or larger folders are cut here.
    private const val MAX_ITEMS = 512
    private const val MAX_DEPTH = 4

    // Keeps read access to the documents picked with ACTION_OPEN_DOCUMENT and describes
    // them.
    fun describePicked(resolver: ContentResolver, data: Intent): List<Map<String, Any?>> {
        val uris = buildList {
            val clip = data.clipData
            if (clip != null) {
                for (i in 0 until clip.itemCount) add(clip.getItemAt(i).uri)
            } else {
                data.data?.let(::add)
            }
        }
        return uris.take(MAX_ITEMS).mapNotNull { uri ->
            try {
                resolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
            } catch (e: SecurityException) {
                // Not persistable (some providers): it still works until the app restarts.
            }
            describe(resolver, uri)
        }
    }

    // Keeps read access to a folder picked with ACTION_OPEN_DOCUMENT_TREE and lists its files,
    // subfolders included, up to the limits above.
    fun describeTree(resolver: ContentResolver, tree: Uri): Map<String, Any?> {
        resolver.takePersistableUriPermission(tree, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        val rootId = DocumentsContract.getTreeDocumentId(tree)
        val files = mutableListOf<Map<String, Any?>>()
        walk(resolver, tree, rootId, 0, files)
        val root = DocumentsContract.buildDocumentUriUsingTree(tree, rootId)
        val name = query(resolver, root, DocumentsContract.Document.COLUMN_DISPLAY_NAME) {
            it.getString(0)
        }
        return mapOf("name" to name, "files" to files)
    }

    private fun walk(
        resolver: ContentResolver,
        tree: Uri,
        parentId: String,
        depth: Int,
        out: MutableList<Map<String, Any?>>,
    ) {
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parentId)
        val cols = arrayOf(
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
            DocumentsContract.Document.COLUMN_SIZE,
            DocumentsContract.Document.COLUMN_LAST_MODIFIED,
        )
        val folders = mutableListOf<String>()
        resolver.query(children, cols, null, null, null)?.use { c ->
            while (c.moveToNext() && out.size < MAX_ITEMS) {
                val id = c.getString(0) ?: continue
                val mime = c.getString(2) ?: "application/octet-stream"
                if (mime == DocumentsContract.Document.MIME_TYPE_DIR) {
                    folders.add(id)
                    continue
                }
                out.add(
                    mapOf(
                        "uri" to DocumentsContract.buildDocumentUriUsingTree(tree, id).toString(),
                        "name" to (c.getString(1) ?: "file"),
                        "mime" to mime,
                        "size" to if (c.isNull(3)) 0L else c.getLong(3),
                        "modified" to if (c.isNull(4)) null else c.getLong(4) / 1000,
                    ),
                )
            }
        }
        if (depth + 1 >= MAX_DEPTH) return
        for (f in folders) {
            if (out.size >= MAX_ITEMS) return
            walk(resolver, tree, f, depth + 1, out)
        }
    }

    private fun describe(resolver: ContentResolver, uri: Uri): Map<String, Any?>? {
        val cols = arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE)
        val (name, size) = resolver.query(uri, cols, null, null, null)?.use { c ->
            if (!c.moveToFirst()) return null
            Pair(c.getString(0) ?: "file", if (c.isNull(1)) 0L else c.getLong(1))
        } ?: return null
        val modified = query(resolver, uri, DocumentsContract.Document.COLUMN_LAST_MODIFIED) {
            if (it.isNull(0)) null else it.getLong(0) / 1000
        }
        return mapOf(
            "uri" to uri.toString(),
            "name" to name,
            "mime" to (resolver.getType(uri) ?: "application/octet-stream"),
            "size" to size,
            "modified" to modified,
        )
    }

    private fun <T> query(resolver: ContentResolver, uri: Uri, col: String, read: (Cursor) -> T?): T? =
        try {
            resolver.query(uri, arrayOf(col), null, null, null)?.use { c ->
                if (c.moveToFirst()) read(c) else null
            }
        } catch (e: Exception) {
            null
        }

    // Copies a shared document to `path` (MEM's own cache) for sealing; at most `limit`
    // bytes. Returns the size, or null when the file went away, lost its permission or is
    // too large.
    fun copy(resolver: ContentResolver, uri: Uri, path: String, limit: Long): Long? {
        val dest = File(path)
        return try {
            val total = resolver.openInputStream(uri)?.use { input ->
                dest.outputStream().use { out ->
                    val buf = ByteArray(64 * 1024)
                    var total = 0L
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        total += n
                        if (total > limit) return@use null
                        out.write(buf, 0, n)
                    }
                    total
                }
            }
            if (total == null) dest.delete()
            total
        } catch (e: Exception) {
            dest.delete()
            null
        }
    }
}
