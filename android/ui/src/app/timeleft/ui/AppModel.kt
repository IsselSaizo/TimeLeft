package app.timeleft.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import app.timeleft.core.Engine
import app.timeleft.core.HabitKind
import app.timeleft.core.LedgerEntry
import app.timeleft.core.Profile
import app.timeleft.core.Status
import app.timeleft.core.TimeLeftException
import app.timeleft.core.localDate

enum class Screen { Onboarding, Main, CheckIn, Explain, Journal, Settings, EditProfile }

/** UI state holder shared by the desktop and Android shells. */
class AppModel(private val storage: Storage, val clock: AppClock) {
    var engine by mutableStateOf<Engine?>(null)
        private set
    var status by mutableStateOf<Status?>(null)
        private set
    var screen by mutableStateOf(Screen.Onboarding)
    /** Latest ledger entry produced by a user action, shown as feedback. */
    var lastEntry by mutableStateOf<LedgerEntry?>(null)
    /** One-shot message for the snackbar. */
    var message by mutableStateOf<String?>(null)
    /** Bumped on every change so screens re-query the engine. */
    var revision by mutableIntStateOf(0)
        private set

    init {
        val json = runCatching { storage.load() }.getOrNull()
        if (json != null) {
            try {
                engine = Engine.load(json)
                screen = Screen.Main
            } catch (e: TimeLeftException) {
                message = "Не удалось прочитать сохранённые данные: ${e.message}"
            }
        }
        refresh()
    }

    fun now(): Long = clock.now()
    fun offset(): Int = clock.utcOffsetMinutes(now())
    fun today(): String = localDate(now(), offset())

    /** Re-read status; records the daily recalibration when due. */
    fun refresh() {
        val e = engine ?: return
        try {
            if (e.recalibrateIfDue(now()) != null) save()
            status = e.status(now())
            revision++
        } catch (ex: TimeLeftException) {
            message = ex.message
        }
    }

    private fun save() {
        engine?.let { storage.save(it.toJson()) }
    }

    fun create(profile: Profile, habits: List<HabitKind>) {
        try {
            engine?.close()
            engine = Engine.create(profile, habits, now())
            save()
            lastEntry = null
            screen = Screen.Main
            refresh()
        } catch (e: TimeLeftException) {
            message = e.message
        }
    }

    /** Run a ledger-producing action, persist, and show its result. */
    fun act(block: (Engine, Long) -> LedgerEntry): LedgerEntry? {
        val e = engine ?: return null
        return try {
            val entry = block(e, now())
            save()
            lastEntry = entry
            refresh()
            entry
        } catch (ex: TimeLeftException) {
            message = ex.message
            null
        }
    }

    fun updateProfile(profile: Profile, habits: List<HabitKind>) {
        val e = engine ?: return
        if (e.profile() != profile) act { en, now -> en.updateProfile(profile, now) }
        if (e.trackedHabits().toSet() != habits.toSet()) act { en, now -> en.setHabits(habits, now) }
        screen = Screen.Main
    }

    fun resetAll() {
        storage.delete()
        engine?.close()
        engine = null
        status = null
        lastEntry = null
        clock.shiftMs = 0
        screen = Screen.Onboarding
    }

    fun shiftTime(ms: Long) {
        clock.shiftMs = if (ms == 0L) 0 else clock.shiftMs + ms
        refresh()
    }

    val storageDescription: String get() = storage.description
}
