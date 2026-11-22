package app.scoretab

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import org.junit.Assert.*
import org.junit.Test
import java.io.ByteArrayOutputStream
import java.util.zip.ZipInputStream

class ProtocolTest {
    @Serializable
    data class Case(val id: String, val rules: Rules = Rules(), val winners: List<Int>, val score: List<Int>,
        val finished: Boolean, val period: Int, val local: Int, val side: Side,
        @SerialName("status_position") val statusPosition: Int)

    @Serializable
    data class Event(val side: Side, val reason: Reason, val planted: Boolean, val normalized: Boolean?)

    private fun resource(name: String) = requireNotNull(javaClass.getResourceAsStream("/$name")).bufferedReader().use { it.readText() }

    private fun game(rules: Rules = Rules()) =
        Match(teams = listOf(Team(41, "Кривичи"), Team(72, "Матрица")), map = "Mirage",
            startingSide = Side.T, rules = rules, createdAt = 1)

    private fun winner(id: Long) = Draft(winner = id, reason = Reason.Elimination)

    @Test
    fun sharedScoringCases() {
        val cases = protocolJson.decodeFromString<List<Case>>(resource("test-cases.json"))
        for (case in cases) {
            var value = game(case.rules)
            for (index in case.winners) {
                value = value.copy(draft = winner(value.teams[index - 1].id)).commit(value.next, 1)
            }
            val progress = value.progress()
            assertEquals(case.id, Score(case.score[0], case.score[1]), progress.score)
            assertEquals(case.id, case.finished, progress.finished)
            assertEquals(case.id, Position(case.period, case.local, case.side), value.position(case.statusPosition))
        }
    }

    @Test
    fun sharedEventCases() {
        val cases = protocolJson.decodeFromString<List<Event>>(resource("round-events.json"))
        for (case in cases) {
            val draft = Draft(reason = case.reason, planted = case.planted)
            assertEquals(case.normalized, runCatching { draft.complete(case.side).planted }.getOrNull())
        }
    }

    @Test
    fun editsAndReplayPreserveHistory() {
        var value = game().copy(draft = winner(41)).commit(1, 1)
        assertTrue(runCatching { value.commit(1, 1) }.isFailure)
        value = value.copy(draft = winner(72)).commit(2, 2).edit(1, winner(72), 3)
        assertEquals(listOf(Score(0, 1), Score(0, 2)), value.progress().scores)
        value = value.undoEdit(4)
        assertEquals(Score(1, 1), value.progress().score)
        value = value.voidLast(5)
        assertEquals(2, value.next)
        assertEquals(3, value.revisions.size)
        assertNull(value.revisions.last().after)
    }

    @Test
    fun conflictingEditLeavesRecordsIntact() {
        var value = game()
        repeat(25) { index -> value = value.copy(draft = winner(if (index % 2 == 0) 41 else 72)).commit(value.next, 1) }
        assertTrue(runCatching { value.edit(2, winner(41), 2) }.isFailure)
        assertEquals(25, value.rounds.size)
        assertTrue(value.revisions.isEmpty())
    }

    @Test
    fun crossPlatformJsonPreservesDraft() {
        val value = protocolJson.decodeFromString<Match>(resource("example-match.json"))
        assertEquals(Score(1, 0), value.progress().score)
        assertEquals(72L, value.draft.winner)
        assertEquals("B", value.draft.site)
        assertEquals(value, protocolJson.decodeFromString<Match>(protocolJson.encodeToString(value)))
    }

    @Test
    fun excelExportsNumericRoundsAndLiteralNotes() {
        val initial = protocolJson.decodeFromString<Match>(resource("example-match.json"))
        val round = initial.rounds.single()
        val value = initial.copy(rounds = listOf(round.copy(result = round.result.copy(note = "=A1 < & >"))))
        val parts = protocolJson.decodeFromString<Map<String, String>>(resource("xlsx-parts.json"))
        val out = ByteArrayOutputStream()
        value.writeExcel(out, value.progress(), parts)
        val entries = ZipInputStream(out.toByteArray().inputStream()).use { input ->
            buildMap {
                while (true) {
                    val entry = input.nextEntry ?: break
                    put(entry.name, input.readBytes().toString(Charsets.UTF_8))
                }
            }
        }
        val sheet = entries.getValue("xl/worksheets/sheet1.xml")
        assertTrue(entries.containsKey("[Content_Types].xml"))
        assertTrue(sheet.contains("""<c r="A5"><v>1</v></c>"""))
        assertTrue(sheet.contains("""<c r="I5" s="2" t="inlineStr"><is><t xml:space="preserve">=A1 &lt; &amp; &gt;</t>"""))
        assertTrue(sheet.contains("""<autoFilter ref="A4:J5"/>"""))
    }
}
