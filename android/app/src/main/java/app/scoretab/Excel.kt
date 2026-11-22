package app.scoretab

import java.io.OutputStream
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

internal fun Match.writeExcel(output: OutputStream, progress: Progress, parts: Map<String, String>) {
    ZipOutputStream(output).use { zip ->
        for ((name, xml) in parts + ("xl/worksheets/sheet1.xml" to excelSheet(progress))) {
            zip.putNextEntry(ZipEntry(name))
            zip.write(xml.toByteArray(Charsets.UTF_8))
            zip.closeEntry()
        }
    }
}

private fun Match.excelSheet(progress: Progress) = buildString {
    append("""<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetViews><sheetView workbookViewId="0"><pane ySplit="4" topLeftCell="A5" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews><cols><col min="1" max="1" width="13" customWidth="1"/><col min="2" max="2" width="24" customWidth="1"/><col min="3" max="4" width="11" customWidth="1"/><col min="5" max="5" width="24" customWidth="1"/><col min="6" max="8" width="13" customWidth="1"/><col min="9" max="9" width="46" customWidth="1"/><col min="10" max="10" width="20" customWidth="1"/></cols><sheetData>""")
    excelRow(1, listOf(teams[0].name, teams[1].name, progress.score.toString(), map))
    excelRow(2, listOf(if (progress.finished) "Матч завершён" else "Матч продолжается"))
    excelRow(4, listOf("Раунд", "Победитель", "Сторона", "Плент", "Причина", "Счёт",
        "Позиция", "Раунд ОТ", "Комментарий", "Период"), header = true)
    rounds.zip(progress.scores).forEachIndexed { index, (round, score) ->
        val position = position(round.number)
        val fields = roundFields(round, score)
        val local = if (position.period == 0) "" else position.number.toString()
        val period = if (position.period == 0) "Основное время" else "Овертайм ${position.period}"
        excelRow(index + 5, fields.take(7) + listOf(local, fields[7], period), round = true)
    }
    append("""</sheetData><autoFilter ref="A4:J${rounds.size + 4}"/></worksheet>""")
}

private fun StringBuilder.excelRow(number: Int, cells: List<String>, header: Boolean = false, round: Boolean = false) {
    append("""<row r="$number">""")
    cells.forEachIndexed { index, text ->
        val cell = "${'A' + index}$number"
        if (round && (index == 0 || index == 7) && text.isNotEmpty()) {
            append("""<c r="$cell"><v>$text</v></c>""")
        } else {
            val style = when {
                header -> 1
                round && index == 8 -> 2
                else -> 0
            }
            append("""<c r="$cell" s="$style" t="inlineStr"><is><t xml:space="preserve">""")
            val codepoints = text.codePoints().iterator()
            var units = 0
            while (codepoints.hasNext()) {
                val cp = codepoints.nextInt()
                units += Character.charCount(cp)
                if (units > 32767) break
                when (cp) {
                    '&'.code -> append("&amp;")
                    '<'.code -> append("&lt;")
                    '>'.code -> append("&gt;")
                    '\r'.code -> append("&#13;")
                    '\n'.code, '\t'.code, in 0x20..0xd7ff, in 0xe000..0xfffd, in 0x10000..0x10ffff -> appendCodePoint(cp)
                }
            }
            append("</t></is></c>")
        }
    }
    append("</row>")
}
