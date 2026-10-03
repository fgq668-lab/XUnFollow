"""One opt-in read-only timeline request; credentials never appear in output/artifacts."""
import json
import os
import sqlite3
import sys
import urllib.parse
import urllib.request
from pathlib import Path

source = sqlite3.connect(Path(sys.argv[1]).resolve().as_uri() + "?mode=ro", uri=True)
key = source.execute("SELECT value FROM settings WHERE key='provider_api_key'").fetchone()[0]
source.close()
username = sys.argv[2]
if not username.isascii() or not username.replace('_', '').isalnum() or len(username) > 15:
    raise SystemExit("Invalid public handle")
url = "https://api.twitterapi.io/twitter/user/last_tweets?" + urllib.parse.urlencode({
    "userName": username, "includeReplies": "false", "cursor": ""
})
request = urllib.request.Request(url, headers={"X-API-Key": key})
try:
    with urllib.request.urlopen(request, timeout=45) as response:
        raw = response.read(4_000_001)
except Exception as error:
    raise SystemExit("Timeline request failed; possible charge; no automatic retry: " + type(error).__name__)
if len(raw) > 4_000_000:
    raise SystemExit("Response too large; possible charge; no retry")
payload = json.loads(raw.decode().replace(key, "[redacted]"))
destination = Path(sys.argv[3]).resolve()
with os.fdopen(os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), 'w') as output:
    json.dump(payload, output, ensure_ascii=False)
print('Saved private API artifact:', destination)
print('Root fields:', list(payload))
container = payload.get('data', payload)
print('Data fields:', list(container) if isinstance(container, dict) else type(container).__name__)
tweets = container.get('tweets', []) if isinstance(container, dict) else []
print('Returned:', len(tweets), 'cost estimate USD:', max(1, len(tweets)) * 0.00015)
for tweet in tweets[:20]:
    print(json.dumps({k: tweet.get(k) for k in ['id', 'text', 'createdAt', 'likeCount', 'replyCount', 'viewCount', 'isReply', 'lang']}, ensure_ascii=False))
