package app.timeleft.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.setValue
import java.time.Instant
import java.time.ZoneId

/** Where the state JSON lives (a file on desktop, app storage on Android). */
interface Storage {
    fun load(): String?
    fun save(json: String)
    fun delete()
    /** Human-readable location, shown in settings. */
    val description: String
}

/**
 * Wall clock with an optional test shift, so a day of habits can be simulated in seconds.
 * The shift is not persisted.
 */
class AppClock {
    var shiftMs by mutableLongStateOf(0L)

    fun now(): Long = System.currentTimeMillis() + shiftMs

    fun utcOffsetMinutes(atMs: Long = now()): Int =
        ZoneId.systemDefault().rules.getOffset(Instant.ofEpochMilli(atMs)).totalSeconds / 60
}
