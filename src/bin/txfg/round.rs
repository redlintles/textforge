//! The read-eval-print loop of one round.

use std::{
    io::{self, BufRead, Write},
    time::{Duration, Instant},
};

use colored::Colorize;

use crate::{
    catalog::Entry,
    engine::Engine,
    game::{RatedGame, lookup},
    score::{RoundResult, par_seconds, round_score},
    syntax::split_instructions,
};

/// Measures how long a round takes.
pub trait Clock {
    /// Starts measuring.
    fn restart(&mut self);
    /// Time since the last [`Clock::restart`].
    fn elapsed(&self) -> Duration;
}

/// The wall clock.
pub struct SystemClock(Instant);

impl Default for SystemClock {
    fn default() -> Self {
        SystemClock(Instant::now())
    }
}

impl Clock for SystemClock {
    fn restart(&mut self) {
        self.0 = Instant::now();
    }

    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }
}

/// Text of `!help`.
pub const HELP: &str = "\
Type one instruction per line to build your pipeline (the final `;` is optional).
The whole pipeline runs on the start text after each one.

  !what [name]   what an allowed instruction does (no name: list them all)
  !undo          remove the last instruction
  !reset         start the round again with an empty pipeline
  !pipeline      show your pipeline
  !status        show the start, current and goal texts
  !show_answer   show the reference answer (this round then scores 0)
  !skip          give up this round (scores 0)
  !help          this text
  !exit          end the match";

/// What [`play_round`] returns: the result, and whether the player ended the match.
pub struct RoundEnd {
    /// The result of the round.
    pub result: RoundResult,
    /// The player typed `!exit` or closed the input.
    pub quit: bool,
}

struct State<'g> {
    rated: &'g RatedGame,
    pipeline: Vec<String>,
    text: String,
    revealed: bool,
}

impl State<'_> {
    fn result(&self, solved: bool, seconds: f64) -> RoundResult {
        let game = &self.rated.game;
        RoundResult {
            game: game.id().to_owned(),
            name: game.name.clone(),
            difficulty: self.rated.difficulty(),
            solved,
            revealed: self.revealed,
            instructions: self.pipeline.len(),
            seconds,
            score: round_score(
                game.answer.len(),
                self.pipeline.len(),
                seconds,
                solved,
                self.revealed,
            ),
            pipeline: self.pipeline.clone(),
        }
    }
}

fn show(text: &str) -> String {
    format!("{text:?}")
}

fn describe(entry: &Entry) -> String {
    format!(
        "  {:<7} {:<36} {}",
        entry.name.bold(),
        entry.syntax,
        entry.effect.replace('`', "")
    )
}

