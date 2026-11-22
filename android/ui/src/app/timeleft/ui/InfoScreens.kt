package app.timeleft.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.timeleft.core.HabitKind
import app.timeleft.core.dataSource
import java.util.Locale

private const val HOUR = 3_600_000L
private const val DAY = 24 * HOUR

@Composable
fun ExplainScreen(model: AppModel) {
    val engine = model.engine ?: return
    val x = remember(model.revision) { runCatching { engine.explain(model.now()) }.getOrNull() } ?: return
    val status = model.status
    Page("Почему такая цифра", onBack = { model.screen = Screen.Main }) {
        Text("${x.countryName}, возраст ${"%.1f".format(Locale.forLanguageTag("ru"), x.estimate.age)}", fontSize = 14.sp)
        SectionTitle("Средний человек того же пола и возраста")
        Text("Медиана: ещё ${Ru.yearsDecimal(x.baseline.median)}")
        Hint("Диапазон ${Ru.yearsDecimal(x.baseline.lower)} – ${Ru.yearsDecimal(x.baseline.upper)}")

        SectionTitle("Ваши ответы")
        val answered = x.factors.filter { it.answered }
        if (answered.isEmpty()) Hint("Анкета не заполнена — показан средний человек.")
        answered.forEach { f ->
            Row(Modifier.fillMaxWidth().padding(vertical = 3.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(Ru.factor(f.factor), fontSize = 14.sp)
                    Hint("риск ×${"%.2f".format(Locale.forLanguageTag("ru"), f.hazardRatio)} к среднему")
                }
                Text(Ru.signedYears(f.years), color = if (f.years < 0) Loss else Glow, fontWeight = FontWeight.SemiBold)
            }
        }
        HorizontalDivider(Modifier.padding(vertical = 8.dp))
        Text(
            "Итог: риск ×${"%.2f".format(Locale.forLanguageTag("ru"), x.estimate.hazardRatio)}" +
                (if (x.estimate.clamped) " (ограничен)" else "") + " → медиана ${Ru.yearsDecimal(x.estimate.median)}",
            fontWeight = FontWeight.SemiBold,
        )
        Hint("Сумма по факторам не равна итогу: факторы взаимодействуют, образ жизни учитывается с поправкой на пересечение.")

        if (status != null && status.habits.isNotEmpty()) {
            SectionTitle("Привычки")
            status.habits.forEach { h ->
                Row(Modifier.fillMaxWidth().padding(vertical = 3.dp), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(Ru.habit(h.kind) + if (h.tracked) "" else " (больше не отслеживается)", fontSize = 14.sp)
                        h.average?.let { Hint("среднее за ~90 дней: ${habitDisplay(h.kind, it)}") }
                    }
                    Text(Ru.delta(h.bankedMs), color = deltaColor(h.bankedMs))
                }
            }
            Hint("Каждая отметка — один день из ожидаемой жизни: сдвиг = 1 день × (изменение медианы, если так жить всегда) / медиана.")
        }
        SectionTitle("Как считается")
        Hint(
            "Смертность по стране и полу — по таблицам ООН с прогнозом улучшения для вашего поколения. " +
                "Каждый ответ меняет риск относительно среднего человека; с возрастом влияние ослабевает. " +
                "Отсчёт идёт к медиане: половина людей с похожим профилем живёт дольше.",
        )
        Spacer(Modifier.height(6.dp))
        Hint("Данные: ${dataSource()}")
        Spacer(Modifier.height(6.dp))
        Hint(Ru.DISCLAIMER)
    }
}

