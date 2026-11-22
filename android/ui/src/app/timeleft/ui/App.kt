package app.timeleft.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier

/** Root of the app UI; platform shells only provide [AppModel]. */
@Composable
fun TimeLeftApp(model: AppModel) {
    TimeLeftTheme {
        val snackbar = remember { SnackbarHostState() }
        LaunchedEffect(model.message) {
            model.message?.let {
                snackbar.showSnackbar(it)
                model.message = null
            }
        }
        Scaffold(snackbarHost = { SnackbarHost(snackbar) }) { padding ->
            Box(Modifier.fillMaxSize().padding(padding)) {
                if (model.engine == null) {
                    ProfileScreen(model, editing = false)
                } else {
                    when (model.screen) {
                        Screen.Onboarding, Screen.Main -> MainScreen(model)
                        Screen.CheckIn -> CheckInScreen(model)
                        Screen.Explain -> ExplainScreen(model)
                        Screen.Journal -> JournalScreen(model)
                        Screen.Settings -> SettingsScreen(model)
                        Screen.EditProfile -> ProfileScreen(model, editing = true)
                    }
                }
            }
        }
    }
}
