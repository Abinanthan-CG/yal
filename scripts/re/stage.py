"""Copies the Raycast binaries and bundles into a plain working folder.

The install lives under Program Files\\WindowsApps, which is awkward to read
with ordinary tools. This copies what the analysis needs somewhere plain.

    python stage.py <raycast install dir> <destination>
"""
import os
import shutil
import sys

source = sys.argv[1]
destination = sys.argv[2]

WANTED_FILES = {
    "Raycast.dll",
    "Raycast.exe",
    "native.dll",
    "Raycast.UIAccess.exe",
    "Raycast.SystemService.exe",
    "Raycast.StartupLauncher.exe",
    "Raycast.PerformanceMonitor.exe",
    "Raycast.TraceRecorder.exe",
    "Raycast.CrashHelper.exe",
}

WANTED_DIRS = ["backend", "frontend", "api"]

# The api folder is mostly third-party packages; only Raycast's own are worth
# reading, so the rest are left where they are.
KEEP_IN_API = ("@raycast", "template")


def copy_file(path, target):
    if os.path.exists(target):
        return "have"
    shutil.copyfile(path, target)
    return "got"


def wanted(relative):
    if not relative.startswith("api"):
        return True
    return any(part in relative for part in KEEP_IN_API)


def main():
    os.makedirs(destination, exist_ok=True)
    counts = {"got": 0, "have": 0, "skipped": 0}
    for name in sorted(os.listdir(source)):
        full = os.path.join(source, name)
        if os.path.isfile(full) and name in WANTED_FILES:
            result = copy_file(full, os.path.join(destination, name))
            counts[result] += 1
            print(f"{name:34} {result}")
        elif os.path.isdir(full) and name in WANTED_DIRS:
            for dirpath, _dirs, files in os.walk(full):
                for entry in files:
                    path = os.path.join(dirpath, entry)
                    relative = os.path.relpath(path, source)
                    if not wanted(relative):
                        counts["skipped"] += 1
                        continue
                    target = os.path.join(destination, relative)
                    os.makedirs(os.path.dirname(target), exist_ok=True)
                    result = copy_file(path, target)
                    counts[result] += 1
                    print(f"{relative[:60]:60} {result}")
    print(f"\n{counts}")


if __name__ == "__main__":
    main()
