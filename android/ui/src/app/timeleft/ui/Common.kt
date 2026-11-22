package app.timeleft.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** Scrollable page with a back button and a title. */
@Composable
fun Page(title: String, onBack: (() -> Unit)?, content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            if (onBack != null) TextButton(onClick = onBack) { Text("← Назад") }
            Text(title, fontSize = 20.sp, fontWeight = FontWeight.SemiBold, color = Glow, modifier = Modifier.padding(start = 4.dp))
        }
        Spacer(Modifier.height(8.dp))
        content()
        Spacer(Modifier.height(24.dp))
    }
}

@Composable
fun SectionTitle(text: String) {
    Text(
        text, color = Glow, fontWeight = FontWeight.SemiBold, fontSize = 15.sp,
        modifier = Modifier.padding(top = 18.dp, bottom = 6.dp),
    )
}

@Composable
fun Hint(text: String) {
    Text(text, color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 12.sp)
}

/** Single-choice chips; `selected == null` means nothing chosen. */
@Composable
fun <T> ChoiceChips(options: List<Pair<T, String>>, selected: T?, onSelect: (T) -> Unit) {
    // FlowRow would wrap; three to four short options fit on one line.
    Row(Modifier.fillMaxWidth()) {
        options.forEach { (value, label) ->
            FilterChip(
                selected = value == selected,
                onClick = { onSelect(value) },
                label = { Text(label, fontSize = 13.sp) },
                modifier = Modifier.padding(end = 6.dp),
            )
        }
    }
}

@Composable
fun NumberField(value: String, onChange: (String) -> Unit, label: String, modifier: Modifier = Modifier.fillMaxWidth()) {
    val bad = value.isNotBlank() && value.toNumberOrNull() == null
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        singleLine = true,
        isError = bad,
        modifier = modifier.padding(vertical = 2.dp),
    )
}
