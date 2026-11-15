//! Round and match scores, kept in a serializable form for future rankings.

use serde::Serialize;

/// Best score of a round.
pub const MAX_ROUND_SCORE: u32 = 100;
/// Points for using as few instructions as the reference answer.
const EFFICIENCY_POINTS: f64 = 70.0;
/// Points for finishing within the par time.
const SPEED_POINTS: f64 = 30.0;

/// Seconds a round may take without losing speed points: 20 s plus 15 s per
/// instruction of the reference answer.
pub fn par_seconds(answer_len: usize) -> f64 {
    20.0 + 15.0 * answer_len as f64
}

/// Score of a round, 0 to 100.
///
/// A round that was not solved, or whose answer was shown, scores 0.
/// Otherwise: 70 points times `answer_len / used` (at most 1), plus 30 points
/// times `par / seconds` (at most 1, see [`par_seconds`]).
pub fn round_score(
    answer_len: usize,
    used: usize,
    seconds: f64,
    solved: bool,
    revealed: bool,
) -> u32 {
    if !solved || revealed || used == 0 {
        return 0;
    }
    let efficiency = (answer_len as f64 / used as f64).min(1.0);
    let par = par_seconds(answer_len);
    let speed = if seconds <= par { 1.0 } else { par / seconds };
    let score = (EFFICIENCY_POINTS * efficiency + SPEED_POINTS * speed).round();
    (score as u32).min(MAX_ROUND_SCORE)
}

/// How a round ended.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RoundResult {
    /// The game identifier (`id`, or the name).
    pub game: String,
    /// The game name.
    pub name: String,
    /// The difficulty the game was chosen with.
    pub difficulty: u8,
    /// Whether the player produced the goal.
    pub solved: bool,
    /// Whether the player asked for the answer.
    pub revealed: bool,
    /// Instructions in the player's final pipeline.
    pub instructions: usize,
    /// Seconds from the start of the round to its end.
    pub seconds: f64,
    /// Score, 0 to 100.
    pub score: u32,
    /// The player's final pipeline.
    pub pipeline: Vec<String>,
}

/// A whole match: every round played and the total.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MatchResult {
    /// Version of this result format.
    pub format: u32,
    /// The TextForge version that ran the match.
    pub textforge_version: &'static str,
    /// Rounds in the order they were played.
    pub rounds: Vec<RoundResult>,
    /// Sum of the round scores.
    pub total: u32,
    /// Best possible total for the rounds of the match.
    pub max_total: u32,
}

impl MatchResult {
    /// A result for `rounds` out of `planned` rounds.
    pub fn new(rounds: Vec<RoundResult>, planned: usize) -> Self {
        let total = rounds.iter().map(|r| r.score).sum();
        MatchResult {
            format: 1,
            textforge_version: env!("CARGO_PKG_VERSION"),
            rounds,
            total,
            max_total: MAX_ROUND_SCORE * planned as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_rounds() {
        assert_eq!(round_score(3, 3, 10.0, true, false), 100);
        assert_eq!(round_score(3, 2, 10.0, true, false), 100);
        assert_eq!(round_score(3, 6, 10.0, true, false), 65);
        assert_eq!(round_score(3, 3, 130.0, true, false), 85);
        assert_eq!(round_score(3, 3, 10.0, false, false), 0);
        assert_eq!(round_score(3, 3, 10.0, true, true), 0);
    }

    #[test]
    fn sums_a_match() {
        let round = RoundResult {
            game: "g".into(),
            name: "G".into(),
            difficulty: 1,
            solved: true,
            revealed: false,
            instructions: 1,
            seconds: 1.0,
            score: 40,
            pipeline: vec!["tua;".into()],
        };
        let result = MatchResult::new(vec![round.clone(), round], 3);
        assert_eq!((result.total, result.max_total), (80, 300));
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"total\":80"), "{json}");
    }
}
