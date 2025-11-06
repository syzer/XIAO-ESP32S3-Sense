import serial, sys

PORT = "/dev/cu.usbmodem2101"  # adjust if needed
BAUD = 921600

ser = serial.Serial(PORT, BAUD)
ser.dtr = False
ser.rts = False

while True:
    data = ser.read(4096)
    if not data:
        continue
    sys.stdout.buffer.write(data)
    sys.stdout.flush()