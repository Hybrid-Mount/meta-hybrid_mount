from telethon import TelegramClient
import asyncio
from telethon.sessions import StringSession
import os

API_ID = 611335
API_HASH = "d524b414d21f4d37f08684c1df41ac9c"

BOT_TOKEN = os.environ.get("BOT_TOKEN")
CHAT_ID = int(os.environ.get("CHAT_ID"))
COMMIT_URL = os.environ.get("COMMIT_URL")
COMMIT_MESSAGE = os.environ.get("COMMIT_MESSAGE")
BOT_CI_SESSION = os.environ.get("BOT_CI_SESSION")
ANOTHER = os.environ.get("ANOTHER")
MSG_TEMPLATE = """
New push to Github
```
{commit_message}
```
by {another}
See commit detail [here]({commit_url})
""".strip()


def get_caption():
    msg = MSG_TEMPLATE.format(
        commit_message=COMMIT_MESSAGE,
        commit_url=COMMIT_URL,
        another=ANOTHER,
    )
    if len(msg) > 1024:
        return COMMIT_URL
    return msg


async def send_telegram_message():
    async with TelegramClient(StringSession(BOT_CI_SESSION), api_id=API_ID, api_hash=API_HASH) as client:
        await client.start(bot_token=BOT_TOKEN)
        print("[+] Caption: ")
        print("---")
        print("---")
        print("[+] Sending")
        await client.send_file(
            entity=CHAT_ID,
            caption=get_caption(),
            file=["./output/magic_mount_rs.zip"],
            parse_mode="markdown",
        )

if __name__ == '__main__':
    asyncio.run(send_telegram_message())
