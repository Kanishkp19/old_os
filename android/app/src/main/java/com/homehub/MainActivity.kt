package com.homehub

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.navigation.compose.*
import com.homehub.queue.QueueRepository
import com.homehub.ui.home.HomeScreen
import com.homehub.ui.pair.PairScreen
import com.homehub.ui.transfers.TransfersScreen
import com.homehub.ui.remote.RemoteScreen
import com.homehub.ui.devices.DevicesScreen
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    private val requestedRoute = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)
    @Inject lateinit var queue: QueueRepository
    @Inject lateinit var connectivity: com.homehub.net.HubConnectivity

    override fun attachBaseContext(newBase: android.content.Context) { super.attachBaseContext(com.homehub.ui.LanguagePreference.wrap(newBase)) }
    override fun onResume() { super.onResume(); connectivity.refresh() }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (savedInstanceState == null) handleShareIntent(intent)
        if (intent?.getStringExtra("route") == "transfers") requestedRoute.value = "transfers"
        setContent {
            MaterialTheme {
                val nav = rememberNavController()
                val openRoute by requestedRoute.collectAsState()
                LaunchedEffect(openRoute) { if (openRoute == "transfers") { nav.navigate("transfers") { launchSingleTop = true }; requestedRoute.value = null } }
                val navBackStackEntry by nav.currentBackStackEntryAsState()
                val currentRoute = navBackStackEntry?.destination?.route ?: "home"
                Scaffold(
                    bottomBar = {
                        NavigationBar {
                            listOf("home" to com.homehub.R.string.nav_home,
                                "photos" to com.homehub.R.string.nav_photos, "files" to com.homehub.R.string.nav_files,
                                "remote" to com.homehub.R.string.nav_remote, "more" to com.homehub.R.string.nav_more).forEach { (route, label) ->
                                NavigationBarItem(selected = currentRoute == route,
                                    onClick = { nav.navigate(route) { popUpTo("home") { saveState = true }; launchSingleTop = true; restoreState = true } },
                                    icon = {}, label = { Text(androidx.compose.ui.res.stringResource(label)) })
                            }
                        }
                    }
                ) { padding ->
                    NavHost(nav, startDestination = "home", modifier = Modifier.padding(padding)) {
                        composable("home") { HomeScreen(onPair = { nav.navigate("pair") }, onBackup = { nav.navigate("backup") }) }
                        composable("photos") { com.homehub.ui.library.LibraryScreen(photos = true) }
                        composable("files") { com.homehub.ui.library.LibraryScreen() }
                        composable("backup") { com.homehub.ui.backup.BackupScreen() }
                        composable("settings") { com.homehub.ui.backup.BackupScreen(settingsOnly = true) }
                        composable("more") {
                            androidx.compose.foundation.layout.Column(Modifier.padding(20.dp)) {
                                listOf("backup" to R.string.nav_backup, "transfers" to R.string.nav_transfers,
                                    "devices" to R.string.nav_devices, "settings" to R.string.nav_settings).forEach { (route, label) ->
                                    TextButton(onClick = { nav.navigate(route) }) { Text(androidx.compose.ui.res.stringResource(label)) }
                                }
                            }
                        }
                        composable("pair") { PairScreen(onDone = { nav.popBackStack() }) }
                        composable("transfers") { TransfersScreen() }
                        composable("remote") { RemoteScreen() }
                        composable("devices") { DevicesScreen() }
                    }
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        if (intent.getStringExtra("route") == "transfers") requestedRoute.value = "transfers"
        handleShareIntent(intent)
    }

    /** Share-sheet entry: enqueue immediately, upload when Home is reachable (FR-3.4). */
    private fun handleShareIntent(intent: Intent?) {
        if (intent?.action == Intent.ACTION_SEND || intent?.action == Intent.ACTION_SEND_MULTIPLE) {
            queue.enqueueFromShare(intent)
        }
    }
}
