"""Opens a Raycast database by restoring the SQLite magic it replaced.

    python db-restore.py <database copy> <output folder>

Raycast's databases keep the 16 bytes "SQLite format 3\\0" replaced with
random data, which is why a plain reader says "file is not a database". The
rest of the file is an ordinary SQLite database. This writes the magic back
into a copy and reports the structure - table names, columns, indexes and row
counts, never the user's own content.
"""
import os
import shutil
import sqlite3
import sys

source = sys.argv[1]
destination = sys.argv[2]
os.makedirs(destination, exist_ok=True)

MAGIC = b"SQLite format 3\x00"


def restore(path, out):
    shutil.copyfile(path, out)
    with open(out, "r+b") as handle:
        handle.seek(0)
        head = handle.read(16)
        handle.seek(0)
        handle.write(MAGIC)
    return head


for name in sorted(os.listdir(source)):
    if not name.endswith(".db"):
        continue
    path = os.path.join(source, name)
    out = os.path.join(destination, name)
    head = restore(path, out)

    print(f"\n===== {name} =====")
    print(f"  original first 16 bytes: {head.hex(' ')}")
    try:
        connection = sqlite3.connect(out)
        cursor = connection.cursor()
        rows = cursor.execute(
            "SELECT type, name, sql FROM sqlite_master "
            "WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name"
        ).fetchall()
        if not rows:
            print("  (no tables)")
            continue
        for kind, table, sql in rows:
            if kind == "table":
                try:
                    count = cursor.execute(
                        f'SELECT COUNT(*) FROM "{table}"'
                    ).fetchone()[0]
                except sqlite3.Error:
                    count = "?"
                print(f"\n  TABLE {table}  ({count} rows)")
                try:
                    columns = cursor.execute(f'PRAGMA table_info("{table}")').fetchall()
                    print("    columns: " + ", ".join(
                        f"{c[1]}:{c[2]}" for c in columns))
                except sqlite3.Error:
                    pass
            else:
                print(f"\n  {kind.upper()} {table}")
            if sql:
                print("    " + " ".join(sql.split()))
        connection.close()
    except sqlite3.Error as error:
        print(f"  still not readable: {error}")
