"""Reads the schema of Raycast's local databases.

    python db-schema.py <raycast data folder> <output folder>

The databases are copied first, because the running application holds them
open. Only the *structure* is reported - table names, columns and indexes -
never the user's own content.
"""
import os
import shutil
import sqlite3
import sys

source = sys.argv[1]
destination = sys.argv[2]
os.makedirs(destination, exist_ok=True)

for name in sorted(os.listdir(source)):
    if not name.endswith(".db"):
        continue
    copy = os.path.join(destination, name)
    try:
        shutil.copyfile(os.path.join(source, name), copy)
        for extra in ("-wal", "-shm"):
            side = os.path.join(source, name + extra)
            if os.path.exists(side):
                shutil.copyfile(side, copy + extra)
    except OSError as error:
        print(f"{name}: could not copy ({error})")
        continue

    print(f"\n===== {name} =====")
    try:
        connection = sqlite3.connect(copy)
        cursor = connection.cursor()
        rows = cursor.execute(
            "SELECT type, name, sql FROM sqlite_master "
            "WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name"
        ).fetchall()
        if not rows:
            print("  (no tables - the file is not a readable SQLite database)")
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
            else:
                print(f"\n  {kind.upper()} {table}")
            if sql:
                print("    " + " ".join(sql.split()))
            if kind == "table":
                try:
                    columns = cursor.execute(
                        f'PRAGMA table_info("{table}")'
                    ).fetchall()
                    print("    columns: " + ", ".join(
                        f"{column[1]}:{column[2]}" for column in columns
                    ))
                except sqlite3.Error:
                    pass
        connection.close()
    except sqlite3.Error as error:
        print(f"  not readable as SQLite: {error}")
