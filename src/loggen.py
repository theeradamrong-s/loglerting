import time
import random

messages = [
    "System initialized",
    "Warning: Voltage spike detected",
    "System recovered",
    "Error: Sensor timeout",
    "Info: Routine check completed",
    "Warning: Temperature high",
    "Info: Communication restored",
    "Error: Disk write failed",
    "Info: Calibration complete",
    "Warning: Low memory condition"
]

log_file = "test.log"

print(f"Generating logs into {log_file}... (Ctrl+C to stop)")

while True:
    with open(log_file, "a") as f:
        msg = random.choice(messages)
        f.write(f"{msg}\n")
    time.sleep(0.5)
