package app.scoretab

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.foundation.Image
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.text.font.FontWeight
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import kotlinx.serialization.encodeToString

internal val amber = Color(0xFFFFB02E)
internal val background = Color(0xFF050B12)
internal val surface = Color(0xFF0A1520)
internal val raised = Color(0xFF11212E)
internal val text = Color(0xFFF1F7FB)
internal val muted = Color(0xFF9CAEBB)
private val palette = darkColorScheme(primary = amber, onPrimary = background, background = background,
    onBackground = text, surface = surface, onSurface = text, surfaceVariant = raised,
    onSurfaceVariant = muted, outline = Color(0xFF526A79))

class MainActivity : ComponentActivity() {
    private val viewModel: ScoreViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge(statusBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT))
        setContent {
            val inter = FontFamily(Font(R.font.inter))
            val defaults = Typography()
            val typography = Typography(
                headlineMedium = defaults.headlineMedium.copy(fontFamily = inter, fontSize = 34.sp, lineHeight = 40.sp),
                headlineSmall = defaults.headlineSmall.copy(fontFamily = inter, fontSize = 28.sp, lineHeight = 34.sp),
                titleMedium = defaults.titleMedium.copy(fontFamily = inter, fontSize = 21.sp, lineHeight = 27.sp),
                bodyLarge = defaults.bodyLarge.copy(fontFamily = inter, fontSize = 19.sp, lineHeight = 25.sp),
                bodyMedium = defaults.bodyMedium.copy(fontFamily = inter, fontSize = 17.sp, lineHeight = 23.sp),
                bodySmall = defaults.bodySmall.copy(fontFamily = inter),
                labelLarge = defaults.labelLarge.copy(fontFamily = inter, fontSize = 17.sp, lineHeight = 23.sp, fontWeight = FontWeight.SemiBold),
                labelMedium = defaults.labelMedium.copy(fontFamily = inter),
                labelSmall = defaults.labelSmall.copy(fontFamily = inter),
            )
            MaterialTheme(colorScheme = palette, typography = typography) { ScoreApp(viewModel) }
        }
    }
}

