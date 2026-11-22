package app.timeleft.desktop

import androidx.compose.runtime.remember
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import app.timeleft.ui.AppClock
import app.timeleft.ui.AppModel
import app.timeleft.ui.Storage
import app.timeleft.ui.TimeLeftApp
import java.io.File

/**
 * Same file as the `timeleft` CLI uses, so both see one state:
 * `TIMELEFT_STATE`, else `%APPDATA%\TimeLeft\state.json` (or `~/.local/share/TimeLeft`).
 */
class FileStorage(private val file: File) : Storage {
    override fun load(): String? = file.takeIf { it.isFile }?.readText()

    override fun save(json: String) {
        file.parentFile?.mkdirs()
        val tmp = File(file.path + ".tmp")
        tmp.writeText(json)
        if (!tmp.renameTo(file)) {
            file.delete()
            tmp.renameTo(file)
        }
    }

    override fun delete() {
        file.delete()
    }

    override val description: String get() = file.absolutePath

    companion object {
        fun default(): FileStorage {
            System.getenv("TIMELEFT_STATE")?.let { return FileStorage(File(it)) }
            val base = System.getenv("APPDATA")?.let(::File)
                ?: System.getenv("XDG_DATA_HOME")?.let(::File)
                ?: File(System.getProperty("user.home"), ".local/share")
            return FileStorage(File(base, "TimeLeft/state.json"))
        }
    }
}

fun main() = application {
    val model = remember { AppModel(FileStorage.default(), AppClock()) }
    Window(
        onCloseRequest = ::exitApplication,
        title = "Time Left",
        state = rememberWindowState(size = DpSize(460.dp, 900.dp)),
    ) {
        TimeLeftApp(model)
    }
}
