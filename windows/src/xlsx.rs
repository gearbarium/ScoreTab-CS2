use crate::domain::{Match, Progress, Reason};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    io::{Seek, Write},
};
use zip::{write::SimpleFileOptions, ZipWriter};

pub fn export(
    output: impl Write + Seek,
    game: &Match,
    progress: &Progress,
) -> zip::result::ZipResult<()> {
    let mut zip = ZipWriter::new(output);
    let parts: BTreeMap<&str, String> =
        serde_json::from_str(include_str!("../../shared-spec/xlsx-parts.json"))
            .expect("embedded workbook parts");
    let options = SimpleFileOptions::default();
    for (name, xml) in parts {
        zip.start_file(name, options)?;
        zip.write_all(xml.as_bytes())?;
    }
    zip.start_file("xl/worksheets/sheet1.xml", options)?;
    zip.write_all(sheet(game, progress).as_bytes())?;
    zip.finish()?;
    Ok(())
}

fn sheet(game: &Match, progress: &Progress) -> String {
    let mut xml = String::from(
        r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetViews><sheetView workbookViewId="0"><pane ySplit="4" topLeftCell="A5" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews><cols><col min="1" max="1" width="13" customWidth="1"/><col min="2" max="2" width="24" customWidth="1"/><col min="3" max="4" width="11" customWidth="1"/><col min="5" max="5" width="24" customWidth="1"/><col min="6" max="8" width="13" customWidth="1"/><col min="9" max="9" width="46" customWidth="1"/><col min="10" max="10" width="20" customWidth="1"/></cols><sheetData>"#,
    );
    let [a, b] = progress.score();
    row(
        &mut xml,
        1,
        &[
            &game.teams[0].name,
            &game.teams[1].name,
            &format!("{a}:{b}"),
            &game.map,
        ],
        0,
        false,
    );
    row(
        &mut xml,
        2,
        &[if progress.finished {
            "Матч завершён"
        } else {
            "Матч продолжается"
        }],
        0,
        false,
    );
    row(
        &mut xml,
        4,
        &[
            "Раунд",
            "Победитель",
            "Сторона",
            "Плент",
            "Причина",
            "Счёт",
            "Позиция",
            "Раунд ОТ",
            "Комментарий",
            "Период",
        ],
        1,
        false,
    );
    for (index, (round, score)) in game.rounds.iter().zip(&progress.scores).enumerate() {
        let result = &round.result;
        let winner = game
            .team_index(result.winner.expect("validated winner"))
            .expect("validated team");
        let position = game.position(round.number);
        let side = if winner == 0 {
            position.first_side
        } else {
            position.first_side.opposite()
        };
        let label = match result.reason.expect("validated reason") {
            Reason::Elimination => "Уничтожение",
            Reason::Explosion => "Взрыв",
            Reason::Defuse => "Дефуз",
            Reason::Timeout => "Время",
            Reason::Technical => "Техническое решение",
        };
        let period = if position.period == 0 {
            "Основное время".to_owned()
        } else {
            format!("Овертайм {}", position.period)
        };
        let local = if position.period == 0 {
            String::new()
        } else {
            position.number.to_string()
        };
        row(
            &mut xml,
            index + 5,
            &[
                &round.number.to_string(),
                &game.teams[winner].name,
                &format!("{side:?}"),
                if result.planted { "Да" } else { "Нет" },
                label,
                &format!("{}:{}", score[0], score[1]),
                result.site.as_deref().unwrap_or(""),
                &local,
                &result.note,
                &period,
            ],
            0,
            true,
        );
    }
    write!(
        xml,
        "</sheetData><autoFilter ref=\"A4:J{}\"/></worksheet>",
        game.rounds.len() + 4
    )
    .unwrap();
    xml
}

fn row(xml: &mut String, number: usize, cells: &[&str], style: u8, round: bool) {
    write!(xml, "<row r=\"{number}\">").unwrap();
    for (index, text) in cells.iter().enumerate() {
        let column = char::from(b'A' + index as u8);
        let style = if round && index == 8 { 2 } else { style };
        if round && matches!(index, 0 | 7) && !text.is_empty() {
            write!(xml, "<c r=\"{column}{number}\"><v>{text}</v></c>").unwrap();
            continue;
        }
        write!(xml, "<c r=\"{column}{number}\" s=\"{style}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">").unwrap();
        let mut units = 0;
        for c in text.chars() {
            units += c.len_utf16();
            if units > 32767 {
                break;
            }
            match c {
                '&' => xml.push_str("&amp;"),
                '<' => xml.push_str("&lt;"),
                '>' => xml.push_str("&gt;"),
                '\r' => xml.push_str("&#13;"),
                '\n'
                | '\t'
                | '\u{20}'..='\u{d7ff}'
                | '\u{e000}'..='\u{fffd}'
                | '\u{10000}'..='\u{10ffff}' => xml.push(c),
                _ => (),
            }
        }
        xml.push_str("</t></is></c>");
    }
    xml.push_str("</row>");
}
