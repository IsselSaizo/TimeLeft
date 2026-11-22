package app.timeleft.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.timeleft.core.Activity
import app.timeleft.core.Country
import app.timeleft.core.Factors
import app.timeleft.core.HabitKind
import app.timeleft.core.Profile
import app.timeleft.core.Sex
import app.timeleft.core.Smoking
import app.timeleft.core.Social
import app.timeleft.core.bmi
import app.timeleft.core.countries
import java.util.Locale

private enum class SmokingChoice { Never, Current, Former }
private enum class ActivityChoice { Steps, Minutes }

/** Mutable form backing the questionnaire. Blank fields mean "not answered". */
private class ProfileForm(p: Profile?, habits: List<HabitKind>) {
    var country by mutableStateOf(p?.country ?: defaultCountry())
    var countryQuery by mutableStateOf("")
    var sex by mutableStateOf(p?.sex)
    var birth by mutableStateOf(p?.birthDate?.let { Ru.date(it) } ?: "")
    var smoking by mutableStateOf(
        when (p?.factors?.smoking) {
            is Smoking.Never -> SmokingChoice.Never
            is Smoking.Current -> SmokingChoice.Current
            is Smoking.Former -> SmokingChoice.Former
            null -> null
        },
    )
    var cigarettes by mutableStateOf((p?.factors?.smoking as? Smoking.Current)?.cigarettesPerDay?.plain() ?: "")
    var quitAge by mutableStateOf((p?.factors?.smoking as? Smoking.Former)?.quitAge?.plain() ?: "")
    var activity by mutableStateOf(
        when (p?.factors?.activity) {
            is Activity.Steps -> ActivityChoice.Steps
            is Activity.Mvpa -> ActivityChoice.Minutes
            null -> null
        },
    )
    var steps by mutableStateOf((p?.factors?.activity as? Activity.Steps)?.perDay?.plain() ?: "")
    var minutes by mutableStateOf((p?.factors?.activity as? Activity.Mvpa)?.minutesPerWeek?.plain() ?: "")
    var height by mutableStateOf("")
    var weight by mutableStateOf("")
    var bmiDirect by mutableStateOf(p?.factors?.bmi?.let { "%.1f".format(Locale.US, it) } ?: "")
    var drinks by mutableStateOf(p?.factors?.alcoholGPerWeek?.let { (it / 10.0).plain() } ?: "")
    var sleep by mutableStateOf(p?.factors?.sleepHours?.plain() ?: "")
    var social by mutableStateOf(p?.factors?.social)
    var diabetes by mutableStateOf(p?.factors?.diabetes ?: false)
    var stroke by mutableStateOf(p?.factors?.stroke ?: false)
    var heartAttack by mutableStateOf(p?.factors?.heartAttack ?: false)
    var habits by mutableStateOf(habits.toSet())
    var consent by mutableStateOf(p != null)

    /** "23.11.1990" → "1990-11-23" */
    fun birthIso(): String? {
        val m = Regex("""^\s*(\d{1,2})[./-](\d{1,2})[./-](\d{4})\s*$""").find(birth) ?: return null
        val (d, mo, y) = m.destructured
        return "%04d-%02d-%02d".format(y.toInt(), mo.toInt(), d.toInt())
    }

    fun bmiValue(): Double? {
        val h = height.toNumberOrNull()
        val w = weight.toNumberOrNull()
        return if (h != null && w != null && h > 0) bmi(w, h) else bmiDirect.toNumberOrNull()
    }

    fun errors(): List<String> = buildList {
        if (sex == null) add("Укажите пол (или «не указывать»)")
        if (birthIso() == null) add("Дата рождения в формате ДД.ММ.ГГГГ")
        if (smoking == SmokingChoice.Current && cigarettes.toNumberOrNull() == null) add("Сколько сигарет в день?")
        if (smoking == SmokingChoice.Former && quitAge.toNumberOrNull() == null) add("В каком возрасте бросили?")
        if (HabitKind.STEPS in habits && HabitKind.ACTIVITY_MINUTES in habits) add("Отслеживайте либо шаги, либо минуты активности")
        if (!consent) add("Подтвердите, что вам есть 18 лет и вы прочитали предупреждение")
    }

