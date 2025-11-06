# XIAO ESP32S3 Sense Microphone Test

## Quick Start

For real-time audio streaming:
```bash
just s3-to-speakers
```

For recording audio to file:
```bash
just s3-record
```

## Usage

To stream audio from the serial port and play it using ffplay:

```bash
python3.9 serial_to_stdout.py | \
ffplay -nodisp -autoexit -f s16le -ar 16000 -af "atrim=start=0.5,asetpts=N/SR/TB,highpass=f=300" -
```

This command:
- Reads audio data from the serial port using `serial_to_stdout.py`
- Pipes the data to `ffplay` for real-time audio playback
- Uses 16-bit signed little-endian format at 16kHz sample rate

## Rust mic diagnostics vs streaming

The Rust mic app (XiaoEsp32s3-mic) has two modes controlled in `src/bin/main.rs`:

- Diagnostic mode: set `const DIAG: bool = true` (default in this repo).
  - Prints per-frame metrics over USB CDC (text only):
    - `DC` (offset), `RMS`, `Peak`, `ZCR` (zero-crossing rate 0..1), `f≈` (dominant freq Hz)
  - Healthy audio: f≈ moves with voice (≈100–1000 Hz), RMS varies with loudness, ZCR drops when quiet.
  - Steady tone (e.g., f≈ 1–3 kHz with high ZCR) = misconfiguration, not real mic audio.
  - USB CDC ignores baud (921600 has no effect). Ensure a single reader (close flash monitor).

- Streaming mode: set `const DIAG: bool = false`.
  - Streams raw `s16le` at 16 kHz over USB CDC (binary only). Do not attach a text monitor.
  - Example playback:
    ```bash
    python3 serial_to_stdout.py --port /dev/cu.usbmodem101 \
    | ffplay -hide_banner -loglevel warning -f s16le -ar 16000 -i - -nodisp -af volume=0.3
    ```

If diagnostics show a steady tone, please capture 2–3 diagnostic lines for: silence, speaking a vowel, and a clap/tap, then share them. That is typically sufficient to triage; no extra logs are required unless requested.

## Recording Microphone to File

To record microphone audio to a WAV file:

```bash
python3.9 serial_to_stdout.py \
| ffmpeg -y -f s16le -ar 16000 -ac 1 -i - \
  -af "atrim=start=0.5,asetpts=N/SR/TB" \
  -c:a pcm_s16le mic.wav
```

This command:
- Reads audio data from the serial port using `serial_to_stdout.py`
- Pipes the data to `ffmpeg` for recording
- Records mono audio (1 channel) at 16kHz sample rate
- Applies audio filtering to trim the first 0.5 seconds and reset timestamps
- Saves the output as `mic.wav` in PCM 16-bit format
- The `-y` flag overwrites existing files