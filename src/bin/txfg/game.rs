//! The game file (one `.json` per round), its validation and its rating.

use std::{collections::BTreeSet, fmt, fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    catalog::{CATALOG, Entry},
    engine::Engine,
    syntax::{Word, split_words},
};

/// The game file format this version reads (`format` field).
pub const FORMAT: u32 = 1;
/// Highest difficulty.
pub const MAX_DIFFICULTY: u8 = 10;
/// Largest `allowed_pipeline_max_size`.
pub const MAX_PIPELINE_SIZE: usize = 100;
/// Instructions a game may not allow: they read or write files, and a game
/// runs its answer when it is loaded, so a downloaded game could touch the
/// player's files.
pub const FORBIDDEN: &[&str] = &["wtf", "apf", "ppf", "inf", "ppjn"];
/// Instructions that hold another instruction, and the keyword before it.
const NESTING: &[(&str, &str)] = &[
    ("blk", "assoc"),
    ("ifdc", "do"),
    ("iter", "times"),
    ("onln", "apply"),
    ("onck", "apply"),
    ("onlni", "apply"),
];

/// One round: a start text, a goal text, the instructions the player may use
/// and a reference answer.
///
/// ```json
/// {
///   "name": "Textforge Introduction",
///   "description": "The basics: trimming, appending and case.",
///   "difficulty": 3,
///   "start": " banana    ",
///   "goal": "BANANA FRUIT!",
///   "allowed_instructions": ["tbs", "trs", "tls", "ate", "tua"],
///   "allowed_pipeline_max_size": 10,
///   "answer": ["tbs;", "ate \" fruit!\";", "tua;"]
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Game {
    /// File format version; [`FORMAT`] when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<u32>,
    /// Stable identifier, for rankings and game repositories; the name when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Title shown to the player.
    pub name: String,
    /// What the round teaches.
    #[serde(default)]
    pub description: String,
    /// Who wrote the game.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Difficulty chosen by the author, 0 to 10; the computed rating when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<u8>,
    /// The text the round starts with.
    pub start: String,
    /// The text the player must produce.
    pub goal: String,
    /// Mnemonics the player may use.
    pub allowed_instructions: Vec<String>,
    /// How many instructions the player's pipeline may have.
    pub allowed_pipeline_max_size: usize,
    /// A pipeline that turns `start` into `goal`, one instruction per item.
    pub answer: Vec<String>,
}

/// A game file that cannot be read or is not a valid game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameError(pub String);

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GameError {}

/// Why an instruction cannot be used in a round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// No instruction has this name.
    Unknown(String),
    /// The instruction exists, but the round does not allow it.
    NotAllowed(String),
    /// The text holds no instruction.
    Empty,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Unknown(name) => write!(f, "`{name}` is not a TextForge instruction"),
            Refusal::NotAllowed(name) => write!(
                f,
                "`{name}` is a TextForge instruction, but this round does not allow it"
            ),
            Refusal::Empty => f.write_str("there is no instruction here"),
        }
    }
}

/// The catalog entry of `name`.
pub fn lookup(name: &str) -> Option<&'static Entry> {
    CATALOG.iter().find(|entry| entry.name == name)
}

/// What one instruction is made of: its instruction names (the outer one
/// first, then nested ones), its keywords and its arguments.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Shape {
    /// Instruction names, outermost first.
    pub names: Vec<String>,
    /// Number of keywords (`times`, `apply`, ...).
    pub keywords: usize,
    /// Number of arguments.
    pub arguments: usize,
}

/// The [`Shape`] of `instruction`.
pub fn shape(instruction: &str) -> Shape {
    let mut shape = Shape::default();
    let words = split_words(instruction);
    shape_of(&words, &mut shape);
    shape
}

fn shape_of(words: &[Word], shape: &mut Shape) {
    let Some((first, rest)) = words.split_first() else {
        return;
    };
    shape.names.push(first.text.clone());
    let keyword = NESTING
        .iter()
        .find(|(name, _)| *name == first.text && !first.quoted)
        .map(|(_, keyword)| *keyword);
    let inner = keyword.and_then(|keyword| {
        rest.iter()
            .position(|w| !w.quoted && w.text == keyword)
            .map(|at| (at, keyword))
    });
    match inner {
        Some((at, _)) => {
            shape.arguments += at;
            shape.keywords += 1;
            shape_of(&rest[at + 1..], shape);
        }
        None => shape.arguments += rest.len(),
    }
}

/// A validated game with its computed rating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RatedGame {
    /// The game as written.
    pub game: Game,
    /// Difficulty computed from the game, 0 to 10 (see [`rate`]).
    pub rating: u8,
}

impl RatedGame {
    /// The difficulty used to choose games: the author's, or the rating.
    pub fn difficulty(&self) -> u8 {
        self.game.difficulty.unwrap_or(self.rating)
    }
}

