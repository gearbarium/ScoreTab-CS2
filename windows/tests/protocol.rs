use scoretab_cs2::{domain::*, storage::Store};
use serde::Deserialize;

fn game(rules: Rules) -> Match {
    Match {
        version: 1,
        teams: [
            Team {
                id: 41,
                name: "Кривичи".into(),
            },
            Team {
                id: 72,
                name: "Матрица".into(),
            },
        ],
        map: "Mirage".into(),
        starting_side: Side::T,
        rules,
        created_at: 1,
        rounds: vec![],
        revisions: vec![],
        draft: Draft::default(),
    }
}

fn winner(id: u32) -> Draft {
    Draft {
        winner: Some(id),
        reason: Some(Reason::Elimination),
        ..Default::default()
    }
}

#[derive(Deserialize)]
struct Case {
    id: String,
    #[serde(default)]
    rules: Rules,
    winners: Vec<usize>,
    score: [u32; 2],
    finished: bool,
    period: u32,
    local: u32,
    side: Side,
    status_position: u32,
}

#[test]
fn shared_scoring_cases() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../../shared-spec/test-cases.json")).unwrap();
    for case in cases {
        let mut value = game(case.rules);
        for index in case.winners {
            value.draft = winner(value.teams[index - 1].id);
            value.commit(value.next(), 1).unwrap();
        }
        let progress = value.progress().unwrap();
        let position = value.position(case.status_position);
        assert_eq!(progress.score(), case.score, "{}", case.id);
        assert_eq!(progress.finished, case.finished, "{}", case.id);
        assert_eq!(
            (position.period, position.number, position.first_side),
            (case.period, case.local, case.side),
            "{}",
            case.id
        );
    }
}

#[derive(Deserialize)]
struct Event {
    side: Side,
    reason: Reason,
    planted: bool,
    normalized: Option<bool>,
}

#[test]
fn shared_event_cases() {
    let cases: Vec<Event> =
        serde_json::from_str(include_str!("../../shared-spec/round-events.json")).unwrap();
    for case in cases {
        let draft = Draft {
            reason: Some(case.reason),
            planted: case.planted,
            ..Default::default()
        };
        assert_eq!(
            draft.complete(case.side).ok().map(|draft| draft.planted),
            case.normalized
        );
    }
}

#[test]
fn edits_and_replay_preserve_history() {
    let mut value = game(Rules::default());
    value.draft = winner(41);
    value.commit(1, 1).unwrap();
    assert!(value.commit(1, 1).is_err());
    value.draft = winner(72);
    value.commit(2, 2).unwrap();
    value.edit(1, winner(72), 3).unwrap();
    assert_eq!(value.progress().unwrap().scores, [[0, 1], [0, 2]]);
    value.undo_edit(4).unwrap();
    assert_eq!(value.progress().unwrap().score(), [1, 1]);
    value.void_last(5).unwrap();
    assert_eq!(value.next(), 2);
    assert_eq!(value.revisions.len(), 3);
    assert!(value.revisions.last().unwrap().after.is_none());
}

#[test]
fn conflicting_edit_leaves_records_intact() {
    let mut value = game(Rules::default());
    for index in 0..25 {
        value.draft = winner(if index % 2 == 0 { 41 } else { 72 });
        value.commit(value.next(), 1).unwrap();
    }
    let previous = value.clone();
    assert!(value.edit(2, winner(41), 2).is_err());
    assert_eq!(value, previous);
}

#[test]
fn cross_platform_json_preserves_draft() {
    let value: Match =
        serde_json::from_str(include_str!("../../shared-spec/example-match.json")).unwrap();
    assert_eq!(value.progress().unwrap().score(), [1, 0]);
    assert_eq!(value.draft.winner, Some(72));
    assert_eq!(value.draft.site.as_deref(), Some("B"));
    let exported = serde_json::to_string(&value).unwrap();
    assert_eq!(serde_json::from_str::<Match>(&exported).unwrap(), value);
}

#[test]
fn excel_exports_numeric_rounds_and_keeps_formula_like_notes_as_text() {
    use std::io::{Cursor, Read};

    let mut value: Match =
        serde_json::from_str(include_str!("../../shared-spec/example-match.json")).unwrap();
    value.rounds[0].result.note = "=A1 < & >".into();
    let mut out = Cursor::new(Vec::new());
    scoretab_cs2::xlsx::export(&mut out, &value, &value.progress().unwrap()).unwrap();
    let mut book = zip::ZipArchive::new(out).unwrap();
    assert!(book.by_name("[Content_Types].xml").is_ok());
    let mut sheet = String::new();
    book.by_name("xl/worksheets/sheet1.xml")
        .unwrap()
        .read_to_string(&mut sheet)
        .unwrap();
    assert!(sheet.contains(r#"<c r="A5"><v>1</v></c>"#));
    assert!(sheet.contains(
        r#"<c r="I5" s="2" t="inlineStr"><is><t xml:space="preserve">=A1 &lt; &amp; &gt;</t>"#
    ));
    assert!(sheet.contains(r#"<autoFilter ref="A4:J5"/>"#));
}

#[test]
fn sqlite_persists_draft_and_rejects_stale_write() {
    let path = std::env::temp_dir().join(format!("scoretab-{}.sqlite3", std::process::id()));
    let initial = game(Rules::default());
    let (id, draft) = {
        let store = Store::open(&path).unwrap();
        assert_eq!(store.export_folder().unwrap(), None);
        store.set_export_folder("C:/Матчи/Экспорт").unwrap();
        let id = store.insert(&initial).unwrap();
        let mut draft = initial.clone();
        draft.draft = winner(41);
        store.update(id, &initial, &draft).unwrap();
        (id, draft)
    };
    {
        let store = Store::open(&path).unwrap();
        assert_eq!(
            store.export_folder().unwrap().as_deref(),
            Some("C:/Матчи/Экспорт")
        );
        assert_eq!(store.list().unwrap().matches, [(id, draft.clone())]);
        let mut confirmed = draft.clone();
        confirmed.commit(1, 123).unwrap();
        store.update(id, &draft, &confirmed).unwrap();
        assert!(store.update(id, &draft, &confirmed).is_err());
        assert_eq!(store.list().unwrap().matches, [(id, confirmed)]);
    }
    std::fs::remove_file(path).unwrap();
}
