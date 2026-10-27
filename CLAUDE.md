@AGENTS.md

## Claude Code

- Run parallel work in worktrees (`claude --worktree <name>`) and set a separate `CARGO_TARGET_DIR`
  for each one, as described in AGENTS.md section 6.
- The instruction design guide lives in `.agents/skills/atp-project/references/`, which Claude Code
  does not load on its own. Read it when the task is adding or changing an instruction.
