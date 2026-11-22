package app.scoretab

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

val protocolJson = Json { encodeDefaults = true; prettyPrint = true }

@Serializable
enum class Side {
    T, CT;
    fun opposite() = if (this == T) CT else T
}

@Serializable
enum class Reason(val label: String) {
    Elimination("Уничтожение"), Explosion("Взрыв"), Defuse("Дефуз"),
    Timeout("Время"), Technical("Техническое решение")
}

@Serializable
data class Team(val id: Long, val name: String)

@Serializable
data class Rules(
    val regulation: Int = 24,
    @SerialName("switch_after") val switchAfter: Int = 12,
    @SerialName("win_target") val winTarget: Int = 13,
    @SerialName("overtime_half") val overtimeHalf: Int? = 3,
    @SerialName("overtime_start") val overtimeStart: Side? = null,
) {
    fun validate() {
        require(regulation in 2..200 && switchAfter in 1 until regulation &&
            winTarget in regulation / 2 + 1..regulation &&
            (overtimeHalf == null || overtimeHalf in 1..50)) {
            "Проверьте лимит раундов, границу половин и порог победы."
        }
    }

    fun position(number: Int, startingSide: Side): Position {
        if (number <= regulation) {
            return Position(0, number, if (number > switchAfter) startingSide.opposite() else startingSide)
        }
        val half = overtimeHalf ?: 1
        val offset = number - regulation - 1
        val local = offset % (half * 2) + 1
        val start = overtimeStart ?: startingSide.opposite()
        return Position(offset / (half * 2) + 1, local, if (local > half) start.opposite() else start)
    }
}

data class Position(val period: Int, val number: Int, val firstSide: Side) {
    fun side(index: Int) = if (index == 0) firstSide else firstSide.opposite()
}

@Serializable
data class Draft(
    val winner: Long? = null,
    val reason: Reason? = null,
    val planted: Boolean = false,
    val site: String? = null,
    val note: String = "",
) {
    fun complete(side: Side): Draft {
        val selected = requireNotNull(reason) { "Выберите причину победы." }
        require(
            when (selected) {
                Reason.Explosion -> side == Side.T
                Reason.Defuse, Reason.Timeout -> side == Side.CT
                else -> true
            }
        ) { "Причина несовместима со стороной победителя." }
        require(!(side == Side.CT && planted && selected in listOf(Reason.Elimination, Reason.Timeout))) {
            "Снимите отметку плента или выберите дефуз."
        }
        require(selected != Reason.Technical || note.isNotBlank()) {
            "Укажите основание технического решения."
        }
        val bomb = planted || selected == Reason.Explosion || selected == Reason.Defuse
        require(site == null || (bomb && site in listOf("A", "B"))) {
            "Место установки допустимо только при пленте. Выберите A или B."
        }
        return copy(planted = bomb)
    }
}

@Serializable
data class Round(val number: Int, @SerialName("saved_at") val savedAt: Long, val result: Draft)

@Serializable
data class Revision(val before: Round, val after: Round?, @SerialName("saved_at") val savedAt: Long)

