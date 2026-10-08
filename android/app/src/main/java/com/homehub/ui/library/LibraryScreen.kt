package com.homehub.ui.library

import android.graphics.BitmapFactory
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.grid.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.homehub.R
import com.homehub.net.HubClient
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import javax.inject.Inject

@HiltViewModel
class LibraryViewModel @Inject constructor(private val hub: HubClient,
    @ApplicationContext private val context: Context) : ViewModel() {
    val rows = MutableStateFlow<List<JSONObject>>(emptyList())
    val error = MutableStateFlow<String?>(null)
    val loading = MutableStateFlow(false)
    var photos = false; var trash = false; var query = ""; var category = ""; var sort = "date"
    private var cursor: String? = null
    private var request: kotlinx.coroutines.Job? = null
    fun refresh(more: Boolean = false) {
        request?.cancel()
        request = viewModelScope.launch {
            loading.value = true
            try {
                val path = if (photos) "/v1/photos/timeline?limit=100" else if (trash) "/v1/trash?limit=100" else "/v1/files?limit=100"
                val page = hub.get(path + (if (more && cursor != null) "&cursor=${Uri.encode(cursor)}" else "") +
                    (if (!photos && !trash) "&q=${Uri.encode(query)}&category=${Uri.encode(category)}" else ""))
                val array = page.getJSONArray("items")
                val new = (0 until array.length()).map(array::getJSONObject)
                cursor = page.optString("next_cursor").takeIf { it.isNotBlank() && it != "null" }
                rows.value = ((if (more) rows.value else emptyList()) + new).distinctBy { id(it) }
                order(); error.value = null
            } catch (e: kotlinx.coroutines.CancellationException) { throw e }
            catch (e: Exception) { error.value = e.message }
            finally { loading.value = false }
        }
    }
    fun order() {
        rows.value = when (sort) {
            "name" -> rows.value.sortedWith(compareBy<JSONObject> { it.getString("name").lowercase() }.thenBy { id(it) })
            "size" -> rows.value.sortedWith(compareByDescending<JSONObject> { it.optLong("size") }.thenBy { id(it) })
            else -> rows.value.sortedWith(compareByDescending<JSONObject> { it.optLong(if (photos) "taken_at" else "created_at") }.thenByDescending { id(it) })
        }
    }
    fun change(row: JSONObject, action: String, name: String? = null) = viewModelScope.launch {
        try {
            val id = id(row)
            when (action) {
                "rename" -> hub.patch("/v1/files/$id", JSONObject().put("name", name))
                "trash" -> hub.delete("/v1/files/$id")
                "restore" -> hub.post("/v1/trash/$id/restore")
            }
            refresh()
        } catch (e: Exception) { error.value = e.message }
    }
    fun download(row: JSONObject, destination: Uri) = viewModelScope.launch(Dispatchers.IO) {
        val temp = File(context.cacheDir, "download-${java.util.UUID.randomUUID()}.part")
        try {
            hub.download(id(row), temp)
            context.contentResolver.openOutputStream(destination, "w")?.use { output -> temp.inputStream().use { it.copyTo(output) } }
                ?: error("Unable to save this file")
            error.value = null
        } catch (e: Exception) { error.value = e.message }
        finally { temp.delete() }
    }
    suspend fun thumbnail(row: JSONObject) = withContext(Dispatchers.IO) {
        val data = hub.bytes("/v1/photos/${id(row)}/thumb?size=256")
        BitmapFactory.decodeByteArray(data, 0, data.size)
    }
    fun id(row: JSONObject) = row.optString("file_id").ifBlank { row.getString("id") }
}

