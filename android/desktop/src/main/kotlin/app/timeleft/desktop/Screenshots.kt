package app.timeleft.desktop

import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.unit.Density
import app.timeleft.ui.AppClock
import app.timeleft.ui.AppModel
import app.timeleft.ui.Screen
import app.timeleft.ui.TimeLeftApp
import org.jetbrains.skia.EncodedImageFormat
import java.io.File

/**
 * Renders every screen to PNG without opening a window (visual check / store screenshots).
 * Usage: `gradlew :desktop:screenshots -Pstate=<state.json> -Pout=<dir>`
 */
fun main(args: Array<String>) {
    val state = File(args.getOrNull(0) ?: error("state.json path required"))
    val out = File(args.getOrNull(1) ?: "build/screenshots").apply { mkdirs() }
    val (w, h) = 460 to 900

    fun shot(name: String, model: AppModel) {
        val scene = ImageComposeScene(w, h, Density(1f)) { TimeLeftApp(model) }
        // A few frames so effects and layout settle.
        repeat(3) { scene.render(it * 100_000_000L) }
        val png = scene.render(400_000_000L).encodeToData(EncodedImageFormat.PNG) ?: error("encode failed")
        File(out, "$name.png").writeBytes(png.bytes)
        scene.close()
        println("wrote ${File(out, "$name.png")}")
    }

    val empty = File.createTempFile("timeleft-empty", ".json").apply { delete() }
    shot("01-onboarding", AppModel(FileStorage(empty), AppClock()))
    val model = AppModel(ReadOnlyStorage(FileStorage(state)), AppClock())
    for ((i, s) in listOf(Screen.Main, Screen.CheckIn, Screen.Explain, Screen.Journal, Screen.Settings, Screen.EditProfile).withIndex()) {
        model.screen = s
        shot("%02d-%s".format(i + 2, s.name.lowercase()), model)
    }
}

/** Never writes back, so screenshots cannot alter the input state. */
private class ReadOnlyStorage(private val inner: FileStorage) : app.timeleft.ui.Storage by inner {
    override fun save(json: String) {}
    override fun delete() {}
}