    fun toProfile(): Profile = Profile(
        country = country,
        sex = sex ?: Sex.UNSPECIFIED,
        birthDate = birthIso() ?: "",
        factors = Factors(
            smoking = when (smoking) {
                SmokingChoice.Never -> Smoking.Never
                SmokingChoice.Current -> Smoking.Current(cigarettes.toNumberOrNull() ?: 0.0)
                SmokingChoice.Former -> Smoking.Former(quitAge.toNumberOrNull() ?: 0.0)
                null -> null
            },
            activity = when (activity) {
                ActivityChoice.Steps -> steps.toNumberOrNull()?.let { Activity.Steps(it) }
                ActivityChoice.Minutes -> minutes.toNumberOrNull()?.let { Activity.Mvpa(it) }
                null -> null
            },
            bmi = bmiValue(),
            alcoholGPerWeek = drinks.toNumberOrNull()?.let { it * 10.0 },
            sleepHours = sleep.toNumberOrNull(),
            social = social,
            diabetes = diabetes,
            stroke = stroke,
            heartAttack = heartAttack,
        ),
    )

    companion object {
        fun defaultCountry(): String =
            runCatching { Locale.getDefault().isO3Country }.getOrNull()?.takeIf { it.length == 3 } ?: "WLD"
    }
}

