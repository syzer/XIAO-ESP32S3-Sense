# Agent Verification Procedure

Every development step should be validated the same way: run the firmware for a short, bounded window and inspect the output.

## Quick Check Command
Use a 20‑second timeout:

```fish
timeout 20s cargo run
```

## What This Does
- Builds the current code.
- Runs it for up to 20 seconds.
- Stops automatically so you can iterate quickly.

## Success Indicators
- Program starts without a panic.
- You see frame/sample lines (synthetic or real mic data) before timeout.
- Exit code is 0 (or the process halts intentionally after printing a completion line).

## Failure Indicators
- Exit code 101 (often a panic or build failure) – inspect the first error lines.
- No output at all – initialization may have stalled early.

## Adjusting As We Add Real Mic Logic
Once real PDM capture replaces the synthetic data, the same command should yield raw PCM frame summaries. If silence occurs, we will switch slot from RIGHT to LEFT in code and re‑run the same 20‑second check.

## Loop
1. Modify code.
2. `timeout 20s cargo run`
3. Observe output & exit code.
4. Repeat.

This keeps feedback fast and consistent for the agent and human collaborators.
