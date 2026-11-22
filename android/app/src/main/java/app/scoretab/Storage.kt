package app.scoretab

import android.app.Application
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import androidx.room.*
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.*
import kotlinx.serialization.encodeToString
import java.io.IOException
import java.io.OutputStream

private val Context.settings by preferencesDataStore("scoretab_settings")
private val lastMatch = longPreferencesKey("last_match")
private val exportFolderKey = stringPreferencesKey("export_folder")
private const val folderGrant = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION

@Entity(tableName = "matches")
data class MatchRow(@PrimaryKey(autoGenerate = true) val id: Long = 0, val protocol: String)

@Dao
interface MatchDao {
    @Query("SELECT * FROM matches ORDER BY id DESC")
    suspend fun list(): List<MatchRow>

    @Insert
    suspend fun insert(value: MatchRow): Long

    @Query("UPDATE matches SET protocol=:after WHERE id=:id AND protocol=:before")
    suspend fun update(id: Long, before: String, after: String): Int

    @Query("DELETE FROM matches WHERE id=:id")
    suspend fun delete(id: Long)
}

@Database(entities = [MatchRow::class], version = 2, exportSchema = false)
abstract class MatchDatabase : RoomDatabase() {
    abstract fun matches(): MatchDao
}

private val migrate1To2 = object : Migration(1, 2) {
    override fun migrate(db: SupportSQLiteDatabase) {
        db.execSQL("CREATE TABLE matches_v2(id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, protocol TEXT NOT NULL)")
        db.execSQL("INSERT INTO matches_v2(id, protocol) SELECT id, protocol FROM matches")
        db.execSQL("DROP TABLE matches")
        db.execSQL("ALTER TABLE matches_v2 RENAME TO matches")
    }
}

data class StoredMatch(val id: Long, val match: Match, val raw: String)
data class Model(
    val matches: List<StoredMatch> = emptyList(),
    val current: StoredMatch? = null,
    val continueId: Long? = null,
    val error: String = "",
    val notice: String = "",
    val busy: Boolean = true,
    val exportFolder: String? = null,
)

fun now() = System.currentTimeMillis() / 1000

class ScoreViewModel(application: Application) : AndroidViewModel(application) {
    private val db = Room.databaseBuilder(application, MatchDatabase::class.java, "scoretab.sqlite3")
        .addMigrations(migrate1To2).build()
    private val settings = application.settings
    private val resolver = application.contentResolver
    private val requests = Channel<suspend () -> Unit>(Channel.UNLIMITED)
    private val mutableState = MutableStateFlow(Model())
    val state = mutableState.asStateFlow()

    init {
        submit {
            reload()
            val preferences = settings.data.first()
            val uri = preferences[exportFolderKey]?.let(Uri::parse)
            mutableState.update { it.copy(continueId = preferences[lastMatch], exportFolder = uri?.let(::folderName)) }
        }
        viewModelScope.launch(Dispatchers.IO) {
            for (request in requests) {
                mutableState.update { it.copy(busy = true, error = "") }
                try {
                    request()
                } catch (error: CancellationException) {
                    throw error
                } catch (error: Exception) {
                    mutableState.update { it.copy(error = error.message ?: "Ошибка хранения.") }
                } finally {
                    mutableState.update { it.copy(busy = false) }
                }
            }
        }
    }

    private fun submit(action: suspend () -> Unit) { requests.trySend(action) }

    private suspend fun reload() {
        val invalid = mutableListOf<Long>()
        val matches = db.matches().list().mapNotNull { row ->
            try {
                val match = protocolJson.decodeFromString<Match>(row.protocol)
                match.progress()
                StoredMatch(row.id, match, row.protocol)
            } catch (_: IllegalArgumentException) {
                invalid += row.id
                null
            }
        }
        mutableState.update { it.copy(matches = matches, error = if (invalid.isEmpty()) "" else
            "Не удалось прочитать матчи: ${invalid.joinToString()}. Записи сохранены в базе.") }
    }

    fun open(id: Long) = submit {
        select(mutableState.value.matches.firstOrNull { it.id == id } ?: error("Матч не найден."))
    }

    private suspend fun select(match: StoredMatch) {
        mutableState.update { it.copy(current = match, notice = "", continueId = match.id) }
        try {
            settings.edit { it[lastMatch] = match.id }
        } catch (_: IOException) {
            mutableState.update { it.copy(notice = "Не удалось сохранить выбор последнего матча.") }
        }
    }

    fun create(match: Match, done: () -> Unit) = submit {
        insert(match)
        withContext(Dispatchers.Main) { done() }
    }

