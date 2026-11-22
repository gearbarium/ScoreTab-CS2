use serde::{Deserialize, Serialize};

pub type RuleResult<T> = Result<T, &'static str>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    T,
    CT,
}

impl Side {
    pub fn opposite(self) -> Self {
        match self {
            Self::T => Self::CT,
            Self::CT => Self::T,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reason {
    Elimination,
    Explosion,
    Defuse,
    Timeout,
    Technical,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Team {
    pub id: u32,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rules {
    pub regulation: u32,
    pub switch_after: u32,
    pub win_target: u32,
    pub overtime_half: Option<u32>,
    pub overtime_start: Option<Side>,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            regulation: 24,
            switch_after: 12,
            win_target: 13,
            overtime_half: Some(3),
            overtime_start: None,
        }
    }
}

impl Rules {
    pub fn validate(&self) -> RuleResult<()> {
        if !(2..=200).contains(&self.regulation)
            || !(1..self.regulation).contains(&self.switch_after)
            || !(self.regulation / 2 + 1..=self.regulation).contains(&self.win_target)
            || self
                .overtime_half
                .is_some_and(|half| !(1..=50).contains(&half))
        {
            return Err("Проверьте лимит раундов, границу половин и порог победы.");
        }
        Ok(())
    }

    pub fn position(&self, number: u32, starting_side: Side) -> Position {
        if number <= self.regulation {
            return Position {
                period: 0,
                number,
                first_side: if number > self.switch_after {
                    starting_side.opposite()
                } else {
                    starting_side
                },
            };
        }
        let half = self.overtime_half.unwrap_or(1);
        let offset = number - self.regulation - 1;
        let number = offset % (half * 2) + 1;
        let start = self
            .overtime_start
            .unwrap_or_else(|| starting_side.opposite());
        Position {
            period: offset / (half * 2) + 1,
            number,
            first_side: if number > half {
                start.opposite()
            } else {
                start
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub period: u32,
    pub number: u32,
    pub first_side: Side,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub winner: Option<u32>,
    pub reason: Option<Reason>,
    pub planted: bool,
    pub site: Option<String>,
    pub note: String,
}

impl Draft {
    pub fn complete(mut self, side: Side) -> RuleResult<Self> {
        match (side, self.reason.ok_or("Выберите причину победы.")?) {
            (Side::T, Reason::Explosion) | (Side::CT, Reason::Defuse) => self.planted = true,
            (Side::T, Reason::Defuse | Reason::Timeout) | (Side::CT, Reason::Explosion) => {
                return Err("Причина несовместима со стороной победителя.")
            }
            (Side::CT, Reason::Elimination | Reason::Timeout) if self.planted => {
                return Err("Снимите отметку плента или выберите дефуз.")
            }
            (_, Reason::Technical) if self.note.trim().is_empty() => {
                return Err("Укажите основание технического решения.")
            }
            _ => (),
        }
        if self
            .site
            .as_deref()
            .is_some_and(|site| !self.planted || !matches!(site, "A" | "B"))
        {
            return Err("Место установки допустимо только при пленте. Выберите A или B.");
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Round {
    pub number: u32,
    pub saved_at: i64,
    pub result: Draft,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    pub before: Round,
    pub after: Option<Round>,
    pub saved_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Match {
    pub version: u32,
    pub teams: [Team; 2],
    pub map: String,
    pub starting_side: Side,
    pub rules: Rules,
    pub created_at: i64,
    pub rounds: Vec<Round>,
    pub revisions: Vec<Revision>,
    pub draft: Draft,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub scores: Vec<[u32; 2]>,
    pub finished: bool,
}

impl Progress {
    pub fn score(&self) -> [u32; 2] {
        self.scores.last().copied().unwrap_or_default()
    }
}

impl Match {
    pub fn next(&self) -> u32 {
        self.rounds.len() as u32 + 1
    }

    pub fn position(&self, number: u32) -> Position {
        self.rules.position(number, self.starting_side)
    }

    pub fn team_index(&self, id: u32) -> RuleResult<usize> {
        self.teams
            .iter()
            .position(|team| team.id == id)
            .ok_or("Неизвестная команда.")
    }

    pub fn normalize(&self, number: u32, draft: Draft) -> RuleResult<Draft> {
        let winner = self.team_index(draft.winner.ok_or("Выберите победителя.")?)?;
        let first_side = self.position(number).first_side;
        draft.complete(if winner == 0 {
            first_side
        } else {
            first_side.opposite()
        })
    }

    pub fn progress(&self) -> RuleResult<Progress> {
        self.rules.validate()?;
        if self.version != 1
            || self.teams[0].id == self.teams[1].id
            || self.teams.iter().any(|team| team.name.trim().is_empty())
        {
            return Err("Некорректная версия протокола или команды.");
        }
        if self
            .draft
            .winner
            .is_some_and(|id| self.team_index(id).is_err())
            || self
                .draft
                .site
                .as_deref()
                .is_some_and(|site| !self.draft.planted || !matches!(site, "A" | "B"))
            || (self.draft.winner.is_none() && self.draft.reason.is_some())
        {
            return Err("Некорректный черновик раунда.");
        }
        for revision in &self.revisions {
            let before = &revision.before;
            if before.number == 0
                || self.normalize(before.number, before.result.clone())? != before.result
                || revision.after.as_ref().is_some_and(|after| {
                    after.number != before.number
                        || self
                            .normalize(after.number, after.result.clone())
                            .ok()
                            .as_ref()
                            != Some(&after.result)
                })
            {
                return Err("Некорректная запись в истории правок.");
            }
        }
        let mut progress = Progress::default();
        let mut period_score = [0, 0];
        for (index, round) in self.rounds.iter().enumerate() {
            if progress.finished {
                return Err("После пересчёта матч заканчивается раньше. Следующие записи сохранены. Отмените исправление или сначала аннулируйте последние раунды.");
            }
            if round.number as usize != index + 1
                || self.normalize(round.number, round.result.clone())? != round.result
            {
                return Err("Некорректная запись раунда.");
            }
            let winner = self.team_index(round.result.winner.ok_or("Нет победителя.")?)?;
            let mut score = progress.score();
            score[winner] += 1;
            progress.scores.push(score);
            let position = self.position(round.number);
            if position.period == 0 {
                progress.finished = score[winner] >= self.rules.win_target
                    || (round.number == self.rules.regulation
                        && (score[0] != score[1] || self.rules.overtime_half.is_none()));
            } else {
                let half = self.rules.overtime_half.ok_or("Овертайм отключён.")?;
                if position.number == 1 {
                    period_score = [0, 0];
                }
                period_score[winner] += 1;
                progress.finished = period_score[winner] > half;
            }
        }
        Ok(progress)
    }

    pub fn commit(&mut self, expected: u32, time: i64) -> RuleResult<()> {
        if self.next() != expected || self.progress()?.finished {
            return Err("Раунд уже закрыт.");
        }
        let result = self.normalize(expected, self.draft.clone())?;
        self.rounds.push(Round {
            number: expected,
            saved_at: time,
            result,
        });
        self.draft = Draft::default();
        Ok(())
    }

    pub fn edit(&mut self, number: u32, draft: Draft, time: i64) -> RuleResult<()> {
        let result = self.normalize(number, draft)?;
        let index = number.checked_sub(1).ok_or("Раунд не найден.")? as usize;
        let before = self.rounds.get(index).cloned().ok_or("Раунд не найден.")?;
        let after = Round {
            result,
            ..before.clone()
        };
        let mut candidate = self.clone();
        candidate.rounds[index] = after.clone();
        candidate.progress()?;
        candidate.revisions.push(Revision {
            before,
            after: Some(after),
            saved_at: time,
        });
        *self = candidate;
        Ok(())
    }

    pub fn void_last(&mut self, time: i64) -> RuleResult<()> {
        let before = self.rounds.pop().ok_or("Нет завершённых раундов.")?;
        self.revisions.push(Revision {
            before,
            after: None,
            saved_at: time,
        });
        self.draft = Draft::default();
        Ok(())
    }

    pub fn undo_edit(&mut self, time: i64) -> RuleResult<()> {
        let revision = self.revisions.last().ok_or("Нет исправлений.")?.clone();
        let after = revision
            .after
            .ok_or("Последнее действие — аннулирование.")?;
        if self.rounds.get((after.number - 1) as usize) != Some(&after) {
            return Err("Раунд изменился после исправления.");
        }
        self.edit(after.number, revision.before.result, time)
    }
}
