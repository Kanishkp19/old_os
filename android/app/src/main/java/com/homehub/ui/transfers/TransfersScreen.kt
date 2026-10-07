package com.homehub.ui.transfers

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.homehub.R
import com.homehub.queue.QueueItem
import com.homehub.queue.QueueRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class TransfersViewModel @Inject constructor(
    private val queue: QueueRepository,
) : ViewModel() {
    val items: StateFlow<List<QueueItem>> = queue.observeQueue()
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun retry(id: String) = viewModelScope.launch { queue.retry(id) }
    fun remove(id: String) = viewModelScope.launch { queue.remove(id) }
}

@Composable
fun TransfersScreen(vm: TransfersViewModel = hiltViewModel()) {
    val items by vm.items.collectAsState()
    Column(Modifier.fillMaxSize().padding(16.dp)) {
        Text(stringResource(R.string.transfers_title), style = MaterialTheme.typography.headlineSmall)
        Spacer(Modifier.height(12.dp))
        if (items.isEmpty()) {
            Text(stringResource(R.string.transfers_empty), color = MaterialTheme.colorScheme.onSurfaceVariant)
        } else {
            LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(items, key = { it.id }) { item ->
                    TransferRow(item, onRetry = { vm.retry(item.id) }, onRemove = { vm.remove(item.id) })
                }
            }
        }
    }
}

@Composable
private fun TransferRow(item: QueueItem, onRetry: () -> Unit, onRemove: () -> Unit) {
    Card(Modifier.fillMaxWidth()) {
        Row(
            Modifier.padding(12.dp).fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(item.name, style = MaterialTheme.typography.bodyLarge, maxLines = 1)
                Text(
                    stateLabel(item),
                    style = MaterialTheme.typography.bodySmall,
                    color = when (item.state) {
                        QueueItem.FAILED_PERM -> MaterialTheme.colorScheme.error
                        QueueItem.DONE -> MaterialTheme.colorScheme.primary
                        else -> MaterialTheme.colorScheme.onSurfaceVariant
                    },
                )
            }
            when (item.state) {
                QueueItem.FAILED_RETRY, QueueItem.FAILED_PERM -> TextButton(onClick = onRetry) {
                    Text(stringResource(R.string.action_retry))
                }
                QueueItem.DONE -> TextButton(onClick = onRemove) {
                    Text(stringResource(R.string.action_clear))
                }
            }
        }
    }
}

@Composable
private fun stateLabel(item: QueueItem): String = when (item.state) {
    QueueItem.QUEUED -> stringResource(R.string.state_queued)
    QueueItem.CONNECTING -> stringResource(R.string.state_connecting)
    QueueItem.UPLOADING -> stringResource(R.string.state_uploading)
    QueueItem.VERIFYING -> stringResource(R.string.state_verifying)
    QueueItem.DONE -> stringResource(R.string.state_done)
    QueueItem.FAILED_RETRY -> {
        val base = stringResource(R.string.state_waiting_retry)
        if (!item.lastError.isNullOrBlank()) "$base: ${item.lastError}" else base
    }
    else -> stringResource(R.string.state_failed, item.lastError ?: "")
}