@Composable
private fun ScoreApp(vm: ScoreViewModel) {
    val model by vm.state.collectAsStateWithLifecycle()
    var screen by rememberSaveable { mutableStateOf("home") }
    var settingsBack by rememberSaveable { mutableStateOf("home") }
    var services by rememberSaveable { mutableStateOf(false) }
    var selected by rememberSaveable { mutableStateOf<Long?>(null) }
    var deleteId by rememberSaveable { mutableStateOf<Long?>(null) }
    val context = LocalContext.current
    val chooseFolder = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        if (uri != null) vm.chooseExportFolder(uri)
    }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) vm.import(read = {
            checkNotNull(context.contentResolver.openInputStream(uri)).bufferedReader().use { it.readText() }
        }) { selected = null; screen = "match" }
    }
    val stored = model.current?.takeIf { selected == null || selected == it.id }
    val progress = remember(stored?.match) { stored?.match?.progress() }

    LaunchedEffect(screen, selected, model.busy, model.matches) {
        if ((screen == "match" || screen == "protocol") && stored == null && !model.busy && model.error.isEmpty()) {
            val candidate = selected ?: model.continueId
            if (candidate != null && model.matches.any { it.id == candidate }) vm.open(candidate) else screen = "home"
        }
    }
    BackHandler(screen != "home") {
        screen = when {
            screen == "settings" -> settingsBack
            screen == "protocol" && progress?.finished == false -> "match"
            else -> "home"
        }
    }

    fun openSettings() {
        settingsBack = screen
        screen = "settings"
    }

    fun open(entry: StoredMatch) {
        selected = entry.id
        services = false
        vm.open(entry.id)
        screen = if (entry.match.progress().finished) "protocol" else "match"
    }

    Surface(color = background, modifier = Modifier.fillMaxSize()) {
        Column(Modifier.safeDrawingPadding().imePadding().fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally) {
            BrandHeader(if (screen == "home") null else { { screen = "home" } })
            val screenScroll = rememberSaveable(screen, saver = ScrollState.Saver) { ScrollState(0) }
            Column(Modifier.widthIn(max = 800.dp).fillMaxWidth().verticalScroll(screenScroll)
                .padding(horizontal = 18.dp, vertical = 22.dp), verticalArrangement = Arrangement.spacedBy(15.dp),
                horizontalAlignment = Alignment.CenterHorizontally) {
                if (model.error.isNotEmpty()) Text(model.error, color = amber)
                when (screen) {
                    "home" -> {
                        Text("Судейский протокол", style = MaterialTheme.typography.headlineMedium)
                        Button(onClick = { screen = "setup" }, enabled = !model.busy,
                            modifier = Modifier.widthIn(max = 420.dp).fillMaxWidth().heightIn(min = 60.dp)) { Text("Новый матч") }
                        val active = model.matches.filter { !it.match.progress().finished }
                        val recent = active.firstOrNull { it.id == model.continueId } ?: active.firstOrNull()
                        if (recent != null) OutlinedButton(onClick = { open(recent) }) {
                            Text("Продолжить · ${recent.match.teams.joinToString(" — ") { it.name }}")
                        }
                        OutlinedButton(onClick = { screen = "history" }) { Text("История матчей") }
                        OutlinedButton(onClick = ::openSettings) { Text("Настройки") }
                        OutlinedButton(onClick = { launcher.launch(arrayOf("application/json", "text/plain", "application/octet-stream")) },
                            enabled = !model.busy) { Text("Импортировать JSON") }
                    }
                    "setup" -> Setup(model.busy) { match ->
                        vm.create(match) { selected = null; screen = "match" }
                    }
                    "settings" -> {
                        Text("Настройки", style = MaterialTheme.typography.headlineMedium)
                        Text("Папка для экспорта", style = MaterialTheme.typography.titleMedium)
                        Text(model.exportFolder ?: "Папка не выбрана")
                        Text("CSV, Excel и JSON сохраняются сюда автоматически.", color = muted)
                        Button(onClick = { chooseFolder.launch(null) }, enabled = !model.busy,
                            modifier = Modifier.heightIn(min = 55.dp)) { Text("Выбрать папку") }
                        OutlinedButton(onClick = { screen = settingsBack }) { Text("Назад") }
                    }
                    "history" -> {
                        Text("История матчей", style = MaterialTheme.typography.headlineMedium)
                        if (model.matches.isEmpty()) Text("Матчей пока нет.")
                        for (entry in model.matches) {
                            val game = entry.match
                            val status = game.progress()
                            Card(colors = CardDefaults.cardColors(containerColor = raised)) {
                                Column(Modifier.fillMaxWidth().padding(12.dp)) {
                                    Text(game.scoreText(status), style = MaterialTheme.typography.titleMedium)
                                    Text(listOf(game.map, date(game.createdAt),
                                        if (status.finished) "Завершён" else "Продолжается").filter(String::isNotBlank).joinToString(" · "), color = muted)
                                    FlowButtons {
                                        TextButton(onClick = { open(entry) }) { Text("Открыть") }
                                        TextButton(onClick = { deleteId = entry.id }) { Text("Удалить") }
                                    }
                                }
                            }
                        }
                    }
                    "match", "protocol" -> if (stored != null && progress != null) {
                        Scoreboard(stored.match, progress)
                        if (model.notice.isNotEmpty()) Text(model.notice, color = amber)
                        if (screen == "protocol" || progress.finished) {
                            Protocol(vm, model, stored, progress, services, { services = it }, openSettings = ::openSettings) { screen = "match" }
                        } else {
                            LiveRound(vm, model, stored) { screen = "protocol" }
                        }
                    } else if (model.busy || model.error.isEmpty()) {
                        CircularProgressIndicator()
                    }
                }
            }
        }
    }
    deleteId?.let { id ->
        Confirm("Удалить матч и его протокол?", dismiss = { deleteId = null }, apply = { deleteId = null; vm.delete(id) })
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun BrandHeader(home: (() -> Unit)?) {
    Surface(color = surface, modifier = Modifier.fillMaxWidth()) {
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            Row(Modifier.widthIn(max = 800.dp).fillMaxWidth().heightIn(min = 72.dp).padding(horizontal = 12.dp),
                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (home != null) {
                    IconButton(onClick = home) {
                        Text("‹", modifier = Modifier.clearAndSetSemantics { contentDescription = "Главная" },
                            fontSize = 30.sp, color = text)
                    }
                }
                Surface(color = raised, shape = RoundedCornerShape(12.dp),
                    border = BorderStroke(1.dp, Color(0xFF526A79)), modifier = Modifier.size(50.dp)) {
                    Image(painterResource(R.drawable.scoretab_emblem), contentDescription = null,
                        modifier = Modifier.padding(2.dp))
                }
                FlowRow(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(7.dp)) {
                    Text(buildAnnotatedString {
                        append("ScoreTab ")
                        withStyle(SpanStyle(color = amber)) { append("CS2") }
                    }, fontSize = 19.sp, lineHeight = 24.sp, fontWeight = FontWeight.Bold,
                        modifier = Modifier.align(Alignment.CenterVertically))
                    Text("by gearbarium", fontSize = 11.sp, color = muted,
                        modifier = Modifier.align(Alignment.CenterVertically))
                }
            }
        }
    }
}

