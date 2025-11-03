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