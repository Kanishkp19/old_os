package com.homehub.ui.library

import android.graphics.BitmapFactory
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import android.content.Intent
import androidx.core.content.FileProvider
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
import android.widget.VideoView
import javax.inject.Inject

@HiltViewModel
class LibraryViewModel @Inject constructor(private val hub: HubClient,
    @ApplicationContext private val context: Context) : ViewModel() {
    val scopes get() = hub.scopes
    val rows = MutableStateFlow<List<JSONObject>>(emptyList())
    val error = MutableStateFlow<String?>(null)
    val loading = MutableStateFlow(false)
    val hasMore = MutableStateFlow(false)
    val months = MutableStateFlow<List<JSONObject>>(emptyList())
    val destinations = MutableStateFlow<List<JSONObject>>(emptyList())
    val operation = MutableStateFlow<String?>(null)
    val canFiles get() = hub.hasScope("files")
    val canTransfer get() = hub.hasScope("transfer")
    var month: java.time.YearMonth? = null
    var photos = false; var trash = false; var query = ""; var category = ""; var sort = "date"
    private var cursor: String? = null
    private var request: kotlinx.coroutines.Job? = null
    fun refresh(more: Boolean = false) {
        request?.cancel()
        request = viewModelScope.launch {
            loading.value = true
            try {
                val path = if (photos) "/v1/photos/timeline?limit=100" else if (trash) "/v1/trash?limit=100" else "/v1/files?limit=100"
                val pageCursor = if (more) cursor else null
                val page = hub.get(path + (pageCursor?.let { "&cursor=${Uri.encode(it)}" } ?: "") +
                    (if (photos && month != null) "&year=${month!!.year}&month=${month!!.monthValue}" else "") +
                    (if (!photos && !trash) "&sort=${if (sort == "date") "newest" else sort}" +
                        (if (query.isNotBlank()) "&q=${Uri.encode(query)}" else "") +
                        (if (category.isNotBlank()) "&category=${Uri.encode(category)}" else "") else ""))
                val array = page.getJSONArray("items")
                val pageRows = (0 until array.length()).map(array::getJSONObject)
                cursor = page.optString("next_cursor").takeIf { it.isNotBlank() && it != "null" }
                rows.value = ((if (more) rows.value else emptyList()) + pageRows).distinctBy { id(it) }
                hasMore.value = cursor != null
                error.value = null
            } catch (e: kotlinx.coroutines.CancellationException) { throw e }
            catch (e: Exception) { error.value = com.homehub.ui.UserErrors.message(context, e) }
            finally { loading.value = false }
        }
    }
    fun order() {
        refresh()
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
        } catch (e: Exception) { error.value = com.homehub.ui.UserErrors.message(context, e) }
    }
    private fun partial(row: JSONObject): File {
        val fid = id(row); require(fid.matches(Regex("[A-Za-z0-9_-]+")))
        val folder = File(context.cacheDir, "verified-downloads").apply { mkdirs() }
        return File(folder, "$fid.part")
    }
    fun download(row: JSONObject, destination: Uri) = viewModelScope.launch(Dispatchers.IO) {
        val temp = partial(row)
        try {
            operation.value = context.getString(R.string.download_active)
            hub.download(id(row), temp)
            context.contentResolver.openOutputStream(destination, "w")?.use { output -> temp.inputStream().use { it.copyTo(output) } }
                ?: throw java.io.IOException("Unable to save")
            temp.delete(); File(temp.path + ".identity").delete()
            error.value = null; operation.value = context.getString(R.string.download_saved)
        } catch (e: kotlinx.coroutines.CancellationException) { throw e }
        catch (e: Exception) { error.value = com.homehub.ui.UserErrors.message(context, e); operation.value = null }
    }
    fun share(row: JSONObject, open: Boolean = false) = viewModelScope.launch {
        try {
            operation.value = context.getString(R.string.download_active)
            val meta = withContext(Dispatchers.IO) {
                val temp = partial(row)
                val verified = hub.download(id(row), temp)
                val safeName = verified.getString("name").replace(Regex("[^\\p{L}\\p{N}._ -]"), "_").take(160).ifBlank { id(row) }
                val folder = File(temp.parentFile, id(row)).apply { mkdirs() }
                val ready = File(folder, safeName)
                require(!ready.exists() || ready.delete()); require(temp.renameTo(ready))
                File(temp.path + ".identity").delete()
                verified to ready
            }
            val uri = FileProvider.getUriForFile(context, "${context.packageName}.verified", meta.second)
            val intent = if (open) Intent(Intent.ACTION_VIEW).setDataAndType(uri, meta.first.optString("mime").takeIf { it.contains('/') } ?: "application/octet-stream")
                else Intent(Intent.ACTION_SEND).setType(meta.first.optString("mime").takeIf { it.contains('/') } ?: "application/octet-stream")
                    .putExtra(Intent.EXTRA_STREAM, uri)
            intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            intent.clipData = android.content.ClipData.newRawUri("", uri)
            context.startActivity(Intent.createChooser(intent, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            operation.value = null
        } catch (e: kotlinx.coroutines.CancellationException) { throw e }
        catch (e: Exception) { error.value = com.homehub.ui.UserErrors.message(context, e); operation.value = null }
    }
    suspend fun details(row: JSONObject) = hub.get("/v1/files/${id(row)}")
    fun loadMonths() = viewModelScope.launch {
        try {
            val array = org.json.JSONArray(hub.request("GET", "/v1/photos/years"))
            months.value = (0 until array.length()).map(array::getJSONObject)
        } catch (e: Exception) { error.value = com.homehub.ui.UserErrors.message(context, e) }
    }
    fun loadDestinations() = viewModelScope.launch {
        try {
            val own = context.getSharedPreferences("homehub_auth", Context.MODE_PRIVATE).getString("device_id", null)
            val array = hub.get("/v1/relay/devices").getJSONArray("items")
            destinations.value = (0 until array.length()).map(array::getJSONObject).filter {
                it.getString("id") != own
            }
        } catch (e: Exception) { error.value = com.homehub.ui.UserErrors.message(context, e) }
    }
    fun relay(row: JSONObject, target: String) = viewModelScope.launch {
        try {
            hub.post("/v1/files/${id(row)}/relay", JSONObject().put("target_device_id", target))
            operation.value = context.getString(R.string.relay_queued)
        } catch (e: Exception) { error.value = com.homehub.ui.UserErrors.message(context, e) }
    }
    suspend fun thumbnail(row: JSONObject, size: Int = 256) = withContext(Dispatchers.IO) {
        val data = hub.bytes("/v1/photos/${id(row)}/thumb?size=$size")
        BitmapFactory.decodeByteArray(data, 0, data.size)
    }
    suspend fun viewerFile(row: JSONObject): File = withContext(Dispatchers.IO) {
        val folder = File(context.cacheDir, "verified-viewer").apply { mkdirs() }
        val file = File(folder, "${id(row)}.part")
        hub.download(id(row), file)
        file
    }
    fun id(row: JSONObject) = row.optString("file_id").ifBlank { row.getString("id") }
}

@Composable
fun LibraryScreen(photos: Boolean = false, vm: LibraryViewModel = hiltViewModel()) {
    val grantedScopes = vm.scopes.collectAsState().value
    val months by vm.months.collectAsState(); val targets by vm.destinations.collectAsState(); val operation by vm.operation.collectAsState()
    val more by vm.hasMore.collectAsState()
    val rows by vm.rows.collectAsState(); val error by vm.error.collectAsState(); val loading by vm.loading.collectAsState()
    var search by remember { mutableStateOf("") }; var selected by remember { mutableStateOf<JSONObject?>(null) }
    var rename by remember { mutableStateOf<JSONObject?>(null) }; var newName by remember { mutableStateOf("") }
    var trashConfirm by remember { mutableStateOf<JSONObject?>(null) }
    var showTrash by remember { mutableStateOf(false) }; var sort by remember { mutableStateOf("date") }
    var downloadRow by remember { mutableStateOf<JSONObject?>(null) }
    var viewer by remember { mutableStateOf<JSONObject?>(null) }
    var relayRow by remember { mutableStateOf<JSONObject?>(null) }
    var monthMenu by remember { mutableStateOf(false) }
    var selectedMonth by remember { mutableStateOf<java.time.YearMonth?>(null) }
    val save = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri ->
        val row = downloadRow
        if (uri != null && row != null) vm.download(row, uri)
        downloadRow = null
    }
    LaunchedEffect(photos) { vm.photos = photos; vm.refresh(); if (photos) vm.loadMonths() }
    Column(Modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(stringResource(if (photos) R.string.nav_photos else R.string.nav_files), style = MaterialTheme.typography.headlineSmall)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TextButton(onClick = { vm.refresh() }) { Text(stringResource(R.string.action_refresh)) }
            if (!photos) TextButton(onClick = { showTrash = !showTrash; vm.trash = showTrash; vm.refresh() }) {
                Text(stringResource(if (showTrash) R.string.nav_files else R.string.action_trash))
            }
            if (!photos) TextButton(onClick = { sort = if (sort == "date") "name" else if (sort == "name" && !photos) "size" else "date"; vm.sort = sort; vm.order() }) {
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
        if (photos) Box {
            TextButton(onClick = { monthMenu = true }) { Text(selectedMonth?.toString() ?: stringResource(R.string.gallery_all_months)) }
            DropdownMenu(monthMenu, onDismissRequest = { monthMenu = false }) {
                DropdownMenuItem(text = { Text(stringResource(R.string.gallery_all_months)) }, onClick = {
                    selectedMonth = null; vm.month = null; monthMenu = false; vm.refresh()
                })
                months.forEach { value ->
                    val date = java.time.YearMonth.of(value.getInt("year"), value.getInt("month"))
                    DropdownMenuItem(text = { Text(stringResource(R.string.gallery_month,
                        date.format(java.time.format.DateTimeFormatter.ofPattern("MMMM yyyy")), value.getLong("count"))) }, onClick = {
                        selectedMonth = date; vm.month = date; monthMenu = false; vm.refresh()
                    })
                }
            }
        }
        operation?.let { Text(it) }
        error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        if (loading) LinearProgressIndicator(Modifier.fillMaxWidth())
        if (rows.isEmpty() && !loading) Text(stringResource(if (photos) R.string.photos_empty else R.string.files_empty))
        if (photos) {
            LazyVerticalGrid(columns = GridCells.Adaptive(120.dp), modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(rows, key = { vm.id(it) }) { row ->
                    Card(Modifier.clickable { viewer = row }) {
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
        TextButton(onClick = { vm.refresh(true) }, enabled = more && !loading) { Text(stringResource(R.string.action_more)) }
    }
    selected?.let { row ->
        AlertDialog(onDismissRequest = { selected = null }, title = { Text(row.getString("name")) },
            text = { Column {
                FileDetails(row, vm)
                if (!showTrash && vm.canFiles) TextButton(onClick = { downloadRow = row; save.launch(row.getString("name")); selected = null }) { Text(stringResource(R.string.action_download)) }
                if (!showTrash && vm.canFiles) TextButton(onClick = { vm.share(row); selected = null }) { Text(stringResource(R.string.action_share)) }
                if (!showTrash && vm.canTransfer) TextButton(onClick = { relayRow = row; vm.loadDestinations(); selected = null }) { Text(stringResource(R.string.action_relay)) }
                if (showTrash) TextButton(onClick = { vm.change(row, "restore"); selected = null }) { Text(stringResource(R.string.action_restore)) }
                else {
                    TextButton(onClick = { rename = row; newName = row.getString("name"); selected = null }) { Text(stringResource(R.string.action_rename)) }
                    TextButton(onClick = { trashConfirm = row; selected = null }) { Text(stringResource(R.string.action_trash)) }
                }
            } }, confirmButton = { TextButton(onClick = { selected = null }) { Text(stringResource(R.string.action_done)) } })
    }
    viewer?.let { row -> PhotoViewer(row, vm, close = { viewer = null }, save = {
        downloadRow = row; save.launch(row.getString("name"))
    }, relay = { relayRow = row; vm.loadDestinations() }) }
    relayRow?.let { row -> AlertDialog(onDismissRequest = { relayRow = null }, title = { Text(stringResource(R.string.action_relay)) },
        text = { Column { if (targets.isEmpty()) Text(stringResource(R.string.relay_empty))
            targets.forEach { device -> TextButton(onClick = { vm.relay(row, device.getString("id")); relayRow = null }) { Text(device.getString("name")) } }
        } }, confirmButton = { TextButton(onClick = { relayRow = null }) { Text(stringResource(R.string.action_cancel)) } }) }
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

@Composable
private fun FileDetails(row: JSONObject, vm: LibraryViewModel) {
    val info by produceState<JSONObject?>(null, vm.id(row)) { value = runCatching { vm.details(row) }.getOrNull() }
    info?.let { value ->
        Text(stringResource(R.string.file_details, android.text.format.Formatter.formatFileSize(LocalContext.current, value.getLong("size")),
            value.optString("mime").takeIf { it != "null" }.orEmpty()))
        Text(java.text.DateFormat.getDateTimeInstance().format(java.util.Date(value.getLong("created_at"))))
        if (row.has("width") && !row.isNull("width")) Text("${row.optInt("width")} × ${row.optInt("height")}")
        if (row.has("taken_at")) Text(stringResource(R.string.photo_taken,
            java.text.DateFormat.getDateTimeInstance().format(java.util.Date(row.getLong("taken_at")))))
        row.optString("camera_make").takeIf { it.isNotBlank() && it != "null" }?.let { make ->
            Text(stringResource(R.string.photo_camera, listOf(make,row.optString("camera_model"))
                .filter { it.isNotBlank() && it != "null" }.joinToString(" ")))
        }
        row.optLong("duration_ms").takeIf { it > 0 }?.let { Text(stringResource(R.string.photo_duration, it / 1000)) }
    }
}
@Composable
private fun PhotoViewer(row: JSONObject, vm: LibraryViewModel, close: () -> Unit, save: () -> Unit, relay: () -> Unit) {
    val isVideo = row.optString("type_") == "video"
    val file by produceState<File?>(null, vm.id(row)) {
        if (vm.canFiles) value = runCatching { vm.viewerFile(row) }.getOrNull()
    }
    val bitmap by produceState<android.graphics.Bitmap?>(null, vm.id(row), file) {
        value = withContext(Dispatchers.IO) {
            if (isVideo) null else if (file == null) runCatching { vm.thumbnail(row, 1024) }.getOrNull()
            else runCatching {
                val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeFile(file!!.path, bounds)
                var sample = 1
                while (bounds.outWidth / sample > 3072 || bounds.outHeight / sample > 3072) sample *= 2
                BitmapFactory.decodeFile(file!!.path, BitmapFactory.Options().apply { inSampleSize = sample })
            }.getOrNull()
        }
    }
    val context = LocalContext.current
    val player = remember(vm.id(row)) { VideoView(context).apply {
        setMediaController(android.widget.MediaController(context))
    } }
    var playbackError by remember(vm.id(row)) { mutableStateOf(false) }
    LaunchedEffect(file, isVideo) {
        if (isVideo && file != null) {
            player.setOnErrorListener { _, _, _ -> playbackError = true; true }
            player.setOnPreparedListener { player.start() }
            player.setVideoPath(file!!.absolutePath)
        }
    }
    DisposableEffect(player, file) { onDispose {
        player.stopPlayback()
        file?.delete()
        file?.let { File(it.path + ".identity").delete() }
    } }
    var zoom by remember { mutableFloatStateOf(1f) }
    var x by remember { mutableFloatStateOf(0f) }; var y by remember { mutableFloatStateOf(0f) }
    Dialog(onDismissRequest = close, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Surface(Modifier.fillMaxSize()) {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(row.getString("name"), style = MaterialTheme.typography.titleLarge)
                Box(Modifier.weight(1f).fillMaxWidth().then(if (isVideo) Modifier else Modifier.pointerInput(Unit) {
                    detectTransformGestures { _, pan, scale, _ -> zoom = (zoom * scale).coerceIn(1f, 6f); x += pan.x; y += pan.y }
                })) {
                    if (isVideo && file != null && !playbackError) AndroidView(factory = { player }, modifier = Modifier.fillMaxSize())
                    else if (isVideo) Text(stringResource(R.string.photo_unsupported))
                    else bitmap?.let { Image(it.asImageBitmap(), row.getString("name"), Modifier.fillMaxSize().graphicsLayer {
                        scaleX = zoom; scaleY = zoom; translationX = x; translationY = y
                    }) } ?: Text(stringResource(R.string.photo_unsupported))
                }
                FileDetails(row, vm)
                if (vm.canFiles) Row {
                    TextButton(onClick = save) { Text(stringResource(R.string.action_download)) }
                    TextButton(onClick = { vm.share(row) }) { Text(stringResource(R.string.action_share)) }
                    TextButton(onClick = { vm.share(row, true) }) { Text(stringResource(R.string.photo_original)) }
                }
                if (vm.canTransfer) TextButton(onClick = relay) { Text(stringResource(R.string.action_relay)) }
                TextButton(onClick = close) { Text(stringResource(R.string.action_done)) }
            }
        }
    }
}
