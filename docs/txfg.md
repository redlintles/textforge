# `txfg`, the TextForge game

`txfg` teaches the TextForge language with short puzzles. Each round gives a
start text and a goal text; the player builds a pipeline, one instruction at a
time, until the text matches the goal. It is a separate program behind the
`game` feature:

```sh
cargo install textforge --features game
txfg                      # 3 rounds: the easiest built-in games
txfg -d 6                 # the hardest built-in games with difficulty 6 or less
txfg -d 6 -r 5            # the same, 5 rounds
txfg my-game.json b.json  # one round per file, in this order
txfg --pool games/ -d 4   # choose the rounds from a directory of game files
txfg --list               # the games, with their difficulty and rating
txfg --check my-game.json # validate a game file and print its rating
txfg --save-result r.json # also write the match result as JSON
```

## Playing

At the prompt (`[used/max] >`) the player types instructions in the text
syntax ([text syntax](text-syntax.md)); the final `;` of a line is optional and
a line may hold several instructions. After each instruction the whole pipeline
runs again on the start text and the new text is shown. An instruction that
fails, that does not exist or that the round does not allow is not added.

| Command | Effect |
|---|---|
| `!what [name]` | What an allowed instruction does. Without a name, every allowed instruction. For an instruction that exists but is not allowed, or that does not exist, it says so. |
| `!undo` | Remove the last instruction. |
| `!reset` | Empty the pipeline and start from the start text. |
| `!pipeline` | Show the pipeline. |
| `!status` | Show the start, current and goal texts. |
| `!show_answer` | Show the reference answer; the round then scores 0, but can still be finished. |
| `!skip` | Give up the round (0 points). |
| `!help` | The list of commands. |
| `!exit` | End the match; the rounds played so far are scored. |

## Scoring

A round scores 0 to 100; a match adds the rounds up (out of 100 per round).

- Not solved, or solved after `!show_answer`: 0.
- Instructions, 70 points × `answer / used`, at most 1: a pipeline as short as
  the reference answer (or shorter) gets all 70.
- Time, 30 points × `par / seconds`, at most 1. The par time is 20 s plus 15 s
  per instruction of the reference answer.

`--save-result` writes the rounds (game id, difficulty, instructions, seconds,
score and final pipeline) and the total as JSON, with a `format` version, so a
future ranking can read it.

## Game files

One JSON file is one round. Unknown keys are errors, so typos are caught.

```json
{
  "id": "me/banana",
  "name": "Textforge Introduction",
  "description": "The basics: trimming, appending and case.",
  "author": "me",
  "difficulty": 3,
  "start": " banana    ",
  "goal": "BANANA FRUIT!",
  "allowed_instructions": ["tbs", "trs", "tls", "ate", "tua"],
  "allowed_pipeline_max_size": 10,
  "answer": ["tbs;", "ate \" fruit!\";", "tua;"]
}
```

| Key | Required | Meaning |
|---|---|---|
| `name` | yes | Title shown to the player. |
| `start`, `goal` | yes | The texts; they must differ. Use JSON escapes for control characters (`"a\nb"`). |
| `allowed_instructions` | yes | Mnemonics the player may use, each at most once. Nested instructions (`iter … times X`, `onln … apply X`, `ifdc … do X`, `blk … assoc X`) are checked too. |
| `allowed_pipeline_max_size` | yes | How many instructions the pipeline may have, 1 to 100. |
| `answer` | yes | A reference pipeline, each instruction ending with `;`. It may use only allowed instructions, must fit in the size limit, and is run when the game loads: it must turn `start` into `goal`. |
| `difficulty` | no | 0 to 10, chosen by the author. When absent, the rating is used. |
| `description`, `author` | no | Shown to the player / for game collections. |
| `id` | no | Stable identifier for results and future game repositories; the name when absent. |
| `format` | no | File format version, `1`. |

File instructions (`wtf`, `apf`, `ppf`, `inf`, `ppjn`) cannot be allowed:
loading a game runs its answer, and a game from someone else must not read or
write the player's files. A round also runs with low limits (1 MiB of generated
text, 100,000 `iter` turns).

### Rating

`txfg` rates every game from 0 to 10, shown by `--list` and `--check`:

| Part | Points | Measure |
|---|---|---|
| Steps | 0-4 | Instruction names in the answer, nested ones included (8 or more: 4). |
| Arguments | 0-2 | Arguments in the answer (8 or more: 2). |
| Distractors | 0-2 | Allowed instructions the answer does not use (8 or more: 2). |
| Tightness | 0-2 | Room left by `allowed_pipeline_max_size`: none, 2; as many as the steps or more, 0. |

The match uses the author's `difficulty` when there is one: the rating is a
guide for authors. With `-d N`, `txfg` takes the hardest games with difficulty
`N` or less and plays them from the easiest to the hardest; without `-d`, the
easiest ones.

## Code

`src/bin/txfg`, built only with the `game` feature (`required-features`):

| File | Role |
|---|---|
| `main.rs` | Command line (`clap`) and the match loop. |
| `game.rs` | The game file, its validation and the rating. |
| `round.rs` | The prompt of one round; the clock is a trait so tests use a fixed one. |
| `score.rs` | Round scores and the serializable `RoundResult`/`MatchResult`. |
| `source.rs` | `GameSource` (built-in games, files, directories) and the round selection. |
| `engine.rs` | Runs a round's pipeline through the public API. |
| `syntax.rs` | Splits a line into instructions and words, to find instruction names. |
| `catalog.rs` | Syntax and effect of every instruction, generated by `scripts/instruction_index.py`. |
| `games/*.json` | The built-in games. |

The library reads text pipelines only from files, so `engine.rs` writes the
round's pipeline to a temporary `.textforge` file before each run.

Rankings and a game repository are not part of this version. They fit in two
places: a new `GameSource` that fetches games, and a consumer of the
`MatchResult` JSON. Both identify games by `id`.
