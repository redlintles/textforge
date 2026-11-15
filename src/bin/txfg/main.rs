//! `txfg`, the TextForge game (`game` feature): rounds that teach how to write
//! pipelines.
//!
//! Each round is a game file (JSON) with a start text, a goal text, the
//! instructions the player may use, how many of them, and a reference answer.
//! The player types instructions at a prompt until the text matches the goal;
//! commands start with `!` (`!help`). Each round scores 0 to 100 from the
//! number of instructions and the time taken, and a match adds them up.
//!
//! See `docs/txfg.md` for the game file format and the scoring rules.

mod catalog;
mod engine;
mod game;
mod round;
mod score;
mod source;
mod syntax;

use std::{
    fs,
    io::{self, IsTerminal, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::{Arg, ArgAction, Command, value_parser};
use colored::Colorize;

use crate::{
    engine::Engine,
    game::{GameError, RatedGame},
    round::{SystemClock, play_round},
    score::{MAX_ROUND_SCORE, MatchResult},
    source::{Builtin, DEFAULT_ROUNDS, Directory, Files, GameSource, select},
};

fn cli() -> Command {
    Command::new("txfg")
        .version(env!("CARGO_PKG_VERSION"))
        .about("The TextForge game: turn the start text into the goal text with a pipeline")
        .arg(
            Arg::new("games")
                .value_name("GAME.json")
                .num_args(0..)
                .value_parser(value_parser!(PathBuf))
                .help("Play these game files, one round each, in this order"),
        )
        .arg(
            Arg::new("difficulty")
                .short('d')
                .long("difficulty")
                .value_name("0-10")
                .value_parser(value_parser!(u8).range(0..=10))
                .conflicts_with("games")
                .help("Play the hardest games with this difficulty or less"),
        )
        .arg(
            Arg::new("rounds")
                .short('r')
                .long("rounds")
                .value_name("N")
                .value_parser(value_parser!(u16).range(1..=100))
                .conflicts_with("games")
                .help("Rounds in the match (default 3)"),
        )
        .arg(
            Arg::new("pool")
                .long("pool")
                .value_name("DIR")
                .value_parser(value_parser!(PathBuf))
                .conflicts_with("games")
                .help("Choose the rounds from the game files in DIR instead of the built-in games"),
        )
        .arg(
            Arg::new("list")
                .long("list")
                .action(ArgAction::SetTrue)
                .help("List the games with their difficulty and rating, then exit"),
        )
        .arg(
            Arg::new("check")
                .long("check")
                .action(ArgAction::SetTrue)
                .conflicts_with("list")
                .help("Validate the games and print their rating without playing"),
        )
        .arg(
            Arg::new("save_result")
                .long("save-result")
                .value_name("PATH")
                .value_parser(value_parser!(PathBuf))
                .help("Write the match result as JSON to PATH"),
        )
}

fn list(games: &[RatedGame]) {
    for rated in games {
        let game = &rated.game;
        let declared = match game.difficulty {
            Some(d) => format!("difficulty {d:>2}"),
            None => "difficulty  -".to_owned(),
        };
        println!(
            "{declared} · rated {:>2} · {:>2} instr. max · {} ({})",
            rated.rating,
            game.allowed_pipeline_max_size,
            game.name.bold(),
            game.id()
        );
    }
}

fn run() -> Result<(), GameError> {
    let matches = cli().get_matches();
    if !io::stdout().is_terminal() {
        colored::control::set_override(false);
    }
    let engine = Engine::new();
    let files: Vec<PathBuf> = matches
        .get_many::<PathBuf>("games")
        .map(|paths| paths.cloned().collect())
        .unwrap_or_default();

    let from_files = !files.is_empty();
    let source: Box<dyn GameSource> = if from_files {
        Box::new(Files(files))
    } else if let Some(dir) = matches.get_one::<PathBuf>("pool") {
        Box::new(Directory(dir.clone()))
    } else {
        Box::new(Builtin)
    };
    let games = source.load(&engine)?;

    if matches.get_flag("check") || matches.get_flag("list") {
        if matches.get_flag("check") {
            println!(
                "{} game(s) of {} are valid:",
                games.len(),
                source.describe()
            );
        }
        list(&games);
        return Ok(());
    }

    let rounds = if from_files {
        games
    } else {
        let count = matches
            .get_one::<u16>("rounds")
            .map(|n| usize::from(*n))
            .unwrap_or(DEFAULT_ROUNDS);
        let chosen = select(games, matches.get_one::<u8>("difficulty").copied(), count)?;
        if chosen.len() < count {
            println!(
                "Only {} game(s) of {} fit; the match has {} round(s).",
                chosen.len(),
                source.describe(),
                chosen.len()
            );
        }
        chosen
    };

    let result =
        play_match(&rounds, &engine).map_err(|e| GameError(format!("terminal error: {e}")))?;
    if let Some(path) = matches.get_one::<PathBuf>("save_result") {
        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| GameError(format!("cannot serialize the result: {e}")))?;
        fs::write(path, json + "\n")
            .map_err(|e| GameError(format!("cannot write `{}`: {e}", path.display())))?;
        println!("Result written to `{}`.", path.display());
    }
    Ok(())
}

fn play_match(rounds: &[RatedGame], engine: &Engine) -> io::Result<MatchResult> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut clock = SystemClock::default();
    let mut results = Vec::new();
    for (i, rated) in rounds.iter().enumerate() {
        let end = play_round(
            rated,
            i + 1,
            rounds.len(),
            engine,
            &mut input,
            &mut out,
            &mut clock,
        )?;
        results.push(end.result);
        if end.quit {
            break;
        }
    }
    let result = MatchResult::new(results, rounds.len());
    writeln!(out)?;
    writeln!(out, "{}", "Match over".bold())?;
    for (i, round) in result.rounds.iter().enumerate() {
        let status = match (round.solved, round.revealed) {
            (true, false) => "solved".green(),
            (true, true) => "solved with the answer".yellow(),
            (false, _) => "not solved".red(),
        };
        writeln!(
            out,
            "  {}. {:<28} {:>3}/{MAX_ROUND_SCORE}  {status}",
            i + 1,
            round.name,
            round.score
        )?;
    }
    writeln!(
        out,
        "  {}",
        format!("Total: {}/{}", result.total, result.max_total).bold()
    )?;
    Ok(result)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{} {error}", "txfg:".red().bold());
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_line_is_consistent() {
        cli().debug_assert();
    }

    #[test]
    fn files_conflict_with_selection_options() {
        assert!(
            cli()
                .try_get_matches_from(["txfg", "a.json", "-d", "3"])
                .is_err()
        );
        assert!(cli().try_get_matches_from(["txfg", "-d", "11"]).is_err());
        assert!(cli().try_get_matches_from(["txfg", "-r", "0"]).is_err());
        let m = cli()
            .try_get_matches_from(["txfg", "-d", "4", "-r", "5"])
            .unwrap();
        assert_eq!(m.get_one::<u8>("difficulty"), Some(&4));
    }
}
