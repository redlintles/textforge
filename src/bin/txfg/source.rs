//! Where games come from, and how the rounds of a match are chosen.
//!
//! A [`GameSource`] yields validated games. This version has built-in games,
//! game files and directories of game files; a remote game repository would be
//! one more implementation.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    engine::Engine,
    game::{Game, GameError, MAX_DIFFICULTY, RatedGame},
};

/// Rounds of a match when the player does not choose.
pub const DEFAULT_ROUNDS: usize = 3;

/// A collection of games.
pub trait GameSource {
    /// A short description for messages, e.g. "the built-in games".
    fn describe(&self) -> String;
    /// Loads and validates every game.
    fn load(&self, engine: &Engine) -> Result<Vec<RatedGame>, GameError>;
}

/// The games shipped with `txfg` (`src/bin/txfg/games`).
pub struct Builtin;

const BUILTIN: &[(&str, &str)] = &[
    (
        "01-first-steps.json",
        include_str!("games/01-first-steps.json"),
    ),
    (
        "02-textforge-introduction.json",
        include_str!("games/02-textforge-introduction.json"),
    ),
    (
        "03-trim-the-edges.json",
        include_str!("games/03-trim-the-edges.json"),
    ),
    (
        "04-snake-case.json",
        include_str!("games/04-snake-case.json"),
    ),
    (
        "05-repeat-yourself.json",
        include_str!("games/05-repeat-yourself.json"),
    ),
    ("06-mirror.json", include_str!("games/06-mirror.json")),
    (
        "07-unique-lines.json",
        include_str!("games/07-unique-lines.json"),
    ),
    (
        "08-digits-only.json",
        include_str!("games/08-digits-only.json"),
    ),
    (
        "09-every-line.json",
        include_str!("games/09-every-line.json"),
    ),
    (
        "10-key-values.json",
        include_str!("games/10-key-values.json"),
    ),
];

impl GameSource for Builtin {
    fn describe(&self) -> String {
        "the built-in games".into()
    }

    fn load(&self, engine: &Engine) -> Result<Vec<RatedGame>, GameError> {
        BUILTIN
            .iter()
            .map(|(file, json)| {
                Game::parse(json)
                    .and_then(|game| game.validate(engine))
                    .map_err(|e| GameError(format!("built-in `{file}`: {e}")))
            })
            .collect()
    }
}

/// Game files, in the given order.
pub struct Files(pub Vec<PathBuf>);

impl GameSource for Files {
    fn describe(&self) -> String {
        "the given files".into()
    }

    fn load(&self, engine: &Engine) -> Result<Vec<RatedGame>, GameError> {
        self.0.iter().map(|path| load_file(path, engine)).collect()
    }
}

/// Every `*.json` file of a directory, by file name.
pub struct Directory(pub PathBuf);

impl GameSource for Directory {
    fn describe(&self) -> String {
        format!("the games in `{}`", self.0.display())
    }

    fn load(&self, engine: &Engine) -> Result<Vec<RatedGame>, GameError> {
        let entries = fs::read_dir(&self.0)
            .map_err(|e| GameError(format!("cannot read `{}`: {e}", self.0.display())))?;
        let mut paths = Vec::new();
        for entry in entries {
            let path = entry
                .map_err(|e| GameError(format!("cannot read `{}`: {e}", self.0.display())))?
                .path();
            if path.extension().is_some_and(|e| e == "json") && path.is_file() {
                paths.push(path);
            }
        }
        paths.sort();
        Files(paths).load(engine)
    }
}

fn load_file(path: &Path, engine: &Engine) -> Result<RatedGame, GameError> {
    Game::from_file(path)?
        .validate(engine)
        .map_err(|e| GameError(format!("`{}`: {e}", path.display())))
}

