package app.scoretab

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun FlowButtons(content: @Composable () -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(5.dp)) { content() }
}

@Composable
internal fun Setup(busy: Boolean, create: (Match) -> Unit) {
    var first by rememberSaveable { mutableStateOf("") }
    var second by rememberSaveable { mutableStateOf("") }
    var map by rememberSaveable { mutableStateOf("") }
    var side by rememberSaveable { mutableStateOf(Side.T) }
    var length by rememberSaveable { mutableStateOf("24") }
    var switch by rememberSaveable { mutableStateOf("12") }
    var target by rememberSaveable { mutableStateOf("13") }
    var overtime by rememberSaveable { mutableStateOf(true) }
    var half by rememberSaveable { mutableStateOf("3") }
    var otSide by rememberSaveable { mutableStateOf<Side?>(null) }
    Text("Новый матч", style = MaterialTheme.typography.headlineMedium)
    OutlinedTextField(first, { first = it }, label = { Text("Команда 1") }, singleLine = true)
    OutlinedTextField(second, { second = it }, label = { Text("Команда 2") }, singleLine = true)
    Text("Команда 1 начинает за")
    FlowButtons { Side.entries.forEach { Pick(side == it, it.name) { side = it } } }
    Text("Команда 2 начинает за ${side.opposite()}")
    OutlinedTextField(map, { map = it }, label = { Text("Карта") }, singleLine = true)
    NumberField(length, { length = it }, "Раундов основного времени")
    NumberField(switch, { switch = it }, "Смена сторон после раунда")
    NumberField(target, { target = it }, "Побед для завершения")
    Row(verticalAlignment = Alignment.CenterVertically) {
        Checkbox(overtime, { overtime = it })
        Text("Овертайм при ничьей")
    }
    if (overtime) {
        NumberField(half, { half = it }, "Раундов в половине овертайма")
        Text("Команда 1 начинает овертайм за")
        FlowButtons {
            Pick(otSide == null, "Как во второй половине") { otSide = null }
            Side.entries.forEach { Pick(otSide == it, it.name) { otSide = it } }
        }
    }
    val rules = Rules(length.toIntOrNull() ?: 0, switch.toIntOrNull() ?: 0,
        target.toIntOrNull() ?: 0, if (overtime) half.toIntOrNull() ?: 0 else null, otSide)
    val error = runCatching { rules.validate() }.exceptionOrNull()?.message
    if (error != null) Text(error, color = amber)
    Button(enabled = !busy && first.isNotBlank() && second.isNotBlank() && error == null, onClick = {
        create(Match(teams = listOf(Team(1, first.trim()), Team(2, second.trim())),
            map = map.trim(), startingSide = side, rules = rules, createdAt = now()))
    }) { Text("Начать матч") }
}

@Composable
private fun NumberField(value: String, change: (String) -> Unit, title: String) {
    OutlinedTextField(value, { change(it.filter(Char::isDigit).take(3)) }, label = { Text(title) },
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true)
}

@Composable
private fun Pick(selected: Boolean, label: String, choose: () -> Unit) {
    if (selected) FilledTonalButton(onClick = choose, colors = ButtonDefaults.filledTonalButtonColors(
        containerColor = amber, contentColor = background), modifier = Modifier.heightIn(min = 55.dp)) { Text(label, textAlign = TextAlign.Center) }
    else OutlinedButton(onClick = choose, modifier = Modifier.heightIn(min = 55.dp)) { Text(label, textAlign = TextAlign.Center) }
}

@Composable
internal fun Scoreboard(value: Match, progress: Progress) {
    val position = value.position(if (progress.finished) value.next - 1 else value.next)
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        for ((index, team) in value.teams.withIndex()) {
            val score = if (index == 0) progress.score.first else progress.score.second
            val animated by animateFloatAsState(score.toFloat(), label = "score")
            Card(Modifier.weight(1f), colors = CardDefaults.cardColors(containerColor = raised)) {
                Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 18.dp),
                    horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(5.dp)) {
                    Text(team.name, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center,
                        style = MaterialTheme.typography.titleMedium)
                    Text(position.side(index).name, color = amber, fontWeight = FontWeight.Bold)
                    val highlight = kotlin.math.abs(score - animated).coerceIn(0f, 1f)
                    Text(score.toString(), fontSize = (68 + highlight * 4).sp, lineHeight = 76.sp,
                        fontWeight = FontWeight.Bold, color = text)
                }
            }
        }
    }
    if (progress.finished) Text("Матч завершён", color = muted)
}

@Composable
internal fun RoundFields(value: Match, number: Int, draft: Draft, change: (Draft) -> Unit) {
    val position = value.position(number)
    Text("Победитель", style = MaterialTheme.typography.titleMedium)
    FlowButtons {
        for ((index, team) in value.teams.withIndex()) {
            Pick(draft.winner == team.id, "Победа ${position.side(index)} · ${team.name}") {
                if (draft.winner != team.id) change(draft.copy(winner = team.id, reason = null))
            }
        }
    }
    val automatic = draft.reason == Reason.Explosion || draft.reason == Reason.Defuse
    Row(verticalAlignment = Alignment.CenterVertically) {
        Checkbox(draft.planted, { planted -> change(draft.copy(planted = planted, site = if (planted) draft.site else null)) }, enabled = !automatic)
        Text("Бомба установлена")
    }
    if (draft.winner != null) {
        val side = position.side(value.teamIndex(draft.winner))
        val reasons = when {
            side == Side.T -> listOf(Reason.Elimination, Reason.Explosion)
            draft.planted -> listOf(Reason.Defuse)
            else -> listOf(Reason.Elimination, Reason.Defuse, Reason.Timeout)
        }
        Text("Причина победы", style = MaterialTheme.typography.titleMedium)
        FlowButtons {
            reasons.forEach { reason ->
                Pick(draft.reason == reason, reason.label) {
                    change(draft.copy(reason = reason, planted = draft.planted || reason == Reason.Explosion || reason == Reason.Defuse))
                }
            }
        }
    }
    if (automatic) TextButton(onClick = { change(draft.copy(reason = null)) }) { Text("Сбросить причину") }
    var details by rememberSaveable(number) { mutableStateOf(draft.reason == Reason.Technical) }
    TextButton(onClick = { details = !details }) { Text("Дополнительные сведения") }
    if (details) {
        if (draft.winner != null) Pick(draft.reason == Reason.Technical, Reason.Technical.label) { change(draft.copy(reason = Reason.Technical)) }
        if (draft.planted) FlowButtons {
            listOf(null, "A", "B").forEach { site -> Pick(draft.site == site, site ?: "Без позиции") { change(draft.copy(site = site)) } }
        }
        OutlinedTextField(draft.note, { change(draft.copy(note = it)) },
            label = { Text(if (draft.reason == Reason.Technical) "Основание решения" else "Комментарий судьи") })
    }
}
