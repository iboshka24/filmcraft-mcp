#!/usr/bin/env python3
"""Build the README demo by calling the FilmCraft MCP HTTP tools.

Start the server first:

    cargo run -p filmcraft-mcp -- http --bind 127.0.0.1:8765

Then:

    python3 scripts/demo_edit.py docs/demo

The timeline is about four and a half seconds at 24 fps, 640x360:

  0.0–1.0   color bars
  1.0–2.2   graded red, with the title
  2.2–3.3   amber, slight blur, dissolve into blue
  3.3–4.5   blue
  audio     bars and tone on A1, −8 dB, fade out
"""

import json
import sys
import urllib.request
from pathlib import Path

URL = "http://127.0.0.1:8765/mcp"
TICKS = 254_016_000_000


def ticks(seconds: float) -> int:
    return int(round(seconds * TICKS))


def call(name, arguments, req_id):
    body = json.dumps(
        {
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments},
        }
    ).encode()
    request = urllib.request.Request(
        URL,
        data=body,
        headers={
            "Content-Type": "application/json",
            "Host": "127.0.0.1:8765",
            "Content-Length": str(len(body)),
        },
    )
    with urllib.request.urlopen(request, timeout=180) as response:
        payload = json.load(response)
    if "error" in payload:
        raise SystemExit(f"{name}: {payload['error']}")
    result = payload["result"]
    text = result["content"][0]["text"]
    data = json.loads(text)
    if result.get("isError"):
        raise SystemExit(f"{name}: {data}")
    return data


def main():
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "docs/demo")
    out.mkdir(parents=True, exist_ok=True)
    n = 1

    def step(name, arguments):
        nonlocal n
        value = call(name, arguments, n)
        print(f"{n:02d} {name}: {json.dumps(value)[:240]}")
        n += 1
        return value

    step("sequence_new", {"name": "MCP Demo", "width": 640, "height": 360, "fps": 24, "video": 2, "audio": 2})
    red = step("generate_color_matte", {"color": "#c0392b", "seconds": 3, "name": "Red", "width": 640, "height": 360})
    amber = step("generate_color_matte", {"color": "#f1c40f", "seconds": 3, "name": "Amber", "width": 640, "height": 360})
    blue = step("generate_color_matte", {"color": "#2980b9", "seconds": 3, "name": "Blue", "width": 640, "height": 360})
    bars = step("generate_bars_and_tone", {"seconds": 5, "name": "Bars", "width": 640, "height": 360})
    red_id, amber_id, blue_id, bars_id = red["item"], amber["item"], blue["item"], bars["item"]

    bars_clip = step(
        "edit_overwrite",
        {"item": bars_id, "track": "V1", "seconds": 0, "durationSeconds": 1.0, "video": True, "audio": False},
    )
    red_clip = step(
        "edit_overwrite",
        {"item": red_id, "track": "V1", "seconds": 1.0, "durationSeconds": 1.25, "video": True, "audio": False},
    )
    amber_clip = step(
        "edit_overwrite",
        {"item": amber_id, "track": "V1", "seconds": 2.25, "durationSeconds": 1.15, "video": True, "audio": False},
    )
    blue_clip = step(
        "edit_overwrite",
        {"item": blue_id, "track": "V1", "seconds": 3.4, "durationSeconds": 1.2, "video": True, "audio": False},
    )
    tone = step(
        "edit_overwrite",
        {"item": bars_id, "track": "A1", "seconds": 0, "durationSeconds": 4.5, "video": False, "audio": True},
    )
    bars_v = bars_clip["clips"][0]
    red_v = red_clip["clips"][0]
    amber_v = amber_clip["clips"][0]
    blue_v = blue_clip["clips"][0]
    audio = tone["clips"][0]

    step("color_grade", {"clips": [red_v], "exposure": 0.25, "contrast": 16, "saturation": 150, "temperature": 8})
    step("effect_apply", {"effect": "gaussian_blur", "clips": [amber_v]})
    step("effect_set_param", {"clip": amber_v, "effect": "gaussian_blur", "param": "blurriness", "value": 8})
    step("edit_roll", {"left": red_v, "right": amber_v, "deltaFrames": 3})
    step("edit_slide", {"clip": amber_v, "deltaFrames": 2})
    step("edit_split", {"clip": bars_v, "seconds": 0.5})
    step("track_set", {"track": "A1", "syncLock": False})
    step("edit_trim", {"clip": blue_v, "edge": "out", "mode": "ripple", "deltaFrames": -4})
    step("edit_slip", {"clip": audio, "deltaFrames": 4})
    step("transition_video", {"clip": blue_v, "effect": "cross_dissolve", "frames": 8, "edge": "in"})

    title = step(
        "title_add",
        {
            "text": "FilmCraft MCP",
            "seconds": 2.1,
            "time": ticks(1.15),
            "position": [48, 200],
            "size": 64,
            "font": "Noto Serif",
        },
    )
    step(
        "command_run",
        {
            "id": "graphics.set",
            "params": {
                "clip": title["clip"],
                "props": {
                    "fillColor": "#ffffff",
                    "fontStyle": "Bold",
                    "background": True,
                    "backgroundColor": "#1a1a1a",
                },
            },
        },
    )
    step(
        "shape_add",
        {
            "shape": "rectangle",
            "position": [320, 300],
            "size": [520, 8],
            "seconds": 2.1,
            "time": ticks(1.15),
        },
    )
    step("marker_add", {"seconds": 1.15, "name": "Title", "comment": "graded red under the title", "color": "Rose"})
    step("mark_in", {"seconds": 0})
    step("mark_out", {"seconds": 4.4})
    step("audio_gain", {"clips": [audio], "mode": "set", "db": -8})
    step("audio_fade", {"clip": audio, "edge": "out", "frames": 12, "effect": "exponential_fade"})
    step("playhead_set", {"seconds": 1.7})

    for label, seconds in (("early", 0.35), ("title", 1.7), ("late", 4.0)):
        step("capture_frame", {"seconds": seconds, "path": str(out / f"{label}.png"), "maxSide": 640})
    movie = out / "demo.mp4"
    step(
        "export_media",
        {
            "path": str(movie),
            "format": "h264",
            "width": 640,
            "height": 360,
            "audio": True,
            "range": "custom",
            "startSeconds": 0,
            "endSeconds": 4.4,
            "wait": True,
        },
    )
    state = step("state_query", {})
    (out / "state.json").write_text(json.dumps(state, indent=2)[:200000])
    print(f"wrote {movie}")


if __name__ == "__main__":
    main()