/// Chooses the rounds of a match from `pool`.
///
/// With a `difficulty`, the hardest games up to that difficulty are chosen;
/// otherwise the easiest. Rounds are played from the easiest to the hardest.
/// A pool with fewer games than `rounds` gives a shorter match.
pub fn select(
    mut pool: Vec<RatedGame>,
    difficulty: Option<u8>,
    rounds: usize,
) -> Result<Vec<RatedGame>, GameError> {
    pool.sort_by(|a, b| {
        a.difficulty()
            .cmp(&b.difficulty())
            .then_with(|| a.game.name.cmp(&b.game.name))
    });
    let mut chosen = match difficulty {
        Some(max) => {
            let easiest = pool
                .first()
                .map(|g| g.difficulty())
                .unwrap_or(MAX_DIFFICULTY);
            pool.retain(|g| g.difficulty() <= max);
            if pool.is_empty() {
                return Err(GameError(format!(
                    "no game has a difficulty of {max} or less; the easiest one is {easiest}"
                )));
            }
            let skip = pool.len().saturating_sub(rounds);
            pool.split_off(skip)
        }
        None => {
            pool.truncate(rounds);
            pool
        }
    };
    if chosen.is_empty() {
        return Err(GameError("there are no games to play".into()));
    }
    chosen.shrink_to_fit();
    Ok(chosen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_game_is_valid() {
        let games = Builtin.load(&Engine::new()).unwrap();
        assert_eq!(games.len(), BUILTIN.len());
        let mut ids: Vec<_> = games.iter().map(|g| g.game.id().to_owned()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), games.len(), "two built-in games share an id");
        for game in &games {
            assert!(game.difficulty() <= MAX_DIFFICULTY);
            assert!(game.game.id.is_some(), "{} has no id", game.game.name);
        }
    }

    #[test]
    fn selects_the_hardest_games_up_to_a_difficulty() {
        let pool = Builtin.load(&Engine::new()).unwrap();
        let chosen = select(pool.clone(), Some(5), 3).unwrap();
        assert_eq!(chosen.len(), 3);
        assert!(chosen.iter().all(|g| g.difficulty() <= 5));
        let best_left = pool
            .iter()
            .filter(|g| g.difficulty() <= 5 && !chosen.contains(g))
            .map(|g| g.difficulty())
            .max()
            .unwrap_or(0);
        assert!(chosen.iter().all(|g| g.difficulty() >= best_left));
        assert!(
            chosen
                .windows(2)
                .all(|w| w[0].difficulty() <= w[1].difficulty())
        );

        let one = select(pool.clone(), Some(10), 1).unwrap();
        let hardest = pool.iter().map(|g| g.difficulty()).max().unwrap();
        assert_eq!(one[0].difficulty(), hardest);
    }

    #[test]
    fn selects_the_easiest_games_by_default() {
        let pool = Builtin.load(&Engine::new()).unwrap();
        let easiest = pool.iter().map(|g| g.difficulty()).min().unwrap();
        let chosen = select(pool, None, DEFAULT_ROUNDS).unwrap();
        assert_eq!(chosen.len(), DEFAULT_ROUNDS);
        assert_eq!(chosen[0].difficulty(), easiest);
    }

    #[test]
    fn reports_an_empty_selection() {
        let pool = Builtin.load(&Engine::new()).unwrap();
        let mut hard = pool.clone();
        hard.retain(|g| g.difficulty() >= 2);
        let error = select(hard, Some(0), 3).unwrap_err().0;
        assert!(error.contains("easiest"), "{error}");
        assert!(select(Vec::new(), None, 3).is_err());
    }

    #[test]
    fn loads_a_directory() {
        let dir = std::env::temp_dir().join(format!("txfg-dir-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.json"), BUILTIN[0].1).unwrap();
        fs::write(dir.join("notes.txt"), "not a game").unwrap();
        let games = Directory(dir.clone()).load(&Engine::new()).unwrap();
        assert_eq!(games.len(), 1);
        fs::write(dir.join("b.json"), "{").unwrap();
        let error = Directory(dir.clone()).load(&Engine::new()).unwrap_err().0;
        assert!(error.contains("b.json"), "{error}");
        fs::remove_dir_all(dir).unwrap();
    }
}