@Serializable
data class Match(
    val version: Int = 1,
    val teams: List<Team>,
    val map: String,
    @SerialName("starting_side") val startingSide: Side,
    val rules: Rules,
    @SerialName("created_at") val createdAt: Long,
    val rounds: List<Round> = emptyList(),
    val revisions: List<Revision> = emptyList(),
    val draft: Draft = Draft(),
) {
    val next get() = rounds.size + 1
    fun position(number: Int) = rules.position(number, startingSide)

    fun teamIndex(id: Long?): Int {
        val index = teams.indexOfFirst { it.id == id }
        require(index >= 0) { "Выберите победителя." }
        return index
    }

    fun normalize(number: Int, draft: Draft) = draft.complete(position(number).side(teamIndex(draft.winner)))

    fun progress(): Progress {
        rules.validate()
        require(version == 1 && teams.size == 2 && teams[0].id != teams[1].id &&
            teams.all { it.id in 0..4294967295L && it.name.isNotBlank() }) {
            "Некорректная версия протокола или команды."
        }
        require((draft.winner == null || teams.any { it.id == draft.winner }) &&
            (draft.site == null || (draft.planted && draft.site in listOf("A", "B"))) &&
            (draft.winner != null || draft.reason == null)) {
            "Некорректный черновик раунда."
        }
        for (revision in revisions) {
            val before = revision.before
            val after = revision.after
            require(before.number > 0 && normalize(before.number, before.result) == before.result &&
                (after == null || (after.number == before.number && normalize(after.number, after.result) == after.result))) {
                "Некорректная запись в истории правок."
            }
        }
        val scores = mutableListOf<Score>()
        var finished = false
        val total = IntArray(2)
        val period = IntArray(2)
        rounds.forEachIndexed { index, round ->
            require(!finished) {
                "После пересчёта матч заканчивается раньше. Следующие записи сохранены. Отмените исправление или сначала аннулируйте последние раунды."
            }
            require(round.number == index + 1 && normalize(round.number, round.result) == round.result) {
                "Некорректная запись раунда."
            }
            val winner = teamIndex(round.result.winner)
            total[winner]++
            scores += Score(total[0], total[1])
            val position = position(round.number)
            finished = if (position.period == 0) {
                total[winner] >= rules.winTarget || (round.number == rules.regulation &&
                    (total[0] != total[1] || rules.overtimeHalf == null))
            } else {
                val half = requireNotNull(rules.overtimeHalf) { "Овертайм отключён." }
                if (position.number == 1) period.fill(0)
                ++period[winner] > half
            }
        }
        return Progress(scores, finished)
    }

    fun commit(expected: Int, time: Long): Match {
        require(next == expected && !progress().finished) { "Раунд уже закрыт." }
        return copy(rounds = rounds + Round(expected, time, normalize(expected, draft)), draft = Draft())
    }

    fun edit(number: Int, result: Draft, time: Long): Match {
        val before = requireNotNull(rounds.getOrNull(number - 1)) { "Раунд не найден." }
        val after = before.copy(result = normalize(number, result))
        return copy(rounds = rounds.toMutableList().apply { this[number - 1] = after },
            revisions = revisions + Revision(before, after, time)).also { it.progress() }
    }

    fun voidLast(time: Long): Match {
        val before = requireNotNull(rounds.lastOrNull()) { "Нет завершённых раундов." }
        return copy(rounds = rounds.dropLast(1), revisions = revisions + Revision(before, null, time), draft = Draft())
    }

    fun undoEdit(time: Long): Match {
        val revision = requireNotNull(revisions.lastOrNull()) { "Нет исправлений." }
        val after = requireNotNull(revision.after) { "Последнее действие — аннулирование." }
        require(rounds.getOrNull(after.number - 1) == after) { "Раунд изменился после исправления." }
        return edit(after.number, revision.before.result, time)
    }
}

data class Score(val first: Int, val second: Int) {
    override fun toString() = "$first:$second"
}

data class Progress(val scores: List<Score>, val finished: Boolean) {
    val score get() = scores.lastOrNull() ?: Score(0, 0)
}

fun Match.scoreText(progress: Progress) = "${teams[0].name} ${progress.score} ${teams[1].name}"

fun Match.roundFields(round: Round, score: Score): List<String> {
    val winner = teamIndex(round.result.winner)
    return listOf(round.number.toString(), teams[winner].name, position(round.number).side(winner).name,
        if (round.result.planted) "Да" else "Нет", requireNotNull(round.result.reason).label,
        score.toString(), round.result.site.orEmpty(), round.result.note)
}

fun Match.protocolText(progress: Progress) = buildString {
    append(scoreText(progress))
    rounds.zip(progress.scores).forEach { (round, score) ->
        append('\n')
        append(roundFields(round, score).filter { it.isNotEmpty() }.joinToString(" · "))
    }
}

fun Match.csv(progress: Progress) = buildString {
    append("\uFEFFРаунд,Победитель,Сторона,Плент,Причина,Счёт,Позиция,Комментарий\r\n")
    rounds.zip(progress.scores).forEach { (round, score) ->
        append(roundFields(round, score).joinToString(",") { field ->
            val prefix = if (field.firstOrNull() in listOf('=', '+', '-', '@', '\t', '\r', '\n')) "'" else ""
            "\"$prefix${field.replace("\"", "\"\"")}\""
        })
        append("\r\n")
    }
}