impl Game {
    /// Reads a game from JSON.
    pub fn parse(json: &str) -> Result<Game, GameError> {
        serde_json::from_str(json).map_err(|e| GameError(format!("invalid game file: {e}")))
    }

    /// Reads a game from a file.
    pub fn from_file(path: &Path) -> Result<Game, GameError> {
        let json = fs::read_to_string(path)
            .map_err(|e| GameError(format!("cannot read `{}`: {e}", path.display())))?;
        Game::parse(&json).map_err(|e| GameError(format!("`{}`: {e}", path.display())))
    }

    /// The identifier: `id`, or the name.
    pub fn id(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.name)
    }

    /// Whether the round allows `name`.
    pub fn allows(&self, name: &str) -> bool {
        self.allowed_instructions
            .iter()
            .any(|allowed| allowed == name)
    }

    /// Checks that every instruction name in `instruction` (nested ones
    /// included) exists and is allowed.
    pub fn check(&self, instruction: &str) -> Result<(), Refusal> {
        let names = shape(instruction).names;
        if names.is_empty() {
            return Err(Refusal::Empty);
        }
        for name in names {
            if lookup(&name).is_none() {
                return Err(Refusal::Unknown(name));
            }
            if !self.allows(&name) {
                return Err(Refusal::NotAllowed(name));
            }
        }
        Ok(())
    }

    /// Checks the game and rates it. The answer runs on `start` and must
    /// produce `goal`.
    pub fn validate(self, engine: &Engine) -> Result<RatedGame, GameError> {
        let fail = |message: String| Err(GameError(format!("game `{}`: {message}", self.name)));
        if let Some(format) = self.format
            && format != FORMAT
        {
            return fail(format!(
                "unknown format {format}; this txfg reads format {FORMAT}"
            ));
        }
        if self.name.trim().is_empty() {
            return Err(GameError("a game needs a `name`".into()));
        }
        if let Some(difficulty) = self.difficulty
            && difficulty > MAX_DIFFICULTY
        {
            return fail(format!(
                "`difficulty` is {difficulty}; it must be between 0 and {MAX_DIFFICULTY}"
            ));
        }
        if self.start == self.goal {
            return fail("`start` and `goal` are the same text".into());
        }
        if self.allowed_instructions.is_empty() {
            return fail("`allowed_instructions` is empty".into());
        }
        let mut seen = BTreeSet::new();
        for name in &self.allowed_instructions {
            if lookup(name).is_none() {
                return fail(format!(
                    "`{name}` in `allowed_instructions` is not an instruction"
                ));
            }
            if FORBIDDEN.contains(&name.as_str()) {
                return fail(format!(
                    "`{name}` reads or writes files and cannot be allowed in a game"
                ));
            }
            if !seen.insert(name) {
                return fail(format!(
                    "`{name}` is listed twice in `allowed_instructions`"
                ));
            }
        }
        let size = self.allowed_pipeline_max_size;
        if size == 0 || size > MAX_PIPELINE_SIZE {
            return fail(format!(
                "`allowed_pipeline_max_size` is {size}; it must be between 1 and {MAX_PIPELINE_SIZE}"
            ));
        }
        if self.answer.is_empty() {
            return fail("`answer` is empty".into());
        }
        if self.answer.len() > size {
            return fail(format!(
                "the answer has {} instructions, more than `allowed_pipeline_max_size` ({size})",
                self.answer.len()
            ));
        }
        for line in &self.answer {
            if !line.trim_end().ends_with(';') {
                return fail(format!("answer instruction `{line}` must end with `;`"));
            }
            if let Err(refusal) = self.check(line) {
                return fail(format!("answer instruction `{line}`: {refusal}"));
            }
        }
        match engine.run(&self.answer, &self.start) {
            Ok(text) if text == self.goal => {}
            Ok(text) => {
                return fail(format!(
                    "the answer produces {text:?}, not the goal {:?}",
                    self.goal
                ));
            }
            Err(e) => return fail(format!("the answer fails: {}", e.error_code.message())),
        }
        let rating = rate(&self);
        Ok(RatedGame { game: self, rating })
    }
}