@Composable
fun LibraryScreen(photos: Boolean = false, vm: LibraryViewModel = hiltViewModel()) {
    val rows by vm.rows.collectAsState(); val error by vm.error.collectAsState(); val loading by vm.loading.collectAsState()
    var search by remember { mutableStateOf("") }; var selected by remember { mutableStateOf<JSONObject?>(null) }
    var rename by remember { mutableStateOf<JSONObject?>(null) }; var newName by remember { mutableStateOf("") }
    var trashConfirm by remember { mutableStateOf<JSONObject?>(null) }
    var showTrash by remember { mutableStateOf(false) }; var sort by remember { mutableStateOf("date") }
    var downloadRow by remember { mutableStateOf<JSONObject?>(null) }
    val save = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri ->
        val row = downloadRow
        if (uri != null && row != null) vm.download(row, uri)
        downloadRow = null
    }
    LaunchedEffect(photos) { vm.photos = photos; vm.refresh() }
    Column(Modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(stringResource(if (photos) R.string.nav_photos else R.string.nav_files), style = MaterialTheme.typography.headlineSmall)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TextButton(onClick = { vm.refresh() }) { Text(stringResource(R.string.action_refresh)) }
            if (!photos) TextButton(onClick = { showTrash = !showTrash; vm.trash = showTrash; vm.refresh() }) {
                Text(stringResource(if (showTrash) R.string.nav_files else R.string.action_trash))
            }
            TextButton(onClick = { sort = if (sort == "date") "name" else if (sort == "name" && !photos) "size" else "date"; vm.sort = sort; vm.order() }) {
                Text(stringResource(when (sort) { "name" -> R.string.sort_name; "size" -> R.string.sort_size; else -> R.string.sort_date }))
            }
        }
        if (!photos && !showTrash) {
            OutlinedTextField(search, { search = it }, label = { Text(stringResource(R.string.action_search)) }, modifier = Modifier.fillMaxWidth())
            Row {
                TextButton(onClick = { vm.query = search; vm.refresh() }) { Text(stringResource(R.string.action_search)) }
                var category by remember { mutableStateOf("") }
                TextButton(onClick = {
                    val categories = listOf("", "photo", "video", "document", "music", "backup", "download")
                    category = categories[(categories.indexOf(category) + 1) % categories.size]; vm.category = category; vm.refresh()
                }) { Text(stringResource(when(category) { "photo" -> R.string.nav_photos; "video" -> R.string.category_video; "document" -> R.string.category_document;
                    "music" -> R.string.category_music; "backup" -> R.string.nav_backup; "download" -> R.string.category_download; else -> R.string.category_all })) }
            }
        }
        error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        if (loading) LinearProgressIndicator(Modifier.fillMaxWidth())
        if (rows.isEmpty() && !loading) Text(stringResource(if (photos) R.string.photos_empty else R.string.files_empty))
        if (photos) {
            LazyVerticalGrid(columns = GridCells.Adaptive(120.dp), modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(rows, key = { vm.id(it) }) { row ->
                    Card(Modifier.clickable { selected = row }) {
                        PhotoThumb(row, vm)
                        Text(row.getString("name"), maxLines = 1, modifier = Modifier.padding(6.dp))
                        Text(java.text.DateFormat.getDateInstance().format(java.util.Date(row.optLong("taken_at"))), modifier = Modifier.padding(6.dp), style = MaterialTheme.typography.labelSmall)
                    }
                }
            }
        } else LazyColumn(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(rows, key = { vm.id(it) }) { row ->
                Card(Modifier.fillMaxWidth().clickable { selected = row }) {
                    Column(Modifier.padding(12.dp)) { Text(row.getString("name")); Text(android.text.format.Formatter.formatFileSize(androidx.compose.ui.platform.LocalContext.current, row.optLong("size"))) }
                }
            }
        }
        TextButton(onClick = { vm.refresh(true) }) { Text(stringResource(R.string.action_more)) }
    }
    selected?.let { row ->
        AlertDialog(onDismissRequest = { selected = null }, title = { Text(row.getString("name")) },
            text = { Column {
                if (photos) PhotoThumb(row, vm)
                TextButton(onClick = { downloadRow = row; save.launch(row.getString("name")); selected = null }) { Text(stringResource(R.string.action_download)) }
                if (showTrash) TextButton(onClick = { vm.change(row, "restore"); selected = null }) { Text(stringResource(R.string.action_restore)) }
                else {
                    TextButton(onClick = { rename = row; newName = row.getString("name"); selected = null }) { Text(stringResource(R.string.action_rename)) }
                    TextButton(onClick = { trashConfirm = row; selected = null }) { Text(stringResource(R.string.action_trash)) }
                }
            } }, confirmButton = { TextButton(onClick = { selected = null }) { Text(stringResource(R.string.action_done)) } })
    }
    rename?.let { row -> AlertDialog(onDismissRequest = { rename = null }, title = { Text(stringResource(R.string.action_rename)) },
        text = { OutlinedTextField(newName, { newName = it }) }, confirmButton = { TextButton(onClick = { vm.change(row, "rename", newName); rename = null }, enabled = newName.isNotBlank()) { Text(stringResource(R.string.action_done)) } },
        dismissButton = { TextButton(onClick = { rename = null }) { Text(stringResource(R.string.action_cancel)) } }) }
    trashConfirm?.let { row -> AlertDialog(onDismissRequest = { trashConfirm = null }, title = { Text(stringResource(R.string.trash_confirm)) },
        text = { Text(row.getString("name")) }, confirmButton = { TextButton(onClick = { vm.change(row, "trash"); trashConfirm = null }) { Text(stringResource(R.string.action_trash)) } },
        dismissButton = { TextButton(onClick = { trashConfirm = null }) { Text(stringResource(R.string.action_cancel)) } }) }
}
@Composable
private fun PhotoThumb(row: JSONObject, vm: LibraryViewModel) {
    val bitmap by produceState<android.graphics.Bitmap?>(null, vm.id(row)) { value = runCatching { vm.thumbnail(row) }.getOrNull() }
    bitmap?.let { Image(it.asImageBitmap(), row.getString("name"), Modifier.fillMaxWidth().height(140.dp)) }
        ?: Box(Modifier.fillMaxWidth().height(140.dp)) { Text(stringResource(R.string.photo_preview_unavailable)) }
}