    fun import(read: () -> String, done: () -> Unit) = submit {
        insert(protocolJson.decodeFromString<Match>(read()))
        withContext(Dispatchers.Main) { done() }
    }

    private suspend fun insert(match: Match) {
        match.progress()
        val raw = protocolJson.encodeToString(match)
        val id = db.matches().insert(MatchRow(protocol = raw))
        val saved = StoredMatch(id, match, raw)
        mutableState.update { it.copy(matches = listOf(saved) + it.matches) }
        select(saved)
    }

    fun delete(id: Long) = submit {
        db.matches().delete(id)
        reload()
        mutableState.update { it.copy(current = it.current?.takeUnless { current -> current.id == id }) }
    }

    fun saveDraft(id: Long, number: Int, draft: Draft) = submit {
        val before = mutableState.value.current
        if (before?.id == id && before.match.next == number && !before.match.progress().finished) {
            save(before, before.match.copy(draft = draft))
        }
    }

    fun commit(id: Long, number: Int, draft: Draft) = submit {
        val before = current(id)
        if (before.match.next == number) {
            val after = before.match.copy(draft = draft).commit(number, now())
            save(before, after)
            val oldSide = after.position(number).firstSide
            val newSide = after.position(after.next).firstSide
            mutableState.update { it.copy(notice = if (oldSide != newSide && !after.progress().finished)
                "Смена сторон · ${after.teams[0].name} $newSide · ${after.teams[1].name} ${newSide.opposite()}" else "") }
        }
    }

    fun edit(id: Long, number: Int, draft: Draft, done: () -> Unit) = submit {
        val before = current(id)
        save(before, before.match.edit(number, draft, now()))
        withContext(Dispatchers.Main) { done() }
    }

    fun undo(id: Long) = submit {
        val before = current(id)
        save(before, before.match.undoEdit(now()))
    }

    fun voidLast(id: Long) = submit {
        val before = current(id)
        save(before, before.match.voidLast(now()))
    }

    fun chooseExportFolder(uri: Uri) = submit {
        val previous = settings.data.first()[exportFolderKey]?.let(Uri::parse)
        resolver.takePersistableUriPermission(uri, folderGrant)
        try {
            settings.edit { it[exportFolderKey] = uri.toString() }
        } catch (error: Exception) {
            if (uri != previous) runCatching { resolver.releasePersistableUriPermission(uri, folderGrant) }
            throw error
        }
        if (previous != null && previous != uri) {
            runCatching { resolver.releasePersistableUriPermission(previous, folderGrant) }
        }
        mutableState.update { it.copy(exportFolder = folderName(uri), notice = "") }
    }

    fun export(matchTime: Long, extension: String, mimeType: String, write: (OutputStream) -> Unit) = submit {
        val folder = settings.data.first()[exportFolderKey]?.let(Uri::parse)
            ?: error("Сначала выберите папку для экспорта в настройках.")
        val name = "ScoreTab-$matchTime-${System.currentTimeMillis()}.$extension"
        val file = try {
            checkNotNull(DocumentsContract.createDocument(resolver, documentUri(folder), mimeType, name))
        } catch (error: Exception) {
            throw IOException("Не удалось открыть папку экспорта. Выберите её заново в настройках.", error)
        }
        try {
            checkNotNull(resolver.openOutputStream(file, "wt")).use(write)
        } catch (error: Exception) {
            runCatching { DocumentsContract.deleteDocument(resolver, file) }
            throw error
        }
        mutableState.update { it.copy(notice = "Сохранено: ${displayName(file) ?: name}") }
    }

    private fun documentUri(tree: Uri): Uri =
        DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))

    private fun folderName(tree: Uri): String =
        displayName(documentUri(tree)) ?: DocumentsContract.getTreeDocumentId(tree)

    private fun displayName(uri: Uri): String? = runCatching {
        resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
            if (it.moveToFirst()) it.getString(0) else null
        }
    }.getOrNull()

    private fun current(id: Long): StoredMatch =
        mutableState.value.current?.takeIf { it.id == id } ?: error("Матч закрыт.")

    private suspend fun save(before: StoredMatch, after: Match) {
        if (before.match == after) return
        val raw = protocolJson.encodeToString(after)
        check(db.matches().update(before.id, before.raw, raw) == 1) {
            "Матч был изменён. Откройте его повторно."
        }
        val saved = StoredMatch(before.id, after, raw)
        mutableState.update { state ->
            state.copy(current = saved, matches = state.matches.map { if (it.id == saved.id) saved else it })
        }
    }
}
