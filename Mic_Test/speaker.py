# pip install pyserial
# python serial_to_stdout.py | ffplay -autoexit -nodisp -f s16le -ar 16000 -ac 1 -
import sys, serial
ser=serial.Serial("/dev/tty.usbmodem*", 921600)
while True:
    b=ser.read(4096)
    if not b: continue
    sys.stdout.buffer.write(b); sys.stdout.flush()