/// Plays one round, reading the player's lines from `input`.
pub fn play_round<R: BufRead, W: Write, C: Clock>(
    rated: &RatedGame,
    number: usize,
    total: usize,
    engine: &Engine,
    input: &mut R,
    out: &mut W,
    clock: &mut C,
) -> io::Result<RoundEnd> {
    let game = &rated.game;
    writeln!(out)?;
    writeln!(
        out,
        "{} {}",
        format!("Round {number}/{total}:").bold(),
        game.name.bold()
    )?;
    if !game.description.is_empty() {
        writeln!(out, "{}", game.description.dimmed())?;
    }
    writeln!(
        out,
        "difficulty {} · up to {} instructions · par {:.0} s · allowed: {}",
        rated.difficulty(),
        game.allowed_pipeline_max_size,
        par_seconds(game.answer.len()),
        game.allowed_instructions.join(" ")
    )?;
    writeln!(out, "start: {}", show(&game.start))?;
    writeln!(out, "goal:  {}", show(&game.goal).green())?;
    writeln!(out, "{}", "Type !help for the commands.".dimmed())?;

    let mut state = State {
        rated,
        pipeline: Vec::new(),
        text: game.start.clone(),
        revealed: false,
    };
    clock.restart();
    let mut line = String::new();
    loop {
        write!(
            out,
            "[{}/{}] > ",
            state.pipeline.len(),
            game.allowed_pipeline_max_size
        )?;
        out.flush()?;
        line.clear();
        if input.read_line(&mut line)? == 0 {
            writeln!(out)?;
            let seconds = clock.elapsed().as_secs_f64();
            return Ok(RoundEnd {
                result: state.result(false, seconds),
                quit: true,
            });
        }
        let typed = line.trim();
        if typed.is_empty() {
            continue;
        }
        if let Some(command) = typed.strip_prefix('!') {
            let mut parts = command.split_whitespace();
            let name = parts.next().unwrap_or("");
            let argument = parts.next();
            match name {
                "exit" | "quit" => {
                    let seconds = clock.elapsed().as_secs_f64();
                    return Ok(RoundEnd {
                        result: state.result(false, seconds),
                        quit: true,
                    });
                }
                "skip" => {
                    writeln!(out, "Round skipped.")?;
                    let seconds = clock.elapsed().as_secs_f64();
                    return Ok(RoundEnd {
                        result: state.result(false, seconds),
                        quit: false,
                    });
                }
                "help" => writeln!(out, "{HELP}")?,
                "reset" => {
                    state.pipeline.clear();
                    state.text = game.start.clone();
                    writeln!(out, "Pipeline cleared. text: {}", show(&state.text))?;
                }
                "undo" => {
                    if state.pipeline.pop().is_none() {
                        writeln!(out, "The pipeline is already empty.")?;
                    } else {
                        state.text = engine
                            .run(&state.pipeline, &game.start)
                            .unwrap_or_else(|_| game.start.clone());
                        writeln!(out, "text: {}", show(&state.text))?;
                    }
                }
                "pipeline" => {
                    if state.pipeline.is_empty() {
                        writeln!(out, "The pipeline is empty.")?;
                    }
                    for (i, instruction) in state.pipeline.iter().enumerate() {
                        writeln!(out, "  {}. {instruction}", i + 1)?;
                    }
                }
                "status" => {
                    writeln!(out, "start: {}", show(&game.start))?;
                    writeln!(out, "now:   {}", show(&state.text))?;
                    writeln!(out, "goal:  {}", show(&game.goal).green())?;
                }
                "what" => what(rated, argument, out)?,
                "show_answer" => {
                    state.revealed = true;
                    writeln!(out, "A reference answer (this round now scores 0):")?;
                    for instruction in &game.answer {
                        writeln!(out, "  {instruction}")?;
                    }
                }
                _ => writeln!(out, "Unknown command `!{name}`; type !help.")?,
            }
            continue;
        }

        let instructions = split_instructions(typed);
        if instructions.len() > 1 {
            writeln!(
                out,
                "{}",
                format!(
                    "One instruction per line: this line has {}. Type them one at a time.",
                    instructions.len()
                )
                .yellow()
            )?;
            continue;
        }
        for instruction in instructions {
            if state.pipeline.len() >= game.allowed_pipeline_max_size {
                writeln!(
                    out,
                    "{}",
                    format!(
                        "The pipeline is full ({} instructions): use !undo or !reset.",
                        game.allowed_pipeline_max_size
                    )
                    .yellow()
                )?;
                break;
            }
            if let Err(refusal) = game.check(&instruction) {
                writeln!(out, "{}", format!("{refusal}.").yellow())?;
                break;
            }
            state.pipeline.push(instruction);
            match engine.run(&state.pipeline, &game.start) {
                Ok(text) => {
                    state.text = text;
                    writeln!(out, "  → {}", show(&state.text))?;
                }
                Err(error) => {
                    state.pipeline.pop();
                    writeln!(out, "{}", error.error_code.message().as_ref().red())?;
                    break;
                }
            }
            if state.text == game.goal {
                let seconds = clock.elapsed().as_secs_f64();
                let result = state.result(true, seconds);
                writeln!(
                    out,
                    "{} {} instruction(s) in {:.1} s: {} points.",
                    "Solved!".green().bold(),
                    result.instructions,
                    seconds,
                    result.score.to_string().bold()
                )?;
                return Ok(RoundEnd {
                    result,
                    quit: false,
                });
            }
        }
    }
}

