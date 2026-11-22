package app.timeleft.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.timeleft.core.countdown
import kotlinx.coroutines.delay

@Composable
fun MainScreen(model: AppModel) {
    val status = model.status ?: return
    var now by remember { mutableLongStateOf(model.now()) }
    LaunchedEffect(model.clock.shiftMs) {
        var lastRefresh = System.currentTimeMillis()
        while (true) {
            now = model.now()
            if (System.currentTimeMillis() - lastRefresh > 60_000) {
                model.refresh()
                lastRefresh = System.currentTimeMillis()
            }
            delay(50)
        }
    }
    val c = countdown(now, status.endAtMs, model.clock.utcOffsetMinutes(now))
    val years = c.years.toLong()
    val days = c.days.toLong()

    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 24.dp, vertical = 16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text("TIME LEFT", color = GlowDim, fontFamily = FontFamily.Monospace, letterSpacing = 6.sp, fontSize = 13.sp)
        if (model.clock.shiftMs != 0L) TestTimeBanner(model)
        Spacer(Modifier.height(16.dp))
        if (c.expired) {
            Text("00", style = glowDigits(72))
            Text(
                "Отсчёт дошёл до медианы. Каждый следующий день — сверх ожиданий.",
                textAlign = TextAlign.Center, color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            Row(verticalAlignment = Alignment.Bottom) {
                Text("$years", style = glowDigits(64))
                Text(" ${Ru.yearsWord(years)}", color = Glow, fontSize = 22.sp, modifier = Modifier.padding(bottom = 12.dp))
            }
            Row(verticalAlignment = Alignment.Bottom) {
                Text("$days", style = glowDigits(48))
                Text(" ${Ru.daysWord(days)}", color = Glow, fontSize = 20.sp, modifier = Modifier.padding(bottom = 8.dp))
            }
        }
        Spacer(Modifier.height(12.dp))
        CountdownDial(c, Modifier.fillMaxWidth().widthIn(max = 340.dp).padding(horizontal = 12.dp))
        Text(
            "%02d:%02d:%02d".format(c.hours.toInt(), c.minutes.toInt(), c.seconds.toInt()),
            style = glowDigits(26),
        )
        Spacer(Modifier.height(12.dp))
        val e = status.estimate
        Text(
            "Половина людей с похожим профилем живёт дольше. Типичный диапазон: " +
                "${Ru.yearsDecimal(e.lower)} – ${Ru.yearsDecimal(e.upper)}.",
            textAlign = TextAlign.Center, fontSize = 13.sp, color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (status.bankedMs != 0L) {
            Spacer(Modifier.height(6.dp))
            Text("Привычки за всё время: ${Ru.delta(status.bankedMs)}", color = deltaColor(status.bankedMs), fontSize = 14.sp)
        }
        model.lastEntry?.let { entry ->
            Spacer(Modifier.height(6.dp))
            Text(
                "${Ru.reason(entry.reason)}: ${Ru.delta(entry.deltaMs)}",
                color = deltaColor(entry.deltaMs), fontSize = 14.sp, textAlign = TextAlign.Center,
            )
        }
        Spacer(Modifier.height(20.dp))
        val checkedToday = status.lastCheckIn?.date == model.today()
        Button(onClick = { model.screen = Screen.CheckIn }, modifier = Modifier.fillMaxWidth()) {
            Text(if (checkedToday) "Изменить отметку за сегодня" else "Отметить день")
        }
        Spacer(Modifier.height(8.dp))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = { model.screen = Screen.Explain }, Modifier.weight(1f)) { Text("Почему") }
            OutlinedButton(onClick = { model.screen = Screen.Journal }, Modifier.weight(1f)) { Text("Журнал") }
            OutlinedButton(onClick = { model.screen = Screen.Settings }, Modifier.weight(1f)) { Text("Ещё") }
        }
        Spacer(Modifier.height(16.dp))
        Text(Ru.DISCLAIMER, fontSize = 11.sp, color = GlowDim, textAlign = TextAlign.Center)
    }
}

@Composable
fun TestTimeBanner(model: AppModel) {
    Surface(color = MaterialTheme.colorScheme.surfaceVariant, shape = MaterialTheme.shapes.small, modifier = Modifier.padding(top = 8.dp)) {
        Row(Modifier.padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("Тестовое время: ${Ru.delta(model.clock.shiftMs)}", fontSize = 13.sp, color = Loss)
            TextButton(onClick = { model.shiftTime(0) }) { Text("Сбросить") }
        }
    }
}