@Composable
private fun LiveRound(vm: ScoreViewModel, model: Model, stored: StoredMatch, protocol: () -> Unit) {
    val value = stored.match
    val number = value.next
    val position = value.position(number)
    Text("Раунд $number", style = MaterialTheme.typography.headlineMedium)
    if (position.period > 0) Text("Овертайм ${position.period} · Раунд ${position.number}", color = amber)
    var raw by rememberSaveable(stored.id, number) { mutableStateOf(protocolJson.encodeToString(value.draft)) }
    val draft = remember(raw) { protocolJson.decodeFromString<Draft>(raw) }
    Card(Modifier.widthIn(max = 620.dp).fillMaxWidth(), colors = CardDefaults.cardColors(containerColor = surface)) {
        Column(Modifier.fillMaxWidth().padding(18.dp), verticalArrangement = Arrangement.spacedBy(9.dp)) {
            RoundFields(value, number, draft) {
                raw = protocolJson.encodeToString(it)
                vm.saveDraft(stored.id, number, it)
            }
        }
    }
    val valid = runCatching { value.normalize(number, draft) }
    if (draft.winner != null && draft.reason != null) valid.exceptionOrNull()?.message?.let { Text(it, color = amber) }
    Button(onClick = { vm.commit(stored.id, number, draft) }, enabled = valid.isSuccess && !model.busy,
        modifier = Modifier.widthIn(max = 620.dp).fillMaxWidth().heightIn(min = 62.dp)) { Text("Следующий раунд") }
    OutlinedButton(onClick = protocol, modifier = Modifier.widthIn(max = 620.dp).fillMaxWidth().heightIn(min = 54.dp)) { Text("Протокол матча") }
}

