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
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.rememberScrollState
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
class BackupViewModel @Inject constructor(val backup: BackupRepository,
    @dagger.hilt.android.qualifiers.ApplicationContext private val context: Context) : ViewModel() {
    val error = MutableStateFlow<String?>(null)
    val busy = MutableStateFlow(false)
    fun action(block: suspend () -> Unit) = viewModelScope.launch {
        busy.value = true
        try { block(); error.value = null }
        catch (e: kotlinx.coroutines.CancellationException) { throw e }
        catch (e: Exception) { error.value = com.homehub.ui.UserErrors.message(context, e) }
        finally { busy.value = false }
    }
}
@Composable
fun BackupScreen(settingsOnly: Boolean = false, vm: BackupViewModel = hiltViewModel()) {
    val context = LocalContext.current
    val remoteReviews by vm.backup.remoteReviews.collectAsState()
    val rules by vm.backup.settings.collectAsState(); val error by vm.error.collectAsState(); val status by vm.backup.status.collectAsState(); val busy by vm.busy.collectAsState()
    var approve by remember { mutableStateOf(false) }
    var permissionApproval by remember { mutableStateOf(false) }
    var cleanupReview by remember { mutableStateOf(false) }
    var reviewedUris by remember { mutableStateOf<List<android.net.Uri>>(emptyList()) }
    val permissions = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { result ->
        if (com.homehub.backup.MediaAccess.granted(context)) vm.action {
            if (permissionApproval && !vm.backup.settings.value.approved) vm.backup.approve()
            permissionApproval = false
            if (vm.backup.settings.value.approved) vm.backup.runNow()
        }
        else vm.error.value = context.getString(R.string.backup_permission_required)
    }
    val systemDelete = rememberLauncherForActivityResult(ActivityResultContracts.StartIntentSenderForResult()) { result ->
        vm.action { vm.backup.finishCleanup(result.resultCode == Activity.RESULT_OK) }
        reviewedUris = emptyList()
    }
    LaunchedEffect(Unit) { vm.backup.refresh(); if (vm.backup.settings.value.approved) vm.action { vm.backup.refreshRemoteReviews() } }
    val notificationPermission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(stringResource(if (settingsOnly) R.string.nav_settings else R.string.nav_backup), style = MaterialTheme.typography.headlineSmall)
        if (settingsOnly) com.homehub.ui.LanguageSettings()
        Text(stringResource(R.string.privacy_lan))
        if (Build.VERSION.SDK_INT >= 33) TextButton(onClick = { notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS) }) {
            Text(stringResource(R.string.notification_permission))
        }
        Text(stringResource(R.string.backup_scope))
        if (com.homehub.backup.MediaAccess.partial(context)) Text(stringResource(R.string.backup_partial))
        TextButton(onClick = { permissions.launch(com.homehub.backup.MediaAccess.permissions()) }) { Text(stringResource(R.string.backup_permissions)) }
        TextButton(onClick = { context.startActivity(android.content.Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
            android.net.Uri.parse("package:${context.packageName}"))) }) { Text(stringResource(R.string.action_settings)) }
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
            Text(stringResource(R.string.backup_effective_interval, rules.effectiveMinutes))
            Button(onClick = { vm.backup.runNow() }, enabled = rules.enabled && !busy) { Text(stringResource(R.string.backup_now)) }
            if (rules.lastRun > 0) Text(stringResource(R.string.backup_last_run, java.text.DateFormat.getDateTimeInstance().format(java.util.Date(rules.lastRun))))
            if (rules.pendingCleanup || remoteReviews.isNotEmpty()) {
                Text(stringResource(R.string.cleanup_pending))
                TextButton(onClick = { vm.action { vm.backup.recoverReview(); vm.backup.releaseOrphanReviews() } }, enabled = !busy) { Text(stringResource(R.string.cleanup_resolve)) }
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
            permissionApproval = true
            permissions.launch(com.homehub.backup.MediaAccess.permissions())
        }) { Text(stringResource(R.string.backup_approve)) } }, dismissButton = { TextButton(onClick = { approve = false }) { Text(stringResource(R.string.action_cancel)) } })
    if (cleanupReview) AlertDialog(onDismissRequest = { cleanupReview = false; vm.action { vm.backup.finishCleanup(false) } },
        title = { Text(stringResource(R.string.cleanup_verified, reviewedUris.size)) }, text = { Text(stringResource(R.string.cleanup_consent)) },
        confirmButton = { TextButton(onClick = {
            cleanupReview = false
            if (Build.VERSION.SDK_INT >= 30) {
                vm.action {
                    try {
                        val fresh = vm.backup.validateCleanup()
                        val request = MediaStore.createDeleteRequest(context.contentResolver, fresh)
                        systemDelete.launch(IntentSenderRequest.Builder(request.intentSender).build())
                    } catch (e: Exception) { vm.backup.finishCleanup(false); throw e }
                }
            }
        }) { Text(stringResource(R.string.cleanup_continue)) } },
        dismissButton = { TextButton(onClick = { cleanupReview = false; vm.action { vm.backup.finishCleanup(false) } }) { Text(stringResource(R.string.action_cancel)) } })
}
@Composable
private fun RuleSwitch(label: String, checked: Boolean, change: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) { Text(label); Switch(checked, change) }
}