@Composable
fun JournalScreen(model: AppModel) {
    val engine = model.engine ?: return
    val entries = remember(model.revision) { engine.ledger(200u) }
    val offset = model.offset()
    Page("Журнал", onBack = { model.screen = Screen.Main }) {
        Hint("Каждое изменение отсчёта и его причины. Последние сверху.")
        entries.forEach { e ->
            HorizontalDivider(Modifier.padding(vertical = 6.dp))
            Row(Modifier.fillMaxWidth()) {
                Column(Modifier.weight(1f)) {
                    Text(Ru.reason(e.reason), fontSize = 14.sp)
                    Hint(Ru.dateTime(e.tsMs, offset))
                }
                val total = if (e.reason is app.timeleft.core.Reason.Initial) Ru.yearsDecimal(e.deltaMs / (365.2425 * DAY)) else Ru.delta(e.deltaMs)
                Text(total, color = deltaColor(e.deltaMs), fontWeight = FontWeight.SemiBold)
            }
            if (e.parts.size > 1) {
                e.parts.forEach { p ->
                    Row(Modifier.fillMaxWidth().padding(start = 12.dp)) {
                        Text("· " + Ru.component(p.component), Modifier.weight(1f), fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Text(Ru.delta(p.deltaMs), fontSize = 12.sp, color = deltaColor(p.deltaMs))
                    }
                }
            }
        }
    }
}

@Composable
fun SettingsScreen(model: AppModel) {
    val status = model.status ?: return
    var confirmReset by remember { mutableStateOf(false) }
    Page("Ещё", onBack = { model.screen = Screen.Main }) {
        Button(onClick = { model.screen = Screen.EditProfile }, Modifier.fillMaxWidth()) { Text("Изменить анкету и привычки") }

        val tracked = status.habits.filter { it.tracked && it.average != null }
        if (tracked.isNotEmpty()) {
            SectionTitle("Новая норма")
            Hint("Если привычка изменилась надолго, сделайте среднее за 90 дней ответом анкеты — прогноз пересчитается.")
            tracked.forEach { h ->
                val avg = h.average ?: return@forEach
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Text("${Ru.habit(h.kind)}: ${habitDisplay(h.kind, avg)}", Modifier.weight(1f), fontSize = 14.sp)
                    TextButton(onClick = { model.act { e, now -> e.adoptAverage(h.kind, now) } }) { Text("Принять") }
                }
            }
        }

        SectionTitle("Тестовое время")
        Hint("Сдвиг часов только для проверки: можно «прожить» несколько дней и отметить привычки за каждый.")
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            OutlinedButton(onClick = { model.shiftTime(HOUR) }) { Text("+1 ч") }
            OutlinedButton(onClick = { model.shiftTime(DAY) }) { Text("+1 день") }
            OutlinedButton(onClick = { model.shiftTime(7 * DAY) }) { Text("+7 дней") }
        }
        if (model.clock.shiftMs != 0L) {
            Surface(color = MaterialTheme.colorScheme.surfaceVariant, shape = MaterialTheme.shapes.small, modifier = Modifier.padding(top = 6.dp)) {
                Row(Modifier.padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("Сейчас сдвиг ${Ru.delta(model.clock.shiftMs)}", color = Loss, fontSize = 13.sp)
                    TextButton(onClick = { model.shiftTime(0) }) { Text("Вернуть реальное время") }
                }
            }
            Hint("Записи, сделанные «в будущем», останутся в журнале. Для чистого старта после проверки удалите данные.")
        }

        SectionTitle("Данные")
        Hint("Всё хранится только на этом устройстве: ${model.storageDescription}")
        OutlinedButton(onClick = { confirmReset = true }, Modifier.fillMaxWidth().padding(top = 6.dp)) {
            Text("Удалить все данные", color = Loss)
        }
    }
    if (confirmReset) {
        AlertDialog(
            onDismissRequest = { confirmReset = false },
            title = { Text("Удалить все данные?") },
            text = { Text("Анкета, отметки и журнал будут удалены без возможности восстановления.") },
            confirmButton = { TextButton(onClick = { confirmReset = false; model.resetAll() }) { Text("Удалить", color = Loss) } },
            dismissButton = { TextButton(onClick = { confirmReset = false }) { Text("Отмена") } },
        )
    }
}