@Composable
private fun Protocol(vm: ScoreViewModel, model: Model, stored: StoredMatch, progress: Progress,
    services: Boolean, setServices: (Boolean) -> Unit, openSettings: () -> Unit, back: () -> Unit) {
    val value = stored.match
    val clipboard = LocalClipboardManager.current
    val context = LocalContext.current
    fun exportExcel() {
        vm.export(value.createdAt, "xlsx", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet") { stream ->
            val parts = context.assets.open("xlsx-parts.json").bufferedReader().use {
                protocolJson.decodeFromString<Map<String, String>>(it.readText())
            }
            value.writeExcel(stream, progress, parts)
        }
    }
    var editNumber by rememberSaveable { mutableIntStateOf(0) }
    var editDraft by rememberSaveable { mutableStateOf("") }
    var voidOpen by rememberSaveable { mutableStateOf(false) }

    Text("Протокол матча", style = MaterialTheme.typography.headlineSmall)
    FlowButtons {
        if (!progress.finished) OutlinedButton(onClick = back) { Text("К текущему раунду") }
        OutlinedButton(onClick = { clipboard.setText(AnnotatedString(value.scoreText(progress))) }) { Text("Копировать счёт") }
        OutlinedButton(onClick = { clipboard.setText(AnnotatedString(value.protocolText(progress))) }) { Text("Копировать протокол") }
    }
    for ((index, round) in value.rounds.withIndex()) {
        val position = value.position(round.number)
        val previous = if (index == 0) null else value.position(round.number - 1)
        if (previous != null && position.period != previous.period) Text("Овертайм ${position.period}", color = amber)
        if (previous != null && position.firstSide != previous.firstSide) Text("Смена сторон", color = amber)
        val fields = value.roundFields(round, progress.scores[index])
        Card(colors = CardDefaults.cardColors(containerColor = raised)) {
            Column(Modifier.fillMaxWidth().padding(12.dp)) {
                Text("Раунд ${fields[0]} · ${fields[5]}", style = MaterialTheme.typography.titleMedium)
                Text("${fields[1]} (${fields[2]}) · ${fields[4]} · Плент: ${fields[3]}")
                if (fields[6].isNotEmpty()) Text("Позиция ${fields[6]}")
                if (fields[7].isNotEmpty()) Text(fields[7])
                TextButton(onClick = {
                    editNumber = round.number
                    editDraft = protocolJson.encodeToString(round.result)
                }) { Text("Редактировать") }
            }
        }
    }
    OutlinedButton(onClick = { setServices(!services) }) { Text("Экспорт и служебные действия") }
    if (services) {
        val configured = model.exportFolder != null
        OutlinedButton(onClick = openSettings) {
            Text(if (configured) "Изменить папку экспорта" else "Настроить папку экспорта")
        }
        FlowButtons {
            OutlinedButton(enabled = configured && !model.busy, onClick = {
                vm.export(value.createdAt, "csv", "text/csv") {
                    it.write(value.csv(progress).toByteArray(Charsets.UTF_8))
                }
            }) { Text("CSV") }
            OutlinedButton(enabled = configured && !model.busy, onClick = ::exportExcel) { Text("Excel (.xlsx)") }
            OutlinedButton(enabled = configured && !model.busy, onClick = {
                vm.export(value.createdAt, "json", "application/json") {
                    it.write(protocolJson.encodeToString(value).toByteArray(Charsets.UTF_8))
                }
            }) { Text("JSON") }
            OutlinedButton(enabled = !model.busy && value.revisions.lastOrNull()?.after != null,
                onClick = { vm.undo(stored.id) }) { Text("Восстановить прошлую редакцию") }
            OutlinedButton(enabled = !model.busy && value.rounds.isNotEmpty(),
                onClick = { voidOpen = true }) { Text("Аннулировать последний раунд") }
        }
        for (revision in value.revisions.asReversed()) {
            Text("${date(revision.savedAt)} · Раунд ${revision.before.number} · ${if (revision.after == null) "Аннулирован" else "Исправлен"}", color = muted)
        }
    }
    if (editNumber > 0) {
        val draft = remember(editDraft) { protocolJson.decodeFromString<Draft>(editDraft) }
        val invalid = runCatching { value.normalize(editNumber, draft) }.exceptionOrNull()?.message
        AlertDialog(onDismissRequest = { if (!model.busy) editNumber = 0 },
            title = { Text("Редактировать раунд $editNumber") },
            text = {
                Column(Modifier.verticalScroll(rememberScrollState())) {
                    RoundFields(value, editNumber, draft) { editDraft = protocolJson.encodeToString(it) }
                    (invalid ?: model.error.takeIf { it.isNotBlank() })?.let { Text(it, color = amber) }
                }
            },
            confirmButton = {
                TextButton(enabled = !model.busy && invalid == null,
                    onClick = { vm.edit(stored.id, editNumber, draft) { editNumber = 0 } }) { Text("Сохранить") }
            },
            dismissButton = { TextButton(enabled = !model.busy, onClick = { editNumber = 0 }) { Text("Отмена") } })
    }
    if (voidOpen) Confirm("Аннулировать последний раунд? Исходная запись останется в истории правок.",
        dismiss = { voidOpen = false }, apply = { voidOpen = false; vm.voidLast(stored.id) })
}

@Composable
private fun Confirm(message: String, dismiss: () -> Unit, apply: () -> Unit) {
    AlertDialog(onDismissRequest = dismiss, title = { Text("Подтверждение") }, text = { Text(message) },
        confirmButton = { TextButton(onClick = apply) { Text("Подтвердить") } },
        dismissButton = { TextButton(onClick = dismiss) { Text("Отмена") } })
}

private fun date(seconds: Long): String = runCatching {
    DateTimeFormatter.ofPattern("dd.MM.yyyy").withZone(ZoneId.systemDefault()).format(Instant.ofEpochSecond(seconds))
}.getOrDefault("—")
