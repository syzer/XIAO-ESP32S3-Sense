# pip install pyserial
import sys, glob, wave, serial

PORT = "/dev/cu.usbmodem1101"  # adjust; or auto-pick below
# cands = glob.glob("/dev/cu.usbmodem*")+glob.glob("/dev/cu.usbserial*"); PORT = cands[0]

BAUD = 921600
SR   = 16000
SECONDS = 10  # set duration

ser = serial.Serial(PORT, BAUD, timeout=0)
ser.dtr = False; ser.rts = False
ser.reset_input_buffer()

with wave.open("mic.wav","wb") as wav:
    wav.setnchannels(1)
    wav.setsampwidth(2)     # 16-bit
    wav.setframerate(SR)

    bytes_needed = SECONDS * SR * 2  # mono, 16-bit
    written = 0
    while written < bytes_needed:
        chunk = ser.read(min(4096, bytes_needed - written))
        if not chunk: continue
        wav.writeframes(chunk)
        written += len(chunk)

print("Saved mic.wav")