/// Rates a game from 0 to 10 from its answer and limits:
///
/// - steps (0-4): instruction names in the answer, nested ones included, up to 8;
/// - arguments (0-2): arguments in the answer, up to 8;
/// - distractors (0-2): allowed instructions the answer does not use, up to 8;
/// - tightness (0-2): how little room `allowed_pipeline_max_size` leaves
///   beyond the answer (none left: 2; as much room as steps, or more: 0).
pub fn rate(game: &Game) -> u8 {
    let mut steps = 0usize;
    let mut arguments = 0usize;
    let mut used = BTreeSet::new();
    for line in &game.answer {
        let shape = shape(line);
        steps += shape.names.len();
        arguments += shape.arguments;
        used.extend(shape.names);
    }
    let capped = |n: usize| n.min(8) as f64 / 8.0;
    let distractors = game
        .allowed_instructions
        .iter()
        .filter(|name| !used.contains(*name))
        .count();
    let slack = game
        .allowed_pipeline_max_size
        .saturating_sub(game.answer.len());
    let tightness = 1.0 - slack.min(steps.max(1)) as f64 / steps.max(1) as f64;
    let score =
        4.0 * capped(steps) + 2.0 * capped(arguments) + 2.0 * capped(distractors) + 2.0 * tightness;
    score.round().clamp(0.0, f64::from(MAX_DIFFICULTY)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTRO: &str = r#"{
        "name": "Textforge Introduction",
        "description": "The basics",
        "difficulty": 3,
        "start": " banana    ",
        "goal": "BANANA FRUIT!",
        "allowed_instructions": ["tbs", "trs", "tls", "ate", "tua"],
        "allowed_pipeline_max_size": 10,
        "answer": ["tbs;", "ate \" fruit!\";", "tua;"]
    }"#;

    fn intro() -> Game {
        Game::parse(INTRO).unwrap()
    }

    fn invalid(change: impl FnOnce(&mut Game)) -> String {
        let mut game = intro();
        change(&mut game);
        game.validate(&Engine::new()).unwrap_err().0
    }

    #[test]
    fn validates_and_rates_the_introduction() {
        let rated = intro().validate(&Engine::new()).unwrap();
        assert_eq!(rated.difficulty(), 3);
        assert_eq!(rated.rating, 2);
        assert_eq!(rated.game.id(), "Textforge Introduction");
    }

    #[test]
    fn rejects_invalid_json_with_its_position() {
        let error = Game::parse(r#"{"name": "x", allowed: 1}"#).unwrap_err().0;
        assert!(error.contains("line 1"), "{error}");
        let error = Game::parse(&INTRO.replace("\"goal\"", "\"gaol\""))
            .unwrap_err()
            .0;
        assert!(error.contains("gaol"), "{error}");
    }

    #[test]
    fn rejects_invalid_games() {
        assert!(
            invalid(|g| {
                g.answer.pop();
            })
            .contains("produces")
        );
        assert!(invalid(|g| g.goal = g.start.clone()).contains("same text"));
        assert!(invalid(|g| g.difficulty = Some(11)).contains("between 0 and 10"));
        assert!(
            invalid(|g| g.allowed_instructions.push("zzz".into())).contains("not an instruction")
        );
        assert!(invalid(|g| g.allowed_instructions.push("tbs".into())).contains("twice"));
        assert!(invalid(|g| g.allowed_instructions.push("wtf".into())).contains("files"));
        assert!(invalid(|g| g.allowed_pipeline_max_size = 2).contains("more than"));
        assert!(invalid(|g| g.allowed_pipeline_max_size = 0).contains("between 1"));
        assert!(invalid(|g| g.answer[0] = "tbs".into()).contains("end with"));
        assert!(invalid(|g| g.answer[0] = "rev;".into()).contains("does not allow"));
        assert!(invalid(|g| g.answer.clear()).contains("empty"));
        assert!(invalid(|g| g.format = Some(2)).contains("format"));
    }

    #[test]
    fn checks_nested_instructions() {
        let mut game = intro();
        game.allowed_instructions = vec!["iter".into(), "onln".into(), "tua".into()];
        assert_eq!(game.check("iter 2 times onln 1 apply tua;"), Ok(()));
        assert_eq!(
            game.check("iter 2 times rev;"),
            Err(Refusal::NotAllowed("rev".into()))
        );
        assert_eq!(
            game.check("iter 2 times zz;"),
            Err(Refusal::Unknown("zz".into()))
        );
        assert_eq!(
            game.check("ate \"rev\";"),
            Err(Refusal::NotAllowed("ate".into()))
        );
        assert_eq!(game.check(";"), Err(Refusal::Empty));
    }

    #[test]
    fn shapes_count_names_keywords_and_arguments() {
        let s = shape("iter 3 times onln [[__COUNTER__+1]] apply tucs 0;");
        assert_eq!(s.names, ["iter", "onln", "tucs"]);
        assert_eq!((s.keywords, s.arguments), (2, 3));
        let s = shape("ate \"times\";");
        assert_eq!((s.names.len(), s.arguments), (1, 1));
    }

    #[test]
    fn harder_games_rate_higher() {
        let easy = intro();
        let mut hard = intro();
        hard.allowed_pipeline_max_size = 3;
        hard.allowed_instructions
            .extend(["rev", "tla", "atb", "dlf", "dll"].map(String::from));
        assert!(rate(&hard) > rate(&easy));
        assert!(rate(&hard) <= MAX_DIFFICULTY);
    }
}
