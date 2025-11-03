import serial, numpy as np, matplotlib.pyplot as plt

ser = serial.Serial("/dev/cu.usbmodem1101", 921600)
plt.ion()
y = np.zeros(1000)
line, = plt.plot(y)
plt.ylim(-33000,33000)

while True:
    try:
        val = int(ser.readline().strip() or 0)
        y = np.roll(y, -1)
        y[-1] = val
        line.set_ydata(y)
        plt.pause(0.001)
    except ValueError:
        pass