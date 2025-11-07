# Agent Verification Procedure

Every development step should be validated the same way: run the firmware for a short, bounded window and inspect the output.
Remember that is esp32s3 xiao sense board, so the serial port is on the USB-C port.

## Quick Check Command
Use a 30‑second timeout:

```fish
export PATH="$HOME/.cargo/bin:$PATH" && export RUSTUP_TOOLCHAIN=esp && source ~/export-esp.sh && unbuffer timeout 30s cargo run --bin xiao_esp32s3_mic
```

## Initial Check
if 
```
Error:   × Failed to open serial port /dev/cu.usbmodem2101
  ╰─▶ Error while connecting to device
```
run one liner to kill all processes blocking port
```
kill -9 $(lsof -t /dev/cu.usb* 2>/dev/null)

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
