package app.timeleft.ui

import app.timeleft.core.Component
import app.timeleft.core.FactorKind
import app.timeleft.core.HabitKind
import app.timeleft.core.Reason
import java.time.Instant
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlin.math.abs

/** Russian UI text. Other languages (en, tr) will get their own object with the same API. */
object Ru {
    private const val MINUTE = 60_000L
    private const val HOUR = 60 * MINUTE
    private const val DAY = 24 * HOUR

    fun plural(n: Long, one: String, few: String, many: String): String {
        val n100 = abs(n) % 100
        val n10 = abs(n) % 10
        return when {
            n100 in 11..14 -> many
            n10 == 1L -> one
            n10 in 2..4 -> few
            else -> many
        }
    }

    fun yearsWord(n: Long) = plural(n, "год", "года", "лет")
    fun daysWord(n: Long) = plural(n, "день", "дня", "дней")

    /** "34,9 года" — fractional numbers take the genitive singular. */
    fun yearsDecimal(y: Double): String = String.format(Locale.forLanguageTag("ru"), "%.1f года", y)

    fun signedYears(y: Double): String {
        if (abs(y) < 0.05) return "0,0 г."
        val sign = if (y < 0) "−" else "+"
        return sign + String.format(Locale.forLanguageTag("ru"), "%.1f", abs(y)) + " г."
    }

    /** "+3 ч 16 мин", "−2 дн 4 ч", "+45 с". */
    fun delta(ms: Long): String {
        val sign = if (ms < 0) "−" else "+"
        val a = abs(ms)
        return sign + when {
            a >= 365 * DAY -> String.format(Locale.forLanguageTag("ru"), "%.1f г.", a / (365.2425 * DAY))
            a >= DAY -> "${a / DAY} дн ${a % DAY / HOUR} ч"
            a >= HOUR -> "${a / HOUR} ч ${a % HOUR / MINUTE} мин"
            a >= MINUTE -> "${a / MINUTE} мин ${a % MINUTE / 1000} с"
            else -> "${a / 1000} с"
        }
    }

    private val dateTime = DateTimeFormatter.ofPattern("dd.MM.yyyy HH:mm")

    fun dateTime(ms: Long, offsetMinutes: Int): String =
        dateTime.format(Instant.ofEpochMilli(ms).atOffset(ZoneOffset.ofTotalSeconds(offsetMinutes * 60)))

    /** "2026-11-23" → "23.11.2026" */
    fun date(iso: String): String = iso.split("-").let { if (it.size == 3) "${it[2]}.${it[1]}.${it[0]}" else iso }

    fun habit(k: HabitKind): String = when (k) {
        HabitKind.CIGARETTES -> "Сигареты"
        HabitKind.ACTIVITY_MINUTES -> "Активность"
        HabitKind.STEPS -> "Шаги"
        HabitKind.ALCOHOL_GRAMS -> "Алкоголь"
        HabitKind.SLEEP_HOURS -> "Сон"
    }

    /** Unit of the value the user types for a habit. */
    fun habitUnit(k: HabitKind): String = when (k) {
        HabitKind.CIGARETTES -> "шт. за день"
        HabitKind.ACTIVITY_MINUTES -> "мин активной нагрузки"
        HabitKind.STEPS -> "шагов за день"
        HabitKind.ALCOHOL_GRAMS -> "порций (1 порция = 10 г спирта)"
        HabitKind.SLEEP_HOURS -> "часов сна прошлой ночью"
    }

    fun factor(f: FactorKind): String = when (f) {
        FactorKind.SMOKING -> "Курение"
        FactorKind.ACTIVITY -> "Физическая активность"
        FactorKind.BMI -> "Индекс массы тела"
        FactorKind.ALCOHOL -> "Алкоголь"
        FactorKind.SLEEP -> "Сон"
        FactorKind.SOCIAL -> "Социальные связи"
        FactorKind.DIABETES -> "Диабет"
        FactorKind.STROKE -> "Инсульт"
        FactorKind.HEART_ATTACK -> "Инфаркт"
    }

    fun component(c: Component): String = when (c) {
        is Component.Initial -> "первая оценка"
        is Component.Ageing -> "перекалибровка (прошло время)"
        is Component.Habit -> habit(c.habit)
        is Component.Revert -> "откат прежней отметки"
        is Component.Profile -> "изменение анкеты"
        is Component.HabitSet -> "смена привычек"
        is Component.Adopt -> "${habit(c.habit)}: новая норма"
    }

    fun reason(r: Reason): String = when (r) {
        is Reason.Initial -> "Первая оценка"
        is Reason.Recalibration -> "Ежедневная перекалибровка"
        is Reason.CheckIn -> "Отметка за ${date(r.date)}"
        is Reason.CheckInUndo -> "Отметка за ${date(r.date)} отменена"
        is Reason.ProfileUpdate -> "Анкета изменена"
        is Reason.HabitsChanged -> "Изменён список привычек"
        is Reason.AdoptAverage -> "${habit(r.habit)}: среднее стало новой нормой"
    }

    const val DISCLAIMER =
        "Оценка по популяционной статистике (ООН, WPP 2024) и опубликованным исследованиям. " +
            "Это не медицинское изделие и не прогноз: показаны связи, а не причины. " +
            "По вопросам здоровья обращайтесь к врачу."
}

/** Parses "7,5" or "7.5"; blank → null. */
fun String.toNumberOrNull(): Double? = trim().replace(',', '.').takeIf { it.isNotEmpty() }?.toDoubleOrNull()

/** Compact number for display and pre-filled fields: "10", "7,5" (comma, Russian style). */
fun Double.plain(): String =
    if (this == Math.floor(this) && abs(this) < 1e12) this.toLong().toString()
    else String.format(Locale.forLanguageTag("ru"), "%.1f", this).trimEnd('0').trimEnd(',')

/** Habit value as the user types it (alcohol in drinks, steps as whole numbers). */
fun habitDisplay(k: HabitKind, coreValue: Double): String = when (k) {
    HabitKind.ALCOHOL_GRAMS -> (coreValue / 10.0).plain()
    HabitKind.STEPS -> Math.round(coreValue).toString()
    else -> coreValue.plain()
}
