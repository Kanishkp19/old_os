package com.homehub

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
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

    @Inject lateinit var queue: QueueRepository

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        handleShareIntent(intent)
        setContent {
            MaterialTheme {
                val nav = rememberNavController()
                val navBackStackEntry by nav.currentBackStackEntryAsState()
                val currentRoute = navBackStackEntry?.destination?.route ?: "home"
                Scaffold(
                    bottomBar = {
                        NavigationBar {
                            NavigationBarItem(
                                selected = currentRoute == "home",
                                onClick = { nav.navigate("home") { popUpTo("home") { saveState = true }; launchSingleTop = true; restoreState = true } },
                                icon = {},
                                label = { Text("Home") }
                            )
                            NavigationBarItem(
                                selected = currentRoute == "transfers",
                                onClick = { nav.navigate("transfers") { popUpTo("home") { saveState = true }; launchSingleTop = true; restoreState = true } },
                                icon = {},
                                label = { Text("Transfers") }
                            )
                            NavigationBarItem(
                                selected = currentRoute == "remote",
                                onClick = { nav.navigate("remote") { popUpTo("home") { saveState = true }; launchSingleTop = true; restoreState = true } },
                                icon = {},
                                label = { Text("Remote") }
                            )
                            NavigationBarItem(
                                selected = currentRoute == "devices",
                                onClick = { nav.navigate("devices") { popUpTo("home") { saveState = true }; launchSingleTop = true; restoreState = true } },
                                icon = {},
                                label = { Text("Devices") }
                            )
                        }
                    }
                ) { padding ->
                    NavHost(nav, startDestination = "home", modifier = Modifier.padding(padding)) {
                        composable("home") { HomeScreen(onPair = { nav.navigate("pair") }) }
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
        handleShareIntent(intent)
    }

    /** Share-sheet entry: enqueue immediately, upload when Home is reachable (FR-3.4). */
    private fun handleShareIntent(intent: Intent?) {
        if (intent?.action == Intent.ACTION_SEND || intent?.action == Intent.ACTION_SEND_MULTIPLE) {
            queue.enqueueFromShare(intent)
        }
    }
}
