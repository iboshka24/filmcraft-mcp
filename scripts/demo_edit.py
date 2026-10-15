#!/usr/bin/env python3
"""Build the README film by calling the FilmCraft MCP HTTP tools.

    cargo run -p filmcraft-mcp -- http --bind 127.0.0.1:8765
    python3 scripts/demo_edit.py docs/demo

Twelve seconds, 1920x1080, 30 fps. Procedural scenes (ocean, aurora, city, dunes)
with a push, pan, or drift on every shot, dissolves and light leaks, a screen-blended
plasma leak, gradient shapes, a title and subtitle with eased position/scale/opacity
keyframes, and a two-layer musical pad with fades. Also writes a GIF preview.
"""

import json
import sys
import urllib.request
from pathlib import Path

URL = "http://127.0.0.1:8765/mcp"
END = 12.0


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
    with urllib.request.urlopen(request, timeout=600) as response:
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
        print(f"{n:02d} {name}: {json.dumps(value)[:220]}")
        n += 1
        return value

    def clip_of(placed):
        return placed["clips"][0]

    def kf(clip, param, value, seconds, ease, effect="motion"):
        step(
            "keyframe_set",
            {
                "clip": clip,
                "effect": effect,
                "param": param,
                "value": value,
                "seconds": seconds,
                "ease": ease,
            },
        )

    def motion(clip, keys):
        for param, frames in keys:
            for seconds, value, ease in frames:
                kf(clip, param, value, seconds, ease)

    step("sequence_new", {"name": "Nightfall", "width": 1920, "height": 1080, "fps": 30, "video": 3, "audio": 2})

    def scene(name):
        got = step("media_import_demo", {"scene": name})
        return got["items"][0]

    ocean, aurora, city, dunes, plasma = (scene(s) for s in ("OceanSunset", "Aurora", "CityNight", "Dunes", "Plasma"))

    shots = [
        ("ocean", ocean, 0.0, 3.6, 0.4),
        ("aurora", aurora, 3.6, 3.2, 0.8),
        ("city", city, 6.8, 2.8, 0.5),
        ("dunes", dunes, 9.6, 2.4, 1.0),
    ]
    placed = {}
    for name, item, start, dur, src in shots:
        placed[name] = clip_of(
            step(
                "edit_overwrite",
                {
                    "item": item,
                    "track": "V1",
                    "seconds": start,
                    "durationSeconds": dur,
                    "sourceInSeconds": src,
                    "video": True,
                    "audio": False,
                },
            )
        )

    leak = clip_of(
        step(
            "edit_overwrite",
            {
                "item": plasma,
                "track": "V2",
                "seconds": 0,
                "durationSeconds": END,
                "sourceInSeconds": 0.2,
                "video": True,
                "audio": False,
            },
        )
    )
    bed = clip_of(
        step(
            "edit_overwrite",
            {"item": city, "track": "A1", "seconds": 0, "durationSeconds": END, "sourceInSeconds": 0.2, "video": False, "audio": True},
        )
    )
    pad = clip_of(
        step(
            "edit_overwrite",
            {"item": aurora, "track": "A2", "seconds": 0, "durationSeconds": END, "sourceInSeconds": 0.4, "video": False, "audio": True},
        )
    )

    step("transition_video", {"clip": placed["aurora"], "effect": "film_dissolve", "frames": 18, "edge": "in"})
    step("transition_video", {"clip": placed["city"], "effect": "light_leak", "frames": 20, "edge": "in"})
    step("transition_video", {"clip": placed["dunes"], "effect": "cross_zoom", "frames": 16, "edge": "in"})

    grades = {
        "ocean": {"temperature": 16, "contrast": 14, "saturation": 118, "highlights": -6, "shadows": 8},
        "aurora": {"temperature": -12, "tint": 8, "contrast": 18, "saturation": 132, "shadows": -6},
        "city": {"temperature": -18, "contrast": 22, "saturation": 108, "shadows": -12, "blacks": -8},
        "dunes": {"temperature": 24, "contrast": 16, "saturation": 122, "highlights": -10, "vibrance": 20},
    }
    for name, grade in grades.items():
        step("color_grade", {"clips": [placed[name]], **grade})
        step("effect_set_param", {"clip": placed[name], "effect": "lumetri", "param": "vignette_amount", "value": -1.35})
        step("effect_set_param", {"clip": placed[name], "effect": "lumetri", "param": "vignette_feather", "value": 62})
        step("effect_set_param", {"clip": placed[name], "effect": "lumetri", "param": "vignette_midpoint", "value": 42})

    motion(
        placed["ocean"],
        [
            ("scale", [(0.0, 106, "easeOut"), (3.55, 122, "easeIn")]),
            ("position", [(0.0, [930, 570], "easeOut"), (3.55, [990, 490], "easeIn")]),
            ("rotation", [(0.0, -0.6, "easeOut"), (3.55, 0.8, "easeIn")]),
        ],
    )
    # opacity lives on its own effect
    kf(placed["ocean"], "opacity", 0, 0.0, "easeOut", "opacity")
    kf(placed["ocean"], "opacity", 100, 0.55, "easeIn", "opacity")
    motion(
        placed["aurora"],
        [
            ("scale", [(3.6, 126, "easeOut"), (6.75, 108, "easeIn")]),
            ("position", [(3.6, [1040, 500], "easeOut"), (6.75, [880, 570], "easeIn")]),
            ("rotation", [(3.6, 1.1, "easeOut"), (6.75, -0.7, "easeIn")]),
        ],
    )
    motion(
        placed["city"],
        [
            ("scale", [(6.8, 136, "easeOut"), (9.55, 112, "easeIn")]),
            ("position", [(6.8, [960, 620], "easeOut"), (9.55, [960, 490], "easeIn")]),
            ("rotation", [(6.8, -0.4, "easeOut"), (9.55, 0.5, "easeIn")]),
        ],
    )
    motion(
        placed["dunes"],
        [
            ("scale", [(9.6, 114, "easeOut"), (12.0, 128, "easeIn")]),
            ("position", [(9.6, [860, 540], "easeOut"), (12.0, [1060, 500], "easeIn")]),
            ("rotation", [(9.6, 1.4, "easeOut"), (12.0, -1.1, "easeIn")]),
        ],
    )
    kf(placed["dunes"], "opacity", 100, 10.8, "easeOut", "opacity")
    kf(placed["dunes"], "opacity", 0, 12.0, "easeIn", "opacity")

    step("effect_set_param", {"clip": leak, "effect": "opacity", "param": "blend", "value": "Screen"})
    motion(
        leak,
        [
            ("scale", [(0.0, 156, "easeOut"), (6.0, 138, "linear"), (12.0, 148, "easeIn")]),
            ("position", [(0.0, [760, 360], "easeOut"), (6.0, [1100, 640], "linear"), (12.0, [860, 420], "easeIn")]),
            ("rotation", [(0.0, -2.0, "easeOut"), (12.0, 3.0, "easeIn")]),
        ],
    )
    for seconds, value, ease in ((0.0, 0, "easeOut"), (1.4, 20, "easeIn"), (4.2, 8, "easeOut"), (7.6, 26, "easeIn"), (10.4, 10, "easeOut"), (12.0, 0, "easeIn")):
        kf(leak, "opacity", value, seconds, ease, "opacity")

    floor = step(
        "shape_add",
        {"shape": "rectangle", "position": [960, 980], "size": [1920, 460], "seconds": END, "time": 0},
    )["clip"]
    step(
        "command_run",
        {
            "id": "graphics.set",
            "params": {
                "clip": floor,
                "props": {
                    "fill": True,
                    "fillKind": "Linear Gradient",
                    "gradientStart": "#00000000",
                    "gradientEnd": "#07080fcc",
                    "gradientAngle": 90,
                },
            },
        },
    )
    kf(floor, "opacity", 0, 0.0, "easeOut", "graphic_shape")
    kf(floor, "opacity", 100, 1.0, "easeIn", "graphic_shape")

    orb = step(
        "shape_add",
        {"shape": "ellipse", "position": [1500, 240], "size": [780, 460], "seconds": END, "time": 0},
    )["clip"]
    step(
        "command_run",
        {
            "id": "graphics.set",
            "params": {
                "clip": orb,
                "props": {
                    "fill": True,
                    "fillKind": "Linear Gradient",
                    "gradientStart": "#ffc27a99",
                    "gradientEnd": "#ff9a3c00",
                    "gradientAngle": 40,
                },
            },
        },
    )
    step("effect_set_param", {"clip": orb, "effect": "opacity", "param": "blend", "value": "Screen"})
    kf(orb, "position", [1540, 220], 0.2, "easeOut", "graphic_shape")
    kf(orb, "position", [420, 760], 11.4, "easeIn", "graphic_shape")
    kf(orb, "scale", 80, 0.2, "easeOut", "graphic_shape")
    kf(orb, "scale", 130, 6.0, "easeIn", "graphic_shape")
    kf(orb, "scale", 90, 11.6, "easeOut", "graphic_shape")
    kf(orb, "opacity", 0, 0.0, "easeOut", "graphic_shape")
    kf(orb, "opacity", 70, 2.2, "easeIn", "graphic_shape")
    kf(orb, "opacity", 28, 7.0, "easeOut", "graphic_shape")
    kf(orb, "opacity", 0, 11.8, "easeIn", "graphic_shape")

    rule = step(
        "shape_add",
        {"shape": "rectangle", "position": [960, 650], "size": [520, 3], "seconds": 9.0, "time": int(1.05 * 254_016_000_000)},
    )["clip"]
    step(
        "command_run",
        {
            "id": "graphics.set",
            "params": {"clip": rule, "props": {"fill": True, "fillColor": "#f3d7a8"}},
        },
    )
    kf(rule, "scale", 8, 1.05, "easeOut", "graphic_shape")
    kf(rule, "scale", 100, 1.9, "easeOut", "graphic_shape")
    kf(rule, "opacity", 0, 1.05, "easeOut", "graphic_shape")
    kf(rule, "opacity", 100, 1.7, "easeIn", "graphic_shape")
    kf(rule, "opacity", 0, 9.4, "easeIn", "graphic_shape")

    title = step(
        "title_add",
        {
            "text": "NIGHTFALL",
            "seconds": 10.2,
            "time": int(0.35 * 254_016_000_000),
            "position": [960, 560],
            "size": 132,
            "font": "Noto Serif",
        },
    )["clip"]
    step(
        "command_run",
        {
            "id": "graphics.set",
            "params": {
                "clip": title,
                "props": {
                    "align": "Center",
                    "fillColor": "#f6f0e6",
                    "tracking": 180,
                    "shadow": True,
                    "shadow_blur": 36,
                    "shadow_opacity": 62,
                    "shadow_distance": 10,
                    "shadow_angle": 120,
                },
            },
        },
    )
    kf(title, "position", [960, 760], 0.35, "easeOut", "graphic_text")
    kf(title, "position", [960, 540], 1.7, "easeOut", "graphic_text")
    kf(title, "position", [960, 500], 9.6, "easeIn", "graphic_text")
    kf(title, "scale", 86, 0.35, "easeOut", "graphic_text")
    kf(title, "scale", 100, 1.7, "easeOut", "graphic_text")
    kf(title, "scale", 106, 9.6, "easeIn", "graphic_text")
    kf(title, "opacity", 0, 0.35, "easeOut", "graphic_text")
    kf(title, "opacity", 100, 1.25, "easeOut", "graphic_text")
    kf(title, "opacity", 100, 8.8, "linear", "graphic_text")
    kf(title, "opacity", 0, 9.7, "easeIn", "graphic_text")

    sub = step(
        "title_add",
        {
            "text": "a study in light",
            "seconds": 8.2,
            "time": int(1.15 * 254_016_000_000),
            "position": [960, 690],
            "size": 36,
            "font": "Noto Serif",
        },
    )["clip"]
    step(
        "command_run",
        {
            "id": "graphics.set",
            "params": {
                "clip": sub,
                "props": {"align": "Center", "fillColor": "#efe4d2", "tracking": 220, "shadow": True, "shadow_blur": 18, "shadow_opacity": 50},
            },
        },
    )
    kf(sub, "opacity", 0, 1.15, "easeOut", "graphic_text")
    kf(sub, "opacity", 100, 2.15, "easeOut", "graphic_text")
    kf(sub, "opacity", 100, 8.4, "linear", "graphic_text")
    kf(sub, "opacity", 0, 9.3, "easeIn", "graphic_text")
    kf(sub, "position", [960, 740], 1.15, "easeOut", "graphic_text")
    kf(sub, "position", [960, 680], 2.15, "easeOut", "graphic_text")

    chapter = step(
        "title_add",
        {
            "text": "DUNES",
            "seconds": 2.15,
            "time": int(9.85 * 254_016_000_000),
            "position": [180, 860],
            "size": 48,
            "font": "Noto Serif",
        },
    )["clip"]
    step(
        "command_run",
        {
            "id": "graphics.set",
            "params": {"clip": chapter, "props": {"fillColor": "#f4e7d0", "tracking": 280, "shadow": True, "shadow_blur": 16}},
        },
    )
    kf(chapter, "opacity", 0, 9.85, "easeOut", "graphic_text")
    kf(chapter, "opacity", 100, 10.35, "easeOut", "graphic_text")
    kf(chapter, "opacity", 0, 11.7, "easeIn", "graphic_text")
    kf(chapter, "position", [120, 900], 9.85, "easeOut", "graphic_text")
    kf(chapter, "position", [180, 860], 10.45, "easeOut", "graphic_text")

    for seconds, name in ((0.0, "Open"), (3.6, "Aurora"), (6.8, "City"), (9.6, "Dunes")):
        step("marker_add", {"seconds": seconds, "name": name, "color": "Iris"})
    step("audio_gain", {"clips": [bed], "mode": "set", "db": -10})
    step("audio_gain", {"clips": [pad], "mode": "set", "db": -16})
    step("audio_fade", {"clip": bed, "edge": "in", "frames": 20, "effect": "exponential_fade"})
    step("audio_fade", {"clip": bed, "edge": "out", "frames": 28, "effect": "exponential_fade"})
    step("audio_fade", {"clip": pad, "edge": "in", "frames": 24, "effect": "constant_power"})
    step("audio_fade", {"clip": pad, "edge": "out", "frames": 30, "effect": "exponential_fade"})
    step("playhead_set", {"seconds": 1.8})

    for label, seconds in (("open", 1.8), ("mid", 5.1), ("close", 10.5)):
        step("capture_frame", {"seconds": seconds, "path": str(out / f"{label}.png"), "maxSide": 1920})

    movie = out / "demo.mp4"
    step(
        "export_media",
        {
            "path": str(movie),
            "format": "h264",
            "width": 1920,
            "height": 1080,
            "audio": True,
            "range": "custom",
            "startSeconds": 0,
            "endSeconds": END,
            "wait": True,
        },
    )
    gif = out / "demo.gif"
    step(
        "export_media",
        {
            "path": str(gif),
            "format": "gif",
            "width": 640,
            "height": 360,
            "fps": 10,
            "audio": False,
            "range": "custom",
            "startSeconds": 0,
            "endSeconds": END,
            "wait": True,
        },
    )
    state = step("state_query", {})
    (out / "state.json").write_text(json.dumps(state, indent=2)[:400000])
    print(f"wrote {movie} and {gif}")


if __name__ == "__main__":
    main()
