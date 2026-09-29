import json
import sys
import time
import urllib.request
from playwright.sync_api import sync_playwright

BASE = "http://127.0.0.1:5173"
OUT = "/tmp/opencode/shots"
VIEWPORT = {"width": 1600, "height": 1000}


def api(path, method="GET", payload=None, timeout=600):
    data = json.dumps(payload).encode() if payload is not None else None
    request = urllib.request.Request(f"{BASE}{path}", data=data, method=method)
    if data:
        request.add_header("content-type", "application/json")
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.loads(response.read().decode())


def seed_conversation():
    session = api("/session", method="POST", payload={})
    session_id = session["id"]
    prompt = (
        "You must use your tools for this task and must not answer from memory. "
        "First call the bash tool to run `echo menzi-screenshot-probe`. Then call "
        "the read tool on /etc/hostname. Then reply with exactly two short lines: "
        "the echo output, then the hostname you read."
    )
    try:
        api(
            f"/session/{session_id}/message",
            method="POST",
            payload={"parts": [{"type": "text", "text": prompt}]},
            timeout=240,
        )
    except Exception as error:
        print(f"warning: tool-forcing turn failed: {error!r}")

    parts = api(f"/session/{session_id}/message", timeout=60)
    tool_count = sum(
        1 for message in parts for part in message.get("parts", []) if part.get("type") == "tool"
    )
    print(f"tool parts in seeded session: {tool_count}")
    return session_id


def capture(page, name, url, settle=2500):
    page.goto(url, wait_until="networkidle", timeout=60000)
    page.wait_for_timeout(settle)
    path = f"{OUT}/{name}.png"
    page.screenshot(path=path, full_page=True)
    print(f"{path}  title={page.title()!r}")
    return path


def main():
    import os

    os.makedirs(OUT, exist_ok=True)
    session_id = seed_conversation()
    print(f"session={session_id}")

    payload = api("/api/v1/projects")
    if isinstance(payload, list):
        items = payload
    else:
        items = payload.get("data") or payload.get("projects") or []
    project_id = items[0]["id"] if items else None
    print(f"project={project_id}")

    errors = []
    with sync_playwright() as play:
        browser = play.chromium.launch(args=["--no-sandbox"])
        context = browser.new_context(viewport=VIEWPORT, device_scale_factor=2)
        page = context.new_page()
        page.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
        page.on("pageerror", lambda e: errors.append(str(e)))

        capture(page, "01-projects", f"{BASE}/projects")

        if project_id:
            capture(
                page,
                "02-code-chat",
                f"{BASE}/projects/{project_id}/code?session={session_id}",
                settle=6000,
            )
            capture(
                page,
                "03-code-review",
                f"{BASE}/projects/{project_id}/code?session={session_id}&panel=review",
                settle=4000,
            )
            capture(page, "04-design", f"{BASE}/projects/{project_id}/design")
            capture(page, "05-project", f"{BASE}/projects/{project_id}")

        page.set_viewport_size({"width": 420, "height": 900})
        if project_id:
            capture(
                page,
                "06-mobile-code",
                f"{BASE}/projects/{project_id}/code?session={session_id}",
                settle=4000,
            )

        page.set_viewport_size(VIEWPORT)
        page.emulate_media(color_scheme="dark")
        capture(page, "07-dark", f"{BASE}/projects", settle=2500)

        browser.close()

    print("console errors:", json.dumps(errors[:10], indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
