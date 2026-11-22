package app.scoretab

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.printToString
import org.junit.Rule
import org.junit.Test

class MatchFlowTest {
    @get:Rule
    val ui = createAndroidComposeRule<MainActivity>()

    @Test
    fun createRecordEditAndRestore() {
        val first = "Кривичи"
        val second = "Матрица"

        waitFor("Новый матч")
        click("Новый матч")
        ui.onNodeWithText("Команда 1").performScrollTo().performClick().performTextInput(first)
        ui.onNodeWithText("Команда 2").performScrollTo().performClick().performTextInput(second)
        click("Начать матч")
        waitFor("Раунд 1")
        click("Победа T · $first")
        click("Взрыв")
        click("Следующий раунд")
        waitFor("Раунд 2")
        click("Протокол матча")
        click("Редактировать")
        click("Победа CT · $second")
        click("Дефуз")
        click("Сохранить", scroll = false)
        waitFor("Раунд 1 · 0:1")
        ui.onNodeWithText("$second (CT) · Дефуз · Плент: Да").performScrollTo().assertIsDisplayed()
        ui.activityRule.scenario.recreate()
        waitFor("Раунд 1 · 0:1")
        click("К текущему раунду")
        waitFor("Раунд 2")
    }

    private fun waitFor(label: String) {
        ui.waitUntil(10_000) { runCatching { ui.onNodeWithText(label).fetchSemanticsNode() }.isSuccess }
    }

    private fun click(label: String, scroll: Boolean = true) {
        val node = ui.onNodeWithText(label)
        if (scroll) node.performScrollTo()
        try {
            ui.waitUntil(10_000) { runCatching { node.assertIsEnabled() }.isSuccess }
        } catch (error: AssertionError) {
            throw AssertionError("Button $label did not become enabled.\n${ui.onRoot().printToString()}", error)
        }
        node.performClick()
    }
}