@Composable
fun ProfileScreen(model: AppModel, editing: Boolean) {
    val engine = model.engine
    val form = remember(editing) {
        if (editing && engine != null) ProfileForm(engine.profile(), engine.trackedHabits()) else ProfileForm(null, emptyList())
    }
    val allCountries = remember { countries().sortedBy { it.name } }
    Page(
        title = if (editing) "Анкета" else "Time Left",
        onBack = if (editing) ({ model.screen = Screen.Settings }) else null,
    ) {
        if (!editing) {
            Text(
                "Обратный отсчёт по мотивам фильма «Время». Ответьте на вопросы — чем больше ответов, " +
                    "тем точнее оценка. Любой вопрос можно пропустить.",
                fontSize = 14.sp,
            )
        }
        SectionTitle("Страна")
        CountryPicker(allCountries, form.country, form.countryQuery, { form.countryQuery = it }) {
            form.country = it
            form.countryQuery = ""
        }
        SectionTitle("Пол")
        ChoiceChips(listOf(Sex.MALE to "Мужской", Sex.FEMALE to "Женский", Sex.UNSPECIFIED to "Не указывать"), form.sex) { form.sex = it }
        SectionTitle("Дата рождения")
        OutlinedTextField(form.birth, { form.birth = it }, label = { Text("ДД.ММ.ГГГГ") }, singleLine = true, modifier = Modifier.fillMaxWidth())

        SectionTitle("Курение")
        ChoiceChips(
            listOf(SmokingChoice.Never to "Никогда", SmokingChoice.Current to "Курю", SmokingChoice.Former to "Бросил(а)"),
            form.smoking,
        ) { form.smoking = if (form.smoking == it) null else it }
        when (form.smoking) {
            SmokingChoice.Current -> NumberField(form.cigarettes, { form.cigarettes = it }, "Сигарет в день")
            SmokingChoice.Former -> NumberField(form.quitAge, { form.quitAge = it }, "Возраст, в котором бросили")
            else -> {}
        }

        SectionTitle("Физическая активность")
        ChoiceChips(listOf(ActivityChoice.Steps to "Шаги в день", ActivityChoice.Minutes to "Минуты в неделю"), form.activity) {
            form.activity = if (form.activity == it) null else it
        }
        when (form.activity) {
            ActivityChoice.Steps -> NumberField(form.steps, { form.steps = it }, "Шагов в день в среднем")
            ActivityChoice.Minutes -> NumberField(form.minutes, { form.minutes = it }, "Минут умеренной/интенсивной нагрузки в неделю")
            null -> {}
        }

        SectionTitle("Рост и вес")
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            NumberField(form.height, { form.height = it }, "Рост, см", Modifier.weight(1f))
            NumberField(form.weight, { form.weight = it }, "Вес, кг", Modifier.weight(1f))
        }
        form.bmiValue()?.let { Hint("Индекс массы тела: %.1f".format(it)) }

        SectionTitle("Алкоголь")
        NumberField(form.drinks, { form.drinks = it }, "Порций в неделю (1 порция = 10 г спирта)")
        Hint("≈ 250 мл пива 5 %, 100 мл вина 12 % или 30 мл водки")

        SectionTitle("Сон")
        NumberField(form.sleep, { form.sleep = it }, "Часов за ночь в среднем")

        SectionTitle("Близкие люди, друзья")
        ChoiceChips(listOf(Social.STRONG to "Много", Social.AVERAGE to "Средне", Social.WEAK to "Мало"), form.social) {
            form.social = if (form.social == it) null else it
        }

        SectionTitle("Диагнозы")
        CheckRow("Диабет", form.diabetes) { form.diabetes = it }
        CheckRow("Перенесённый инсульт", form.stroke) { form.stroke = it }
        CheckRow("Перенесённый инфаркт", form.heartAttack) { form.heartAttack = it }

        SectionTitle("Отмечать каждый день")
        Hint("Каждая отметка сдвигает отсчёт на несколько минут или часов.")
        for (k in HabitKind.entries) {
            CheckRow("${Ru.habit(k)} — ${Ru.habitUnit(k)}", k in form.habits) { on ->
                form.habits = if (on) {
                    form.habits - when (k) {
                        HabitKind.STEPS -> HabitKind.ACTIVITY_MINUTES
                        HabitKind.ACTIVITY_MINUTES -> HabitKind.STEPS
                        else -> k
                    } + k
                } else {
                    form.habits - k
                }
            }
        }

        if (!editing) {
            Spacer(Modifier.height(12.dp))
            Hint(Ru.DISCLAIMER)
            CheckRow("Мне есть 18 лет, я понимаю, что это не медицинский прогноз", form.consent) { form.consent = it }
        }

        val errors = form.errors()
        Spacer(Modifier.height(12.dp))
        errors.forEach { Text("• $it", color = Loss, fontSize = 13.sp) }
        Button(
            onClick = {
                val habits = HabitKind.entries.filter { it in form.habits }
                if (editing) model.updateProfile(form.toProfile(), habits) else model.create(form.toProfile(), habits)
            },
            enabled = errors.isEmpty(),
            modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        ) { Text(if (editing) "Сохранить" else "Запустить отсчёт") }
    }
}

@Composable
fun CheckRow(label: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickable { onChange(!checked) },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Checkbox(checked, onChange)
        Text(label, fontSize = 14.sp)
    }
}

@Composable
private fun CountryPicker(
    all: List<Country>,
    selected: String,
    query: String,
    onQuery: (String) -> Unit,
    onPick: (String) -> Unit,
) {
    val name = all.firstOrNull { it.iso3 == selected }?.name ?: selected
    OutlinedTextField(
        value = query,
        onValueChange = onQuery,
        label = { Text("Выбрано: $name — начните вводить, чтобы сменить") },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
    )
    if (query.isNotBlank()) {
        val q = query.trim().lowercase()
        val hits = all.filter { it.name.lowercase().contains(q) || it.iso3.lowercase().startsWith(q) }.take(6)
        Surface(color = MaterialTheme.colorScheme.surfaceVariant, shape = MaterialTheme.shapes.small) {
            Column(Modifier.fillMaxWidth()) {
                if (hits.isEmpty()) Text("Ничего не найдено (названия — на английском)", Modifier.padding(12.dp), fontSize = 13.sp)
                hits.forEach { c ->
                    Text(
                        "${c.name}  (${c.iso3})",
                        Modifier.fillMaxWidth().clickable { onPick(c.iso3) }.padding(horizontal = 12.dp, vertical = 10.dp),
                        fontSize = 14.sp,
                    )
                }
            }
        }
    }
}
