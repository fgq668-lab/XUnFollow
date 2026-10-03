"""Check git-visible files against local API keys without printing credentials."""
import sqlite3
import subprocess
import sys
from pathlib import Path

connection = sqlite3.connect(Path(sys.argv[1]).resolve().as_uri() + "?mode=ro", uri=True)
keys = [str(row[0]).encode() for row in connection.execute(
    "SELECT value FROM settings WHERE key IN ('provider_api_key','deepseek_api_key')"
) if len(str(row[0])) >= 12]
connection.close()
files = subprocess.check_output(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"]).split(b"\0")
found = []
checked = 0
for name in files:
    if not name:
        continue
    path = Path(name.decode())
    if not path.is_file():
        continue
    checked += 1
    contents = path.read_bytes()
    if any(key in contents for key in keys):
        found.append(str(path))
print(f"Checked {checked} git-visible files against {len(keys)} local credentials.")
if found:
    print("Credential matches found in paths:", *found, sep="\n")
    sys.exit(1)
print("No local API keys found; credential values were not printed.")
