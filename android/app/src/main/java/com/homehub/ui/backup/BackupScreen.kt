package com.homehub.ui.backup

import android.Manifest
import android.app.Activity
import android.content.Context
import android.os.Build
import android.provider.MediaStore
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.IntentSenderRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.homehub.R
import com.homehub.backup.BackupRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class BackupViewModel @Inject constructor(val backup: BackupRepository) : ViewModel() {
    val error = MutableStateFlow<String?>(null)
    val busy = MutableStateFlow(false)
    fun action(block: suspend () -> Unit) = viewModelScope.launch {
        busy.value = true
        try { block(); error.value = null }
        catch (e: kotlinx.coroutines.CancellationException) { throw e }
        catch (e: Exception) { error.value = e.message }
        finally { busy.value = false }
    }
}
@Composable
fun BackupScreen(settingsOnly: Boolean = false, vm: BackupViewModel = hiltViewModel()) {
    val context = LocalContext.current
    val rules by vm.backup.settings.collectAsState(); val error by vm.error.collectAsState(); val status by vm.backup.status.collectAsState(); val busy by vm.busy.collectAsState()
    var approve by remember { mutableStateOf(false) }
    var cleanupReview by remember { mutableStateOf(false) }
    var reviewedUris by remember { mutableStateOf<List<android.net.Uri>>(emptyList()) }
    val permissions = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { result ->
        if (result.values.all { it }) vm.action { vm.backup.approve(); vm.backup.runNow() }
        else vm.error.value = context.getString(R.string.backup_permission_required)
    }
    val systemDelete = rememberLauncherForActivityResult(ActivityResultContracts.StartIntentSenderForResult()) { result ->
        vm.action { vm.backup.finishCleanup(result.resultCode == Activity.RESULT_OK) }
        reviewedUris = emptyList()
    }
    val notificationPermission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
    Column(Modifier.fillMaxSize().padding(20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(stringResource(if (settingsOnly) R.string.nav_settings else R.string.nav_backup), style = MaterialTheme.typography.headlineSmall)
        Text(stringResource(R.string.privacy_lan))
        if (Build.VERSION.SDK_INT >= 33) TextButton(onClick = { notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS) }) {
            Text(stringResource(R.string.notification_permission))
        }
        Text(stringResource(R.string.backup_scope))
        error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        status?.let { Text(it) }
        if (busy) LinearProgressIndicator(Modifier.fillMaxWidth())
        if (!rules.approved) Button(onClick = { approve = true }, enabled = !busy) { Text(stringResource(R.string.backup_approve)) }
        else {
            RuleSwitch(stringResource(R.string.backup_enabled), rules.enabled) { enabled -> vm.action { vm.backup.updateRules(enabled, rules.wifiOnly, rules.chargingOnly, rules.intervalHours) } }
            RuleSwitch(stringResource(R.string.backup_wifi), rules.wifiOnly) { wifi -> vm.action { vm.backup.updateRules(rules.enabled, wifi, rules.chargingOnly, rules.intervalHours) } }
            RuleSwitch(stringResource(R.string.backup_charging), rules.chargingOnly) { charging -> vm.action { vm.backup.updateRules(rules.enabled, rules.wifiOnly, charging, rules.intervalHours) } }
            TextButton(onClick = { vm.action {
                val values = listOf(1L, 6L, 12L, 24L)
                vm.backup.updateRules(rules.enabled, rules.wifiOnly, rules.chargingOnly, values[(values.indexOf(rules.intervalHours) + 1) % values.size])
            } }) { Text(stringResource(R.string.backup_interval, rules.intervalHours)) }
            Button(onClick = { vm.backup.runNow() }, enabled = rules.enabled && !busy) { Text(stringResource(R.string.backup_now)) }
            if (rules.lastRun > 0) Text(stringResource(R.string.backup_last_run, java.text.DateFormat.getDateTimeInstance().format(java.util.Date(rules.lastRun))))
            if (rules.pendingCleanup) {
                Text(stringResource(R.string.cleanup_pending))
                TextButton(onClick = { vm.action { vm.backup.finishCleanup(true) } }, enabled = !busy) { Text(stringResource(R.string.cleanup_resolve)) }
            } else TextButton(onClick = { vm.action {
                reviewedUris = vm.backup.prepareCleanup()
                if (reviewedUris.isEmpty()) { vm.backup.finishCleanup(false); vm.error.value = context.getString(R.string.cleanup_empty) }
                else cleanupReview = true
            } }, enabled = !busy && Build.VERSION.SDK_INT >= 30) { Text(stringResource(R.string.cleanup_review)) }
        }
        if (Build.VERSION.SDK_INT < 30) Text(stringResource(R.string.cleanup_old_android))
    }
    if (approve) AlertDialog(onDismissRequest = { approve = false }, title = { Text(stringResource(R.string.backup_approve)) },
        text = { Text(stringResource(R.string.backup_consent)) }, confirmButton = { TextButton(onClick = {
            approve = false
            val media = if (Build.VERSION.SDK_INT >= 33) arrayOf(Manifest.permission.READ_MEDIA_IMAGES, Manifest.permission.READ_MEDIA_VIDEO)
                else arrayOf(Manifest.permission.READ_EXTERNAL_STORAGE)
            permissions.launch(media)
        }) { Text(stringResource(R.string.backup_approve)) } }, dismissButton = { TextButton(onClick = { approve = false }) { Text(stringResource(R.string.action_cancel)) } })
    if (cleanupReview) AlertDialog(onDismissRequest = { cleanupReview = false; vm.action { vm.backup.finishCleanup(false) } },
        title = { Text(stringResource(R.string.cleanup_verified, reviewedUris.size)) }, text = { Text(stringResource(R.string.cleanup_consent)) },
        confirmButton = { TextButton(onClick = {
            cleanupReview = false
            if (Build.VERSION.SDK_INT >= 30) {
                try {
                    val request = MediaStore.createDeleteRequest(context.contentResolver, reviewedUris)
                    systemDelete.launch(IntentSenderRequest.Builder(request.intentSender).build())
                } catch (e: Exception) { vm.action { vm.backup.finishCleanup(false) }; vm.error.value = e.message }
            }
        }) { Text(stringResource(R.string.cleanup_continue)) } },
        dismissButton = { TextButton(onClick = { cleanupReview = false; vm.action { vm.backup.finishCleanup(false) } }) { Text(stringResource(R.string.action_cancel)) } })
}
@Composable
private fun RuleSwitch(label: String, checked: Boolean, change: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) { Text(label); Switch(checked, change) }
}
