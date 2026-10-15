# FilmCraft MCP server

`filmcraft-mcp` is a Model Context Protocol server (protocol `2025-06-18`) for the FilmCraft engine. It is part of this fork of [storytold/filmcraft](https://github.com/storytold/filmcraft) at commit `8309f21`. It is not an official FilmCraft release.

The process holds one `filmcraft_engine::Session`. Every named tool calls `Session::execute`, the program renderer, or the demuxer. Unknown arguments are rejected before the engine sees them. A tool error comes back as a JSON-RPC result with `isError: true` and an `error` string, not a crash.

Client setup (Cursor, Claude Desktop, Claude Code, Codex, OpenCode) is at the top of the README.

## Transports

**Stdio.** One JSON object per line. `Content-Length` framing is also accepted. This is the transport desktop clients should spawn:

```sh
cargo build -p filmcraft-mcp
./target/debug/filmcraft-mcp stdio
./target/debug/filmcraft-mcp --project cut.fcproj stdio
./target/debug/filmcraft-mcp --demo stdio
```

**HTTP.** Loopback only. `POST /mcp` is one JSON-RPC request. `GET /health` returns `{"ok":true,"tools":N}`.

```sh
cargo run -p filmcraft-mcp -- http --bind 127.0.0.1:8765
```

- The bind address must be loopback. Anything else is refused.
- `Host` must be `127.0.0.1`, `localhost`, or `::1`.
- A browser `Origin` that is not loopback HTTP is refused.
- If `FILMCRAFT_MCP_TOKEN` is set, send `Authorization: Bearer <token>`.
- Body limit: 1 MiB.
- Notifications (requests without `id`) get HTTP 202 and an empty object.

## Methods and resources

| Method | |
|---|---|
| `initialize` | Protocol `2025-06-18`. Capabilities: tools and resources. |
| `tools/list` | JSON Schema for every tool, including `readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint`. |
| `tools/call` | `{name, arguments}`. The text content is JSON. `capture_frame` also returns an image block. |
| `resources/list` | `filmcraft://document`, `filmcraft://tools`. |
| `resources/read` | The document is `state_query`. The tools resource is this catalog. |
| `ping` | Empty result. |

## Calling conventions

- Item and clip ids are integers returned by earlier tools (`item`, `clips`, `clip`, `sequence`).
- Tracks are names such as `V1` and `A1`. `track: "new"` on a place adds a track.
- `durationSeconds` on insert/overwrite is the timeline length. `sourceInSeconds` is the source in-point. The engine also accepts tick fields (`duration`, `sourceIn`, `time`).
- `edit_trim` `edge` is `in` or `out`. `mode` is the engine trim mode (`ripple`, `roll`, `trim`, …). A ripple that would move a sync-locked track with a clip in the way fails until that track's sync lock is off (`track_set`).
- `export_media` `format` is an engine format id (`h264`, `prores`, `wav`, `png`, `gif`, `aac`, …). `range` is `entire`, `inOut`, `workArea`, or `custom` (then `startSeconds` / `endSeconds`). `wait` defaults to true.
- `color_grade` applies Lumetri Color and sets the basic-correction parameters you pass.
- `command_run` is the escape hatch: `{id, params}` is `Session::execute`. `command_list` shows ids, labels, and whether each command is enabled.
- `undo` / `redo` walk the same history the desktop app uses.

## Limits

- One project per process. Calls are serialized on that session.
- HTTP is not a remote service and not a full streamable-HTTP session with server-sent events.
- UI tools (click, drag, screenshot of the window) stay on upstream `filmcraft-cli mcp --bridge`. This server renders frames headlessly.
- Synthetic generators (`generate_color_matte`, bars and tone, black, leader, transparent) create project items. Picture from a file goes through `media_import`, then `edit_insert` / `edit_overwrite`.
- `media_probe` reads the file with `filmcraft_codecs` and does not import it.

Regenerate the tables below with `filmcraft-mcp tools`.

# FilmCraft MCP tools

70 named tools. Each row is wired to the FilmCraft engine (a command id, the program renderer, or the demuxer). `command_run` still reaches every other engine command.

| Tool | Calls | Read-only | Arguments |
|---|---|---|---|
| `project_new` | file.newProject | no | `name?` |
| `project_open` | file.open | no | `path` |
| `project_save` | file.save | no | `path?` |
| `project_save_as` | file.saveAs | no | `path` |
| `project_inspect` | project.inspect | yes | — |
| `bin_new` | file.newBin | no | `name?`, `parent?` |
| `media_import` | file.import | no | `paths`, `bin?`, `imageSequence?`, `replace?` |
| `media_probe` | filmcraft_codecs::open_bytes | yes | `path` |
| `media_status` | media.status | yes | — |
| `media_search` | media.search | yes | `query?` |
| `generate_color_matte` | file.newColorMatte | no | `color?`, `seconds?`, `name?`, `width?`, `height?` |
| `generate_bars_and_tone` | file.newBarsAndTone | no | `seconds?`, `name?`, `width?`, `height?` |
| `generate_black_video` | file.newBlackVideo | no | `seconds?`, `name?` |
| `generate_counting_leader` | file.newCountingLeader | no | `seconds?`, `name?` |
| `generate_transparent` | file.newTransparentVideo | no | `seconds?`, `name?` |
| `sequence_new` | file.newSequence | no | `name?`, `width?`, `height?`, `fps?`, `sampleRate?`, `video?`, `audio?`, `fromItem?`, `fromItems?` |
| `sequence_open` | sequence.open | no | `item` |
| `sequence_inspect` | sequence.inspect | yes | — |
| `sequence_settings` | sequence.settings | no | `name?`, `width?`, `height?`, `fps?`, `sampleRate?`, `mix?`, `dropFrame?`, `workingSpace?` |
| `sequence_close` | sequence.close | no | `item?` |
| `tracks_add` | sequence.addTracks | no | `video?`, `audio?`, `submix?`, `audioType?` |
| `track_delete` | sequence.deleteTrack | no | `track` |
| `track_set` | timeline.setTrack | no | `track`, `locked?`, `syncLock?`, `enabled?`, `muted?`, `solo?`, `name?`, `volumeDb?`, `pan?` |
| `track_target` | timeline.setTargeting | no | `track`, `targeted?`, `sourcePatch?` |
| `edit_insert` | timeline.place (insert) | no | `item?`, `items?`, `track?`, `audioTrack?`, `seconds?`, `frame?`, `time?`, `timecode?`, `durationSeconds?`, `duration?`, `sourceInSeconds?`, `sourceIn?`, `video?`, `audio?` |
| `edit_overwrite` | timeline.place (overwrite) | no | `item?`, `items?`, `track?`, `audioTrack?`, `seconds?`, `frame?`, `time?`, `timecode?`, `durationSeconds?`, `duration?`, `sourceInSeconds?`, `sourceIn?`, `video?`, `audio?` |
| `edit_ripple_delete` | edit.rippleDelete | no | `clips?` |
| `edit_delete` | edit.clear | no | `clips?` |
| `edit_lift` | sequence.lift | no | `seconds?`, `frame?`, `time?`, `endSeconds?`, `endFrame?` |
| `edit_extract` | sequence.extract | no | `seconds?`, `frame?`, `time?`, `endSeconds?`, `endFrame?` |
| `edit_roll` | timeline.roll | no | `left`, `right`, `deltaFrames?`, `delta?` |
| `edit_slip` | timeline.slip | no | `clip`, `deltaFrames?`, `delta?` |
| `edit_slide` | timeline.slide | no | `clip`, `deltaFrames?`, `delta?` |
| `edit_split` | timeline.razor | no | `seconds?`, `frame?`, `time?`, `timecode?`, `track?`, `clip?` |
| `edit_split_all` | sequence.addEditAllTracks | no | `seconds?`, `frame?`, `time?` |
| `edit_trim` | timeline.trim | no | `clip`, `edge`, `mode?`, `deltaFrames?`, `delta?` |
| `edit_move` | timeline.move | no | `moves`, `insert?`, `linked?`, `copy?` |
| `edit_rate_stretch` | timeline.rateStretch | no | `clip`, `edge`, `delta` |
| `edit_select` | timeline.select | no | `clips`, `add?`, `toggle?`, `linked?` |
| `transition_video` | sequence.applyVideoTransition | no | `clip?`, `effect?`, `frames?`, `edge?`, `params?`, `reverse?` |
| `transition_audio` | sequence.applyAudioTransition | no | `clip?`, `effect?`, `frames?`, `edge?`, `params?`, `reverse?` |
| `transition_set` | sequence.setTransition | no | `transition`, `frames?`, `duration?`, `align?`, `params?`, `reverse?` |
| `transition_clear` | sequence.removeTransition | no | `transitions?` |
| `title_add` | graphics.newText | no | `text`, `seconds?`, `time?`, `position?`, `size?`, `font?`, `clip?`, `newClip?` |
| `shape_add` | graphics.newRectangle | no | `shape?`, `position?`, `size?`, `clip?`, `seconds?`, `time?` |
| `effect_apply` | effects.apply | no | `effect`, `clips?` |
| `effect_remove` | effects.remove | no | `clip`, `index` |
| `effect_set_param` | effects.setParam | no | `clip`, `effect`, `param`, `value` |
| `effect_list` | filmcraft_project::effect_defs | yes | `query?`, `kind?` |
| `color_grade` | effects.apply lumetri + effects.setParam | no | `clip?`, `clips?`, `exposure?`, `contrast?`, `saturation?`, `temperature?`, `tint?`, `highlights?`, `shadows?`, `whites?`, `blacks?`, `vibrance?` |
| `audio_gain` | clip.audioGain | no | `clips?`, `mode?`, `db?` |
| `audio_fade` | sequence.applyAudioTransition | no | `clip?`, `edge?`, `frames?`, `effect?` |
| `audio_peak` | clip.audioPeak | yes | `clips?` |
| `marker_add` | markers.add | no | `seconds?`, `frame?`, `time?`, `name?`, `comment?`, `color?` |
| `marker_range` | markers.addRange | no | `seconds?`, `frame?`, `durationFrames?`, `name?`, `comment?`, `color?` |
| `marker_clear` | markers.clearAll | no | `all?`, `target?` |
| `mark_in` | markers.markIn | no | `seconds?`, `frame?`, `time?`, `timecode?`, `target?` |
| `mark_out` | markers.markOut | no | `seconds?`, `frame?`, `time?`, `timecode?`, `target?` |
| `mark_clear` | markers.clearInOut | no | `target?` |
| `undo` | edit.undo | no | — |
| `redo` | edit.redo | no | — |
| `playhead_set` | playhead.set | no | `seconds?`, `frame?`, `time?`, `timecode?`, `target?` |
| `capture_frame` | Session::try_render_program_at | yes | `seconds?`, `path?`, `maxSide?` |
| `export_frame` | file.exportFrame | no | `path`, `target?`, `format?` |
| `export_media` | file.exportMedia | no | `path`, `format?`, `preset?`, `width?`, `height?`, `audio?`, `range?`, `startSeconds?`, `endSeconds?`, `wait?` |
| `close_gap` | sequence.closeGap | no | `track`, `seconds?`, `frame?`, `time?` |
| `jobs_list` | jobs.list | yes | — |
| `state_query` | project.inspect + sequence.inspect | yes | — |
| `command_list` | command.list | yes | `filter?`, `enabledOnly?` |
| `command_run` | Session::execute | no | `id`, `params?` |

`?` marks an optional argument. Time arguments accept `seconds`, `frame`, `time` (ticks; 254016000000 = 1 second) or `timecode` when the engine command does. On `title_add` and `shape_add`, `seconds` is the clip duration and `time` is the start in ticks (the playhead, if omitted).

## What each tool does

- **`project_new`** — Replace the session with an empty project. Optional name.
- **`project_open`** — Open a .fcproj file from disk.
- **`project_save`** — Save the project. Pass path when it has never been saved.
- **`project_save_as`** — Write the project to a new path.
- **`project_inspect`** — Bins, items (ids, types, durations) and the active sequence id.
- **`bin_new`** — Create a project bin.
- **`media_import`** — Import media files into the project (and optionally a bin).
- **`media_probe`** — Demux a file with FilmCraft's codecs and return stream info. Does not modify the project.
- **`media_status`** — Online / offline / missing status of project media.
- **`media_search`** — Search project items by name.
- **`generate_color_matte`** — Create a color-matte generator item (solid frames the engine can place and render).
- **`generate_bars_and_tone`** — Create bars-and-tone (picture plus a tone). Useful as synthetic picture and sound.
- **`generate_black_video`** — Create a black-video generator item.
- **`generate_counting_leader`** — Create a universal counting-leader generator.
- **`generate_transparent`** — Create a transparent-video generator.
- **`sequence_new`** — Create a sequence and open it on the timeline. Optional fromItem / fromItems edit the clip in.
- **`sequence_open`** — Open a sequence item in the timeline.
- **`sequence_inspect`** — Active sequence: tracks, clips, transitions, markers, in/out, playhead, selection. Times are ticks and frames.
- **`sequence_settings`** — Change frame size, rate, name, sample rate, mix, or working color space.
- **`sequence_close`** — Close the active sequence tab, or the sequence named by item.
- **`tracks_add`** — Add video, audio, or submix tracks.
- **`track_delete`** — Delete one track by name (V3, A2) or id.
- **`track_set`** — Lock, mute, solo, enable, rename, or set a track fader.
- **`track_target`** — Set timeline track targeting and source patching.
- **`edit_insert`** — Ripple-insert a project item at a timeline time (timeline.place with insert=true).
- **`edit_overwrite`** — Overwrite a project item onto the timeline (timeline.place with insert=false).
- **`edit_ripple_delete`** — Delete clips and close the gap. Pass clips, or use the current selection.
- **`edit_delete`** — Lift clips off the timeline without closing the gap (Clear).
- **`edit_lift`** — Lift the marked range (leave a gap). Set the range with seconds/endSeconds or use existing In/Out.
- **`edit_extract`** — Extract the marked range and ripple the tail. Set the range with seconds/endSeconds or use existing In/Out.
- **`edit_roll`** — Rolling edit between two adjacent clips: the cut moves, the combined duration stays.
- **`edit_slip`** — Slip a clip's source in/out without moving it on the timeline. The source must be longer than the clip.
- **`edit_slide`** — Slide a clip in time; its neighbors trim to keep the gap closed.
- **`edit_split`** — Razor clips at a time. Optional track or a single clip (linked partners follow).
- **`edit_split_all`** — Add an edit (through-edit) on every track at a time.
- **`edit_trim`** — Trim a clip's in or out point. mode is regular or ripple.
- **`edit_move`** — Move or copy clips. Each move is {clip, track, time|seconds|frame}. insert=true ripples them in.
- **`edit_rate_stretch`** — Stretch a clip's speed by dragging an edge.
- **`edit_select`** — Replace or extend the timeline selection.
- **`transition_video`** — Apply a video transition (default Cross Dissolve) on a cut.
- **`transition_audio`** — Apply an audio transition (constant power, constant gain, exponential fade, …).
- **`transition_set`** — Change a transition's duration, alignment, or parameters.
- **`transition_clear`** — Remove transitions by id, or the selected transition.
- **`title_add`** — Create a text graphic (graphics.newText) on the timeline.
- **`shape_add`** — Create a rectangle or ellipse graphic.
- **`effect_apply`** — Apply an effect by id or display name (gaussian_blur, Lumetri Color, …). See effect_list.
- **`effect_remove`** — Remove one non-intrinsic effect by index on a clip.
- **`effect_set_param`** — Set one effect parameter. Pass effect as an id or index.
- **`effect_list`** — Built-in video, audio, and transition effects with parameter ids.
- **`color_grade`** — Apply Lumetri Color and set basic correction (exposure, contrast, saturation, temperature, tint, highlights, shadows, whites, blacks, vibrance).
- **`audio_gain`** — Set, adjust, or normalize clip gain in dB (clip.audioGain).
- **`audio_fade`** — Fade a clip's audio edge with an audio transition (default exponential fade).
- **`audio_peak`** — Peak amplitude of the selected audio clips, with current gain.
- **`marker_add`** — Add a sequence marker at a time.
- **`marker_range`** — Add a marker that spans a duration.
- **`marker_clear`** — Clear sequence markers. all=true clears every marker; otherwise the marker at the playhead.
- **`mark_in`** — Set the sequence In point.
- **`mark_out`** — Set the sequence Out point.
- **`mark_clear`** — Clear In and Out.
- **`undo`** — Undo the last edit.
- **`redo`** — Redo the last undone edit.
- **`playhead_set`** — Move the playhead.
- **`capture_frame`** — Render one program frame to PNG. Returns an image and, if path is set, writes the file. Does not move the playhead.
- **`export_frame`** — Write the program (or source) frame through file.exportFrame.
- **`export_media`** — Render the sequence (file.exportMedia). Waits until the file is written unless wait is false.
- **`close_gap`** — Ripple-delete the gap on a track at a time.
- **`jobs_list`** — Background jobs (exports) and their progress.
- **`state_query`** — Project tree plus the active sequence, in one call.
- **`command_list`** — Every engine command id, label, menu, shortcut, parameter doc, and whether it is enabled.
- **`command_run`** — Run any engine command by id. params is passed through unchanged. Edits are undoable.
