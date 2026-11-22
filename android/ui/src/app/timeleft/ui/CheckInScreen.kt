package app.timeleft.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.timeleft.core.HabitKind
import app.timeleft.core.HabitValue
import app.timeleft.core.LedgerEntry
import app.timeleft.core.TimeLeftException

/** Alcohol is typed in standard drinks, stored in grams. */
private fun toCore(k: HabitKind, typed: Double) = if (k == HabitKind.ALCOHOL_GRAMS) typed * 10.0 else typed
private fun fromCore(k: HabitKind, v: Double) = if (k == HabitKind.ALCOHOL_GRAMS) v / 10.0 else v

@Composable
fun CheckInScreen(model: AppModel) {
    val engine = model.engine ?: return
    val status = model.status ?: return
    val today = model.today()
    val tracked = remember(model.revision) { engine.trackedHabits() }
    val existing = status.lastCheckIn?.takeIf { it.date == today }
    val inputs = remember(existing) {
        mutableStateMapOf<HabitKind, String>().apply {
            existing?.values?.forEach { put(it.kind, fromCore(it.kind, it.value).plain()) }
        }
    }
    var result by remember { mutableStateOf<LedgerEntry?>(null) }

    Page("Отметка за ${Ru.date(today)}", onBack = { model.screen = Screen.Main }) {
        if (tracked.isEmpty()) {
            Text("Вы пока не отслеживаете привычки. Включите их в разделе «Ещё → Анкета».")
            return@Page
        }
        if (existing != null) Hint("Сегодня уже отмечено — новые значения заменят прежние.")
        val res = result
        if (res != null) {
            ResultCard(res)
            Spacer(Modifier.height(12.dp))
            Button(onClick = { model.screen = Screen.Main }, Modifier.fillMaxWidth()) { Text("Готово") }
            return@Page
        }
        for (k in tracked) {
            SectionTitle(Ru.habit(k))
            val text = inputs[k] ?: ""
            NumberField(text, { inputs[k] = it }, Ru.habitUnit(k))
            val v = text.toNumberOrNull()
            if (v != null) {
                val preview = try {
                    val ms = engine.preview(k, toCore(k, v), model.now())
                    "Эффект дня: ${Ru.delta(ms)}" to deltaColor(ms)
                } catch (e: TimeLeftException) {
                    (e.message ?: "Недопустимое значение") to Loss
                }
                Text(preview.first, color = preview.second, fontSize = 13.sp)
            }
            status.habits.firstOrNull { it.kind == k }?.average?.let {
                Hint("Среднее за ~90 дней: ${habitDisplay(k, it)}")
            }
        }
        val values = tracked.mapNotNull { k -> inputs[k]?.toNumberOrNull()?.let { HabitValue(k, toCore(k, it)) } }
        Spacer(Modifier.height(16.dp))
        Hint("Пустое поле — значение из анкеты, отсчёт не меняется.")
        Row(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (existing != null) {
                OutlinedButton(
                    onClick = { model.act { e, now -> e.undoLastCheckIn(now) }?.let { result = it } },
                    Modifier.weight(1f),
                ) { Text("Отменить отметку") }
            }
            Button(
                onClick = { model.act { e, now -> e.checkIn(today, values, now) }?.let { result = it } },
                enabled = values.isNotEmpty(),
                modifier = Modifier.weight(1f),
            ) { Text("Сохранить") }
        }
    }
}

@Composable
fun ResultCard(entry: LedgerEntry) {
    Surface(color = MaterialTheme.colorScheme.surfaceVariant, shape = MaterialTheme.shapes.medium) {
        Column(Modifier.fillMaxWidth().padding(16.dp)) {
            Text(Ru.reason(entry.reason), fontWeight = FontWeight.SemiBold)
            Text(Ru.delta(entry.deltaMs), style = glowDigits(34).copy(color = deltaColor(entry.deltaMs)))
            entry.parts.forEach { p ->
                Row(Modifier.fillMaxWidth()) {
                    Text(Ru.component(p.component), Modifier.weight(1f), fontSize = 13.sp)
                    Text(Ru.delta(p.deltaMs), color = deltaColor(p.deltaMs), fontSize = 13.sp)
                }
            }
        }
    }
}