fn what<W: Write>(rated: &RatedGame, name: Option<&str>, out: &mut W) -> io::Result<()> {
    let game = &rated.game;
    let Some(name) = name else {
        writeln!(out, "Instructions of this round:")?;
        for allowed in &game.allowed_instructions {
            if let Some(entry) = lookup(allowed) {
                writeln!(out, "{}", describe(entry))?;
            }
        }
        return Ok(());
    };
    match lookup(name) {
        Some(entry) if game.allows(name) => writeln!(out, "{}", describe(entry)),
        Some(_) => writeln!(
            out,
            "`{name}` is a TextForge instruction, but this round does not allow it."
        ),
        None => writeln!(out, "`{name}` is not a TextForge instruction."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    struct FixedClock(Duration);

    impl Clock for FixedClock {
        fn restart(&mut self) {}
        fn elapsed(&self) -> Duration {
            self.0
        }
    }

    fn intro() -> RatedGame {
        let json = include_str!("games/02-textforge-introduction.json");
        Game::parse(json).unwrap().validate(&Engine::new()).unwrap()
    }

    fn play(input: &str) -> (RoundEnd, String) {
        colored::control::set_override(false);
        let mut out = Vec::new();
        let mut clock = FixedClock(Duration::from_secs(5));
        let end = play_round(
            &intro(),
            1,
            3,
            &Engine::new(),
            &mut input.as_bytes(),
            &mut out,
            &mut clock,
        )
        .unwrap();
        (end, String::from_utf8(out).unwrap())
    }

    #[test]
    fn solves_a_round() {
        let (end, out) = play("tbs\nate \" fruit!\";\ntua\n");
        assert!(end.result.solved && !end.quit, "{out}");
        assert_eq!(end.result.score, 100);
        assert_eq!(end.result.pipeline, ["tbs;", "ate \" fruit!\";", "tua;"]);
        assert!(out.contains("→ \"banana\""), "{out}");
        assert!(out.contains("Solved!"), "{out}");
    }

    #[test]
    fn refuses_unknown_and_disallowed_instructions() {
        let (end, out) = play("rev\nzzz\n!what rev\n!what zzz\n!what tbs\n!what\n");
        assert!(end.quit && !end.result.solved);
        assert_eq!(end.result.instructions, 0);
        assert!(out.contains("`rev` is a TextForge instruction, but this round does not allow it"));
        assert!(out.contains("`zzz` is not a TextForge instruction"));
        assert!(out.contains("Trim whitespace on both sides."));
        assert!(out.contains("Instructions of this round:"));
    }

    #[test]
    fn errors_are_not_counted() {
        let (end, out) = play("ate\ntbs\n!pipeline\n!exit\n");
        assert_eq!(end.result.pipeline, ["tbs;"], "{out}");
        assert!(end.quit);
    }

    #[test]
    fn undo_reset_and_answer() {
        let (end, out) =
            play("tua\n!undo\n!undo\ntbs\n!reset\n!show_answer\ntbs\nate \" fruit!\"\ntua\n");
        assert!(out.contains("already empty"), "{out}");
        assert!(out.contains("Pipeline cleared"), "{out}");
        assert!(end.result.solved && end.result.revealed);
        assert_eq!(end.result.score, 0);
    }

    #[test]
    fn one_instruction_per_line() {
        let (end, out) = play("tbs; tua\ntbs;;\nate \"a;b\"\n");
        assert!(
            out.contains("One instruction per line: this line has 2"),
            "{out}"
        );
        assert_eq!(end.result.pipeline, ["tbs;", "ate \"a;b\";"], "{out}");
    }

    #[test]
    fn a_full_pipeline_refuses_more() {
        let (end, out) = play(&"trs\n".repeat(11));
        assert_eq!(end.result.instructions, 10);
        assert!(out.contains("The pipeline is full"), "{out}");
    }

    #[test]
    fn skip_ends_the_round_without_quitting() {
        let (end, out) = play("!skip\n");
        assert!(!end.quit && !end.result.solved, "{out}");
        let (_, out) = play("!nope\n");
        assert!(out.contains("Unknown command"));
    }
}
