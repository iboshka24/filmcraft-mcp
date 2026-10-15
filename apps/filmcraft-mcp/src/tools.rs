//! Named tools. Each one is a real engine command, a renderer call, or a media probe.

use std::sync::OnceLock;

use filmcraft_engine::Session;
use filmcraft_time::Tick;
use serde_json::{Map, Value, json};

use crate::Server;
use crate::protocol::tool_result_text;

pub type RunResult = Result<Value, String>;

#[derive(Clone, Copy)]
pub struct Arg {
    pub name: &'static str,
    pub json_type: &'static str,
    pub description: &'static str,
    pub required: bool,
}

#[derive(Clone, Copy)]
pub enum Op {
    /// `Session::execute` with the arguments object.
    Cmd(&'static str),
    /// `timeline.place` with `insert` forced.
    Place(bool),
    Move,
    /// Lift (`false`) or extract (`true`). Optional in/out from the arguments.
    Lift(bool),
    Color,
    AudioFade,
    Probe,
    Capture,
    State,
    Effects,
    Commands,
    Run,
    /// `effects.setParam` with `keyframe: true`, then optional interpolation.
    Keyframe,
}

pub struct Tool {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub read_only: bool,
    pub destructive: bool,
    pub idempotent: bool,
    pub op: Op,
    pub args: Vec<Arg>,
}

impl Tool {
    pub fn schema(&self) -> Value {
        let mut properties = Map::new();
        let mut required = Vec::new();
        for arg in &self.args {
            let mut spec = json!({"description": arg.description});
            if arg.json_type != "any" {
                spec["type"] = json!(arg.json_type);
            }
            properties.insert(arg.name.to_string(), spec);
            if arg.required {
                required.push(json!(arg.name));
            }
        }
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": {
                "type": "object",
                "properties": properties,
                "required": required,
                "additionalProperties": false
            },
            "annotations": {
                "readOnlyHint": self.read_only,
                "destructiveHint": self.destructive,
                "idempotentHint": self.idempotent,
                "openWorldHint": false
            }
        })
    }

    pub fn engine(&self) -> &'static str {
        match self.op {
            Op::Cmd(id) => id,
            Op::Place(true) => "timeline.place (insert)",
            Op::Place(false) => "timeline.place (overwrite)",
            Op::Move => "timeline.move",
            Op::Lift(false) => "sequence.lift",
            Op::Lift(true) => "sequence.extract",
            Op::Color => "effects.apply lumetri + effects.setParam",
            Op::AudioFade => "sequence.applyAudioTransition",
            Op::Probe => "filmcraft_codecs::open_bytes",
            Op::Capture => "Session::try_render_program_at",
            Op::State => "project.inspect + sequence.inspect",
            Op::Effects => "filmcraft_project::effect_defs",
            Op::Commands => "command.list",
            Op::Run => "Session::execute",
            Op::Keyframe => "effects.setParam keyframe + effects.setInterpolation",
        }
    }
}

const fn arg(name: &'static str, json_type: &'static str, description: &'static str, required: bool) -> Arg {
    Arg { name, json_type, description, required }
}

pub fn tools() -> &'static [Tool] {
    static TOOLS: OnceLock<Vec<Tool>> = OnceLock::new();
    TOOLS.get_or_init(build)
}

fn build() -> Vec<Tool> {
    fn t(
        name: &'static str,
        title: &'static str,
        description: &'static str,
        read_only: bool,
        destructive: bool,
        idempotent: bool,
        op: Op,
        args: impl IntoIterator<Item = Arg>,
    ) -> Tool {
        Tool { name, title, description, read_only, destructive, idempotent, op, args: args.into_iter().collect() }
    }
    vec![
        t(
            "project_new",
            "New project",
            "Replace the session with an empty project. Optional name.",
            false,
            true,
            false,
            Op::Cmd("file.newProject"),
            [arg("name", "string", "Project name.", false)],
        ),
        t(
            "project_open",
            "Open project",
            "Open a .fcproj file from disk.",
            false,
            true,
            true,
            Op::Cmd("file.open"),
            [arg("path", "string", "Absolute or working-directory path to the project.", true)],
        ),
        t(
            "project_save",
            "Save project",
            "Save the project. Pass path when it has never been saved.",
            false,
            false,
            true,
            Op::Cmd("file.save"),
            [arg("path", "string", "Destination .fcproj. Optional when the project already has a path.", false)],
        ),
        t(
            "project_save_as",
            "Save project as",
            "Write the project to a new path.",
            false,
            false,
            true,
            Op::Cmd("file.saveAs"),
            [arg("path", "string", "Destination .fcproj.", true)],
        ),
        t(
            "project_inspect",
            "Inspect project",
            "Bins, items (ids, types, durations) and the active sequence id.",
            true,
            false,
            true,
            Op::Cmd("project.inspect"),
            std::iter::empty(),
        ),
        t(
            "bin_new",
            "New bin",
            "Create a project bin.",
            false,
            false,
            false,
            Op::Cmd("file.newBin"),
            [arg("name", "string", "Bin name.", false), arg("parent", "integer", "Parent bin id.", false)],
        ),
        t(
            "media_import",
            "Import media",
            "Import media files into the project (and optionally a bin).",
            false,
            false,
            false,
            Op::Cmd("file.import"),
            [
                arg("paths", "array", "File paths.", true),
                arg("bin", "integer", "Destination bin id.", false),
                arg("imageSequence", "boolean", "Treat a numbered still as an image sequence.", false),
                arg("replace", "boolean", "Replace existing footage.", false),
            ],
        ),
        t(
            "media_import_demo",
            "Import demo footage",
            "Import FilmCraft's procedural scenes (moving picture and a musical pad). scene is OceanSunset, Aurora, CityNight, Dunes, Plasma, or Forest. Omit scene to import all six.",
            false,
            false,
            false,
            Op::Cmd("file.importDemoFootage"),
            [arg("scene", "string", "One scene name. Omit to import every scene.", false)],
        ),
        t(
            "media_probe",
            "Probe media",
            "Demux a file with FilmCraft's codecs and return stream info. Does not modify the project.",
            true,
            false,
            true,
            Op::Probe,
            [arg("path", "string", "Media file to probe.", true)],
        ),
        t(
            "media_status",
            "Media status",
            "Online / offline / missing status of project media.",
            true,
            false,
            true,
            Op::Cmd("media.status"),
            std::iter::empty(),
        ),
        t(
            "media_search",
            "Search media",
            "Search project items by name.",
            true,
            false,
            true,
            Op::Cmd("media.search"),
            [arg("query", "string", "Substring to search for.", false)],
        ),
        t(
            "generate_color_matte",
            "Color matte",
            "Create a color-matte generator item (solid frames the engine can place and render).",
            false,
            false,
            false,
            Op::Cmd("file.newColorMatte"),
            [
                arg("color", "string", "Hex color such as #c0392b.", false),
                arg("seconds", "number", "Duration in seconds (default 5).", false),
                arg("name", "string", "Item name.", false),
                arg("width", "integer", "Width in pixels. Defaults to the active sequence.", false),
                arg("height", "integer", "Height in pixels.", false),
            ],
        ),
        t(
            "generate_bars_and_tone",
            "Bars and tone",
            "Create bars-and-tone (picture plus a tone). Useful as synthetic picture and sound.",
            false,
            false,
            false,
            Op::Cmd("file.newBarsAndTone"),
            [
                arg("seconds", "number", "Duration in seconds.", false),
                arg("name", "string", "Item name.", false),
                arg("width", "integer", "Width.", false),
                arg("height", "integer", "Height.", false),
            ],
        ),
        t(
            "generate_black_video",
            "Black video",
            "Create a black-video generator item.",
            false,
            false,
            false,
            Op::Cmd("file.newBlackVideo"),
            [arg("seconds", "number", "Duration in seconds.", false), arg("name", "string", "Item name.", false)],
        ),
        t(
            "generate_counting_leader",
            "Counting leader",
            "Create a universal counting-leader generator.",
            false,
            false,
            false,
            Op::Cmd("file.newCountingLeader"),
            [arg("seconds", "number", "Duration in seconds.", false), arg("name", "string", "Item name.", false)],
        ),
        t(
            "generate_transparent",
            "Transparent video",
            "Create a transparent-video generator.",
            false,
            false,
            false,
            Op::Cmd("file.newTransparentVideo"),
            [arg("seconds", "number", "Duration in seconds.", false), arg("name", "string", "Item name.", false)],
        ),
        t(
            "sequence_new",
            "New sequence",
            "Create a sequence and open it on the timeline. Optional fromItem / fromItems edit the clip in.",
            false,
            false,
            false,
            Op::Cmd("file.newSequence"),
            [
                arg("name", "string", "Sequence name.", false),
                arg("width", "integer", "Frame width. Default 1920.", false),
                arg("height", "integer", "Frame height. Default 1080.", false),
                arg("fps", "number", "Frame rate, for example 24 or 29.97.", false),
                arg("sampleRate", "integer", "Audio sample rate. Default 48000.", false),
                arg("video", "integer", "Video track count. Default 3.", false),
                arg("audio", "integer", "Audio track count. Default 3.", false),
                arg("fromItem", "integer", "Project item to place at the start.", false),
                arg("fromItems", "array", "Items to lay out back to back.", false),
            ],
        ),
        t(
            "sequence_open",
            "Open sequence",
            "Open a sequence item in the timeline.",
            false,
            false,
            true,
            Op::Cmd("sequence.open"),
            [arg("item", "integer", "Sequence item id.", true)],
        ),
        t(
            "sequence_inspect",
            "Inspect sequence",
            "Active sequence: tracks, clips, transitions, markers, in/out, playhead, selection. Times are ticks and frames.",
            true,
            false,
            true,
            Op::Cmd("sequence.inspect"),
            std::iter::empty(),
        ),
        t(
            "sequence_settings",
            "Sequence settings",
            "Change frame size, rate, name, sample rate, mix, or working color space.",
            false,
            false,
            false,
            Op::Cmd("sequence.settings"),
            [
                arg("name", "string", "Sequence name.", false),
                arg("width", "integer", "Width.", false),
                arg("height", "integer", "Height.", false),
                arg("fps", "number", "Frame rate.", false),
                arg("sampleRate", "integer", "Sample rate.", false),
                arg("mix", "string", "Stereo, Mono, 5.1, or Adaptive.", false),
                arg("dropFrame", "boolean", "Drop-frame timecode (29.97 / 59.94 / 119.88 only).", false),
                arg("workingSpace", "string", "rec709, rec2100-pq, or rec2100-hlg.", false),
            ],
        ),
        t(
            "sequence_close",
            "Close sequence",
            "Close the active sequence tab, or the sequence named by item.",
            false,
            false,
            true,
            Op::Cmd("sequence.close"),
            [arg("item", "integer", "Sequence item id. Default: the active sequence.", false)],
        ),
        t(
            "tracks_add",
            "Add tracks",
            "Add video, audio, or submix tracks.",
            false,
            false,
            false,
            Op::Cmd("sequence.addTracks"),
            [
                arg("video", "integer", "How many video tracks to add.", false),
                arg("audio", "integer", "How many audio tracks to add.", false),
                arg("submix", "integer", "How many submix tracks to add.", false),
                arg("audioType", "string", "standard, mono, 5.1, or adaptive.", false),
            ],
        ),
        t(
            "track_delete",
            "Delete track",
            "Delete one track by name (V3, A2) or id.",
            false,
            true,
            false,
            Op::Cmd("sequence.deleteTrack"),
            [arg("track", "string", "Track name such as V3, or a numeric id passed as a string.", true)],
        ),
        t(
            "track_set",
            "Track settings",
            "Lock, mute, solo, enable, rename, or set a track fader.",
            false,
            false,
            false,
            Op::Cmd("timeline.setTrack"),
            [
                arg("track", "string", "Track name or id.", true),
                arg("locked", "boolean", "Lock the track.", false),
                arg("syncLock", "boolean", "Sync lock.", false),
                arg("enabled", "boolean", "Enable the track (video).", false),
                arg("muted", "boolean", "Mute the track.", false),
                arg("solo", "boolean", "Solo the track.", false),
                arg("name", "string", "Display name.", false),
                arg("volumeDb", "number", "Track fader in dB.", false),
                arg("pan", "number", "Track pan.", false),
            ],
        ),
        t(
            "track_target",
            "Target track",
            "Set timeline track targeting and source patching.",
            false,
            false,
            true,
            Op::Cmd("timeline.setTargeting"),
            [
                arg("track", "string", "Track name or id.", true),
                arg("targeted", "boolean", "Whether the track is targeted.", false),
                arg("sourcePatch", "boolean", "Source patch.", false),
            ],
        ),
        t(
            "edit_insert",
            "Insert edit",
            "Ripple-insert a project item at a timeline time (timeline.place with insert=true).",
            false,
            false,
            false,
            Op::Place(true),
            place_args(),
        ),
        t(
            "edit_overwrite",
            "Overwrite edit",
            "Overwrite a project item onto the timeline (timeline.place with insert=false).",
            false,
            true,
            false,
            Op::Place(false),
            place_args(),
        ),
        t(
            "edit_ripple_delete",
            "Ripple delete",
            "Delete clips and close the gap. Pass clips, or use the current selection.",
            false,
            true,
            false,
            Op::Cmd("edit.rippleDelete"),
            [arg("clips", "array", "Clip ids. Default: the selection.", false)],
        ),
        t(
            "edit_delete",
            "Delete clips",
            "Lift clips off the timeline without closing the gap (Clear).",
            false,
            true,
            false,
            Op::Cmd("edit.clear"),
            [arg("clips", "array", "Clip ids. Default: the selection.", false)],
        ),
        t(
            "edit_lift",
            "Lift in/out",
            "Lift the marked range (leave a gap). Set the range with seconds/endSeconds or use existing In/Out.",
            false,
            true,
            false,
            Op::Lift(false),
            range_args(),
        ),
        t(
            "edit_extract",
            "Extract in/out",
            "Extract the marked range and ripple the tail. Set the range with seconds/endSeconds or use existing In/Out.",
            false,
            true,
            false,
            Op::Lift(true),
            range_args(),
        ),
        t(
            "edit_roll",
            "Roll edit",
            "Rolling edit between two adjacent clips: the cut moves, the combined duration stays.",
            false,
            false,
            false,
            Op::Cmd("timeline.roll"),
            [
                arg("left", "integer", "Clip id on the left of the cut.", true),
                arg("right", "integer", "Clip id on the right of the cut.", true),
                arg("deltaFrames", "integer", "Frames to move the cut (positive moves it later).", false),
                arg("delta", "integer", "Same delta in ticks.", false),
            ],
        ),
        t(
            "edit_slip",
            "Slip edit",
            "Slip a clip's source in/out without moving it on the timeline. The source must be longer than the clip.",
            false,
            false,
            false,
            Op::Cmd("timeline.slip"),
            [
                arg("clip", "integer", "Clip id.", true),
                arg("deltaFrames", "integer", "Source frames to slip.", false),
                arg("delta", "integer", "Slip in ticks.", false),
            ],
        ),
        t(
            "edit_slide",
            "Slide edit",
            "Slide a clip in time; its neighbors trim to keep the gap closed.",
            false,
            false,
            false,
            Op::Cmd("timeline.slide"),
            [
                arg("clip", "integer", "Middle clip id.", true),
                arg("deltaFrames", "integer", "Frames to slide.", false),
                arg("delta", "integer", "Slide in ticks.", false),
            ],
        ),
        t(
            "edit_split",
            "Split / razor",
            "Razor clips at a time. Optional track or a single clip (linked partners follow).",
            false,
            false,
            false,
            Op::Cmd("timeline.razor"),
            [
                arg("seconds", "number", "Timeline time in seconds. Default: the playhead.", false),
                arg("frame", "integer", "Timeline frame.", false),
                arg("time", "integer", "Timeline time in ticks.", false),
                arg("timecode", "string", "Timecode.", false),
                arg("track", "string", "Limit the razor to one track (V1, A2, …).", false),
                arg("clip", "integer", "Razor this clip only.", false),
            ],
        ),
        t(
            "edit_split_all",
            "Add edit on all tracks",
            "Add an edit (through-edit) on every track at a time.",
            false,
            false,
            false,
            Op::Cmd("sequence.addEditAllTracks"),
            [
                arg("seconds", "number", "Time in seconds. Default: the playhead.", false),
                arg("frame", "integer", "Frame.", false),
                arg("time", "integer", "Ticks.", false),
            ],
        ),
        t(
            "edit_trim",
            "Trim",
            "Trim a clip's in or out point. mode is regular or ripple.",
            false,
            false,
            false,
            Op::Cmd("timeline.trim"),
            [
                arg("clip", "integer", "Clip id.", true),
                arg("edge", "string", "in or out.", true),
                arg("mode", "string", "regular (default) or ripple.", false),
                arg("deltaFrames", "integer", "Trim delta in frames.", false),
                arg("delta", "integer", "Trim delta in ticks.", false),
            ],
        ),
        t(
            "edit_move",
            "Move clips",
            "Move or copy clips. Each move is {clip, track, time|seconds|frame}. insert=true ripples them in.",
            false,
            false,
            false,
            Op::Move,
            [
                arg("moves", "array", "List of {clip, track, time|seconds|frame}.", true),
                arg("insert", "boolean", "Ripple insert at the destination.", false),
                arg("linked", "boolean", "Move linked partners by the same offset. Default follows Linked Selection.", false),
                arg("copy", "boolean", "Copy instead of moving.", false),
            ],
        ),
        t(
            "edit_rate_stretch",
            "Rate stretch",
            "Stretch a clip's speed by dragging an edge.",
            false,
            false,
            false,
            Op::Cmd("timeline.rateStretch"),
            [arg("clip", "integer", "Clip id.", true), arg("edge", "string", "in or out.", true), arg("delta", "integer", "Edge delta in ticks.", true)],
        ),
        t(
            "edit_select",
            "Select clips",
            "Replace or extend the timeline selection.",
            false,
            false,
            true,
            Op::Cmd("timeline.select"),
            [
                arg("clips", "array", "Clip ids.", true),
                arg("add", "boolean", "Add to the selection instead of replacing it.", false),
                arg("toggle", "boolean", "Toggle each clip.", false),
                arg("linked", "boolean", "Also select linked partners. Default true.", false),
            ],
        ),
        t(
            "transition_video",
            "Video transition",
            "Apply a video transition (default Cross Dissolve) on a cut.",
            false,
            false,
            false,
            Op::Cmd("sequence.applyVideoTransition"),
            transition_args(),
        ),
        t(
            "transition_audio",
            "Audio transition",
            "Apply an audio transition (constant power, constant gain, exponential fade, …).",
            false,
            false,
            false,
            Op::Cmd("sequence.applyAudioTransition"),
            transition_args(),
        ),
        t(
            "transition_set",
            "Edit transition",
            "Change a transition's duration, alignment, or parameters.",
            false,
            false,
            false,
            Op::Cmd("sequence.setTransition"),
            [
                arg("transition", "integer", "Transition id.", true),
                arg("frames", "integer", "Duration in frames.", false),
                arg("duration", "integer", "Duration in ticks.", false),
                arg("align", "string", "center, start, or end.", false),
                arg("params", "object", "Parameter map.", false),
                arg("reverse", "boolean", "Reverse the transition.", false),
            ],
        ),
        t(
            "transition_clear",
            "Remove transitions",
            "Remove transitions by id, or the selected transition.",
            false,
            true,
            false,
            Op::Cmd("sequence.removeTransition"),
            [arg("transitions", "array", "Transition ids.", false)],
        ),
        t(
            "title_add",
            "Add title",
            "Create a text graphic (graphics.newText) on the timeline.",
            false,
            false,
            false,
            Op::Cmd("graphics.newText"),
            [
                arg("text", "string", "Title text.", true),
                arg("seconds", "number", "Duration in seconds when creating a new clip.", false),
                arg("time", "integer", "Start in ticks.", false),
                arg("position", "array", "Anchor position [x, y] in pixels.", false),
                arg("size", "number", "Font size.", false),
                arg("font", "string", "Font name, for example Noto Serif.", false),
                arg("clip", "integer", "Add another layer to this graphic clip.", false),
                arg("newClip", "boolean", "Force a new clip.", false),
            ],
        ),
        t(
            "shape_add",
            "Add shape",
            "Create a rectangle or ellipse graphic.",
            false,
            false,
            false,
            Op::Cmd("graphics.newRectangle"),
            [
                arg("shape", "string", "rectangle (default) or ellipse.", false),
                arg("position", "array", "[x, y] in pixels.", false),
                arg("size", "array", "[width, height] in pixels.", false),
                arg("clip", "integer", "Add the shape to this graphic clip.", false),
                arg("seconds", "number", "Duration when creating a new clip.", false),
                arg("time", "integer", "Start in ticks (254016000000 = 1 second). Default: the playhead.", false),
            ],
        ),
        t(
            "effect_apply",
            "Apply effect",
            "Apply an effect by id or display name (gaussian_blur, Lumetri Color, …). See effect_list.",
            false,
            false,
            false,
            Op::Cmd("effects.apply"),
            [arg("effect", "string", "Effect id or name.", true), arg("clips", "array", "Clip ids. Default: the selection.", false)],
        ),
        t(
            "effect_remove",
            "Remove effect",
            "Remove one non-intrinsic effect by index on a clip.",
            false,
            true,
            false,
            Op::Cmd("effects.remove"),
            [arg("clip", "integer", "Clip id.", true), arg("index", "integer", "Effect index on that clip.", true)],
        ),
        t(
            "effect_set_param",
            "Set effect parameter",
            "Set one effect parameter. Pass effect as an id or index.",
            false,
            false,
            false,
            Op::Cmd("effects.setParam"),
            [
                arg("clip", "integer", "Clip id.", true),
                arg("effect", "string", "Effect id (lumetri, gaussian_blur, motion, opacity, …).", true),
                arg("param", "string", "Parameter id (exposure, blurriness, scale, …).", true),
                arg("value", "any", "New value: number, bool, string, or array.", true),
            ],
        ),
        t(
            "keyframe_set",
            "Set keyframe",
            "Animate a parameter. Writes a keyframe at seconds (timeline) and turns animation on. effect defaults to motion; use opacity, graphic_text, or graphic_shape for fades and title or shape motion. ease is linear, easeIn, easeOut, hold, bezier, or autoBezier (easeOut leaves this key, easeIn arrives at it).",
            false,
            false,
            false,
            Op::Keyframe,
            [
                arg("clip", "integer", "Clip id.", true),
                arg("param", "string", "Parameter id: position, scale, rotation, opacity, …", true),
                arg("value", "any", "Number, [x, y], color, or a choice name.", true),
                arg("seconds", "number", "Timeline time in seconds. Default: the playhead.", false),
                arg("frame", "integer", "Timeline frame, if seconds is omitted.", false),
                arg("time", "integer", "Timeline ticks, if seconds and frame are omitted.", false),
                arg("effect", "string", "Effect id. Default motion. graphic_text and graphic_shape animate a title or shape layer.", false),
                arg("layer", "integer", "Which layer of that effect id (0 is the first). Default 0 when effect is a graphic layer.", false),
                arg("ease", "string", "linear, easeIn, easeOut, hold, bezier, or autoBezier.", false),
            ],
        ),
        t(
            "effect_list",
            "List effects",
            "Built-in video, audio, and transition effects with parameter ids.",
            true,
            false,
            true,
            Op::Effects,
            [arg("query", "string", "Filter by id or name.", false), arg("kind", "string", "Video, Audio, VideoTransition, or AudioTransition.", false)],
        ),
        t(
            "color_grade",
            "Lumetri grade",
            "Apply Lumetri Color and set basic correction (exposure, contrast, saturation, temperature, tint, highlights, shadows, whites, blacks, vibrance).",
            false,
            false,
            false,
            Op::Color,
            [
                arg("clip", "integer", "One clip id.", false),
                arg("clips", "array", "Clip ids. Default: the selection.", false),
                arg("exposure", "number", "Exposure, about -5 to 5.", false),
                arg("contrast", "number", "Contrast, -100 to 100.", false),
                arg("saturation", "number", "Saturation, 0 to 200 (100 is unchanged).", false),
                arg("temperature", "number", "Temperature, -100 to 100.", false),
                arg("tint", "number", "Tint, -100 to 100.", false),
                arg("highlights", "number", "Highlights.", false),
                arg("shadows", "number", "Shadows.", false),
                arg("whites", "number", "Whites.", false),
                arg("blacks", "number", "Blacks.", false),
                arg("vibrance", "number", "Creative vibrance.", false),
            ],
        ),
        t(
            "audio_gain",
            "Audio clip gain",
            "Set, adjust, or normalize clip gain in dB (clip.audioGain).",
            false,
            false,
            false,
            Op::Cmd("clip.audioGain"),
            [
                arg("clips", "array", "Audio clip ids. Default: the selection.", false),
                arg("mode", "string", "set, adjust, normalizeMax, or normalizeAll. Default set when db is given.", false),
                arg("db", "number", "Gain in dB, or the normalize target.", false),
            ],
        ),
        t(
            "audio_fade",
            "Audio fade",
            "Fade a clip's audio edge with an audio transition (default exponential fade).",
            false,
            false,
            false,
            Op::AudioFade,
            [
                arg("clip", "integer", "Audio clip id. Default: the selection.", false),
                arg("edge", "string", "in or out. Default in.", false),
                arg("frames", "integer", "Fade length in frames.", false),
                arg("effect", "string", "exponential_fade, constant_power, or constant_gain.", false),
            ],
        ),
        t(
            "audio_peak",
            "Audio peak",
            "Peak amplitude of the selected audio clips, with current gain.",
            true,
            false,
            true,
            Op::Cmd("clip.audioPeak"),
            [arg("clips", "array", "Clip ids. Default: the selection.", false)],
        ),
        t(
            "marker_add",
            "Add marker",
            "Add a sequence marker at a time.",
            false,
            false,
            false,
            Op::Cmd("markers.add"),
            [
                arg("seconds", "number", "Time in seconds. Default: the playhead.", false),
                arg("frame", "integer", "Frame.", false),
                arg("time", "integer", "Ticks.", false),
                arg("name", "string", "Marker name.", false),
                arg("comment", "string", "Comment.", false),
                arg("color", "string", "Label color such as Rose or Green.", false),
            ],
        ),
        t(
            "marker_range",
            "Range marker",
            "Add a marker that spans a duration.",
            false,
            false,
            false,
            Op::Cmd("markers.addRange"),
            [
                arg("seconds", "number", "Start in seconds.", false),
                arg("frame", "integer", "Start frame.", false),
                arg("durationFrames", "integer", "Length in frames.", false),
                arg("name", "string", "Name.", false),
                arg("comment", "string", "Comment.", false),
                arg("color", "string", "Label color.", false),
            ],
        ),
        t(
            "marker_clear",
            "Clear markers",
            "Clear sequence markers. all=true clears every marker; otherwise the marker at the playhead.",
            false,
            true,
            false,
            Op::Cmd("markers.clearAll"),
            [
                arg("all", "boolean", "When true, clear every marker. When false or omitted, clear the current marker.", false),
                arg("target", "string", "program (default) or source.", false),
            ],
        ),
        t("mark_in", "Mark in", "Set the sequence In point.", false, false, true, Op::Cmd("markers.markIn"), time_args()),
        t("mark_out", "Mark out", "Set the sequence Out point.", false, false, true, Op::Cmd("markers.markOut"), time_args()),
        t(
            "mark_clear",
            "Clear in and out",
            "Clear In and Out.",
            false,
            false,
            true,
            Op::Cmd("markers.clearInOut"),
            [arg("target", "string", "program or source.", false)],
        ),
        t("undo", "Undo", "Undo the last edit.", false, false, false, Op::Cmd("edit.undo"), std::iter::empty()),
        t("redo", "Redo", "Redo the last undone edit.", false, false, false, Op::Cmd("edit.redo"), std::iter::empty()),
        t("playhead_set", "Set playhead", "Move the playhead.", false, false, true, Op::Cmd("playhead.set"), time_args()),
        t(
            "capture_frame",
            "Capture frame",
            "Render one program frame to PNG. Returns an image and, if path is set, writes the file. Does not move the playhead.",
            true,
            false,
            true,
            Op::Capture,
            [
                arg("seconds", "number", "Time in seconds. Default: the playhead.", false),
                arg("path", "string", "PNG path to write.", false),
                arg("maxSide", "integer", "Longest side in pixels after scaling. 0 keeps the sequence size. Default 960.", false),
            ],
        ),
        t(
            "export_frame",
            "Export frame",
            "Write the program (or source) frame through file.exportFrame.",
            false,
            false,
            true,
            Op::Cmd("file.exportFrame"),
            [
                arg("path", "string", "Output image path.", true),
                arg("target", "string", "program (default) or source.", false),
                arg("format", "string", "png, bmp, or another still format the command accepts.", false),
            ],
        ),
        t(
            "export_media",
            "Export / render",
            "Render the sequence (file.exportMedia). Waits until the file is written unless wait is false.",
            false,
            false,
            false,
            Op::Cmd("file.exportMedia"),
            [
                arg("path", "string", "Output path.", true),
                arg("format", "string", "h264, prores, wav, png, gif, aac, …", false),
                arg("preset", "string", "Export preset name.", false),
                arg("width", "integer", "Output width.", false),
                arg("height", "integer", "Output height.", false),
                arg("fps", "number", "Output frame rate. Default: the sequence rate.", false),
                arg("audio", "boolean", "Include audio. Default true.", false),
                arg("range", "string", "entire, inOut, workArea, or custom.", false),
                arg("startSeconds", "number", "Custom range start.", false),
                arg("endSeconds", "number", "Custom range end.", false),
                arg("wait", "boolean", "Block until the encode finishes. Default true.", false),
            ],
        ),
        t(
            "close_gap",
            "Close gap",
            "Ripple-delete the gap on a track at a time.",
            false,
            true,
            false,
            Op::Cmd("sequence.closeGap"),
            [
                arg("track", "string", "Track name or id.", true),
                arg("seconds", "number", "Time inside the gap.", false),
                arg("frame", "integer", "Frame inside the gap.", false),
                arg("time", "integer", "Ticks inside the gap.", false),
            ],
        ),
        t("jobs_list", "List jobs", "Background jobs (exports) and their progress.", true, false, true, Op::Cmd("jobs.list"), std::iter::empty()),
        t("state_query", "Query state", "Project tree plus the active sequence, in one call.", true, false, true, Op::State, std::iter::empty()),
        t(
            "command_list",
            "List engine commands",
            "Every engine command id, label, menu, shortcut, parameter doc, and whether it is enabled.",
            true,
            false,
            true,
            Op::Commands,
            [arg("filter", "string", "Substring of the id or label.", false), arg("enabledOnly", "boolean", "Only commands that are enabled now.", false)],
        ),
        t(
            "command_run",
            "Run engine command",
            "Run any engine command by id. params is passed through unchanged. Edits are undoable.",
            false,
            true,
            false,
            Op::Run,
            [arg("id", "string", "Command id, for example timeline.razor.", true), arg("params", "object", "Parameter object.", false)],
        ),
    ]
}

fn place_args() -> Vec<Arg> {
    vec![
        arg("item", "integer", "Project item id. Use item or items.", false),
        arg("items", "array", "Item ids laid back to back as one undo step.", false),
        arg("track", "string", "Video track (V1) or an audio track when the item is sound only. \"new\" adds a track.", false),
        arg("audioTrack", "string", "Audio track (A1), or \"new\".", false),
        arg("seconds", "number", "Timeline start in seconds. Default: the playhead.", false),
        arg("frame", "integer", "Timeline start frame.", false),
        arg("time", "integer", "Timeline start in ticks.", false),
        arg("timecode", "string", "Timeline start timecode.", false),
        arg("durationSeconds", "number", "Clip length in seconds. Converted to ticks.", false),
        arg("duration", "integer", "Clip length in ticks. Default: the rest of the source.", false),
        arg("sourceInSeconds", "number", "Source in-point in seconds.", false),
        arg("sourceIn", "integer", "Source in-point in ticks.", false),
        arg("video", "boolean", "Place picture. Default true.", false),
        arg("audio", "boolean", "Place sound. Default true.", false),
    ]
}

fn range_args() -> Vec<Arg> {
    vec![
        arg("seconds", "number", "In point in seconds.", false),
        arg("frame", "integer", "In point frame.", false),
        arg("time", "integer", "In point in ticks.", false),
        arg("endSeconds", "number", "Out point in seconds.", false),
        arg("endFrame", "integer", "Out point frame.", false),
    ]
}

fn time_args() -> Vec<Arg> {
    vec![
        arg("seconds", "number", "Time in seconds.", false),
        arg("frame", "integer", "Frame index.", false),
        arg("time", "integer", "Ticks (254016000000 = 1 second).", false),
        arg("timecode", "string", "Timecode.", false),
        arg("target", "string", "program or source, where the command accepts it.", false),
    ]
}

fn transition_args() -> Vec<Arg> {
    vec![
        arg("clip", "integer", "Clip whose edge receives the transition.", false),
        arg("effect", "string", "Transition id or name. Video default is cross dissolve.", false),
        arg("frames", "integer", "Duration in frames.", false),
        arg("edge", "string", "in or out.", false),
        arg("params", "object", "Parameter overrides.", false),
        arg("reverse", "boolean", "Reverse the transition.", false),
    ]
}

pub fn call(server: &Server, params: &Value) -> Result<Value, (i64, String)> {
    let name = params.get("name").and_then(Value::as_str).ok_or((-32602, "tools/call needs a name".to_string()))?;
    let tool = tools().iter().find(|t| t.name == name).ok_or_else(|| (-32602, format!("unknown tool \"{name}\"")))?;
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let obj = args.as_object().ok_or((-32602, "arguments must be an object".to_string()))?;
    if let Some(bad) = obj.keys().find(|k| !tool.args.iter().any(|a| a.name == k.as_str())) {
        let accepted = if tool.args.is_empty() { "no arguments".to_string() } else { tool.args.iter().map(|a| a.name).collect::<Vec<_>>().join(", ") };
        return Err((-32602, format!("unknown argument \"{bad}\" for {name}; expected: {accepted}")));
    }
    for spec in tool.args.iter().filter(|a| a.required) {
        match obj.get(spec.name) {
            None | Some(Value::Null) => return Err((-32602, format!("missing argument \"{}\" for {name}", spec.name))),
            Some(v) if !type_ok(spec.json_type, v) => {
                return Err((-32602, format!("argument \"{}\" for {name} must be a {}", spec.name, spec.json_type)));
            }
            _ => {}
        }
    }
    for spec in tool.args.iter().filter(|a| !a.required) {
        if let Some(v) = obj.get(spec.name)
            && !v.is_null()
            && !type_ok(spec.json_type, v)
        {
            return Err((-32602, format!("argument \"{}\" for {name} must be a {}", spec.name, spec.json_type)));
        }
    }
    let op = tool.op;
    let outcome = server.with_session(|session| dispatch(session, op, &args));
    match outcome {
        Ok(v) => Ok(tool_result_text(&v, false)),
        Err(e) => Ok(tool_result_text(&json!({"error": e}), true)),
    }
}

fn type_ok(ty: &str, v: &Value) -> bool {
    match ty {
        "string" => v.is_string(),
        "number" => v.is_number(),
        "any" => v.is_number() || v.is_boolean() || v.is_string() || v.is_array() || v.is_object(),
        "integer" => v.as_i64().is_some() || v.as_u64().is_some(),
        "boolean" => v.is_boolean(),
        "array" => v.is_array(),
        "object" => v.is_object(),
        _ => true,
    }
}

fn dispatch(session: &mut Session, op: Op, args: &Value) -> RunResult {
    match op {
        Op::Cmd(id) => {
            let mut params = args.clone();
            if id == "file.exportMedia" && params.get("wait").is_none() {
                params["wait"] = json!(true);
            }
            if id == "markers.clearAll" {
                let all = params.get("all").and_then(Value::as_bool).unwrap_or(false);
                if let Some(obj) = params.as_object_mut() {
                    obj.remove("all");
                }
                let cmd = if all { "markers.clearAll" } else { "markers.clearCurrent" };
                return exec(session, cmd, params);
            }
            if id == "graphics.newRectangle" {
                let shape = params.get("shape").and_then(Value::as_str).unwrap_or("rectangle").to_string();
                if let Some(obj) = params.as_object_mut() {
                    obj.remove("shape");
                }
                let cmd = if shape.eq_ignore_ascii_case("ellipse") { "graphics.newEllipse" } else { "graphics.newRectangle" };
                return exec(session, cmd, params);
            }
            if id == "clip.audioGain" && params.get("mode").is_none() {
                params["mode"] = json!("set");
            }
            if id == "sequence.deleteTrack" || id == "timeline.setTrack" || id == "timeline.setTargeting" || id == "sequence.closeGap" {
                coerce_track(&mut params, "track");
            }
            exec(session, id, params)
        }
        Op::Place(insert) => place(session, args, insert),
        Op::Move => move_clips(session, args),
        Op::Lift(extract) => lift(session, args, extract),
        Op::Color => color(session, args),
        Op::AudioFade => audio_fade(session, args),
        Op::Probe => {
            let path = args.get("path").and_then(Value::as_str).ok_or("need path")?;
            crate::probe::probe_path(path)
        }
        Op::Capture => capture(session, args),
        Op::State => state_query(session, args),
        Op::Effects => effect_list(args),
        Op::Commands => command_list(session, args),
        Op::Run => {
            let id = args.get("id").and_then(Value::as_str).ok_or("need id")?;
            let params = args.get("params").cloned().unwrap_or_else(|| json!({}));
            exec(session, id, params)
        }
        Op::Keyframe => keyframe_set(session, args),
    }
}

fn keyframe_set(session: &mut Session, args: &Value) -> RunResult {
    let clip = args.get("clip").cloned().ok_or("need clip")?;
    let param = args.get("param").and_then(Value::as_str).ok_or("need param")?;
    let value = args.get("value").cloned().ok_or("need value")?;
    let mut effect = args.get("effect").cloned().unwrap_or_else(|| json!("motion"));
    if let Some(layer) = args.get("layer").and_then(Value::as_u64) {
        let kind = effect.as_str().unwrap_or("graphic_text");
        let index = graphic_layer_index(session, clip.as_u64().ok_or("clip must be an integer")?, kind, layer)?;
        effect = json!(index);
    }
    let mut params = json!({"clip": clip, "effect": effect, "param": param, "value": value, "keyframe": true});
    for key in ["seconds", "frame", "time"] {
        if let Some(v) = args.get(key) {
            params[key] = v.clone();
        }
    }
    let result = exec(session, "effects.setParam", params.clone())?;
    if let Some(ease) = args.get("ease").and_then(Value::as_str) {
        let clip_id = clip.as_u64().ok_or("clip must be an integer")?;
        let media = keyframe_media_time(session, clip_id, args)?;
        let interp = json!({"clip": clip, "effect": effect, "param": param, "interpolation": ease, "mediaTime": media});
        exec(session, "effects.setInterpolation", interp)?;
    }
    Ok(result)
}

/// Media time of a timeline keyframe, including one that sits on the clip's last tick.
/// `effects.setInterpolation` clamps a timeline `seconds` to `end - 1`, which misses that key.
fn keyframe_media_time(session: &Session, clip: u64, args: &Value) -> Result<i64, String> {
    let seq = session.active_sequence().ok_or("no sequence is open")?;
    let (_, item) = seq.find_item(filmcraft_project::ClipId(clip)).ok_or_else(|| format!("no clip {clip}"))?;
    let timeline = if let Some(t) = args.get("time").and_then(Value::as_i64) {
        Tick(t)
    } else if let Some(frame) = args.get("frame").and_then(Value::as_i64) {
        session.sequence_rate().tick_of(frame)
    } else if let Some(seconds) = args.get("seconds").and_then(Value::as_f64) {
        Tick::from_seconds_f64(seconds)
    } else {
        session.playhead()
    };
    Ok(item.source_time_at(timeline).0)
}

fn graphic_layer_index(session: &Session, clip: u64, kind: &str, layer: u64) -> Result<usize, String> {
    let seq = session.active_sequence().ok_or("no sequence is open")?;
    let (_, item) = seq.find_item(filmcraft_project::ClipId(clip)).ok_or_else(|| format!("no clip {clip}"))?;
    let mut seen = 0u64;
    for (index, effect) in item.effects.iter().enumerate() {
        let matches = effect.effect == kind || (kind == "graphic" && (effect.effect == "graphic_text" || effect.effect == "graphic_shape"));
        if !matches {
            continue;
        }
        if seen == layer {
            return Ok(index);
        }
        seen += 1;
    }
    Err(format!("clip {clip} has no {kind} layer {layer}"))
}

fn exec(session: &mut Session, id: &str, params: Value) -> RunResult {
    session.execute(id, params).map_err(|e| e.to_string())
}

/// Track commands accept a name (`"V1"`) or a numeric id. JSON integers stay integers.
fn coerce_track(params: &mut Value, key: &str) {
    let Some(raw) = params.get(key).and_then(Value::as_str) else { return };
    if let Ok(n) = raw.parse::<u64>() {
        params[key] = json!(n);
    }
}

fn place(session: &mut Session, args: &Value, insert: bool) -> RunResult {
    let mut params = args.clone();
    if let Some(sec) = take_f64(&mut params, "durationSeconds") {
        params["duration"] = json!(Tick::from_seconds_f64(sec).0);
    }
    if let Some(sec) = take_f64(&mut params, "sourceInSeconds") {
        params["sourceIn"] = json!(Tick::from_seconds_f64(sec).0);
    }
    params["insert"] = json!(insert);
    coerce_track(&mut params, "track");
    coerce_track(&mut params, "audioTrack");
    exec(session, "timeline.place", params)
}

fn take_f64(params: &mut Value, key: &str) -> Option<f64> {
    let v = params.as_object_mut()?.remove(key)?;
    v.as_f64().or_else(|| v.as_i64().map(|n| n as f64))
}

fn move_clips(session: &mut Session, args: &Value) -> RunResult {
    let mut params = args.clone();
    let rate = session.sequence_rate();
    if let Some(moves) = params.get_mut("moves").and_then(Value::as_array_mut) {
        for m in moves {
            if m.get("time").and_then(Value::as_i64).is_none() {
                if let Some(sec) = m.get("seconds").and_then(Value::as_f64) {
                    m["time"] = json!(Tick::from_seconds_f64(sec).0);
                } else if let Some(frame) = m.get("frame").and_then(Value::as_i64) {
                    m["time"] = json!(rate.tick_of(frame).0);
                }
            }
            if let Some(obj) = m.as_object_mut() {
                obj.remove("seconds");
                obj.remove("frame");
                if let Some(track) = obj.get("track").and_then(Value::as_str)
                    && let Ok(n) = track.parse::<u64>()
                {
                    obj.insert("track".into(), json!(n));
                }
            }
        }
    }
    exec(session, "timeline.move", params)
}

fn lift(session: &mut Session, args: &Value, extract: bool) -> RunResult {
    let has_in = ["seconds", "frame", "time", "timecode"].iter().any(|k| args.get(*k).is_some());
    if has_in {
        exec(session, "markers.markIn", args.clone())?;
    }
    if let Some(end) = args.get("endSeconds") {
        exec(session, "markers.markOut", json!({"seconds": end}))?;
    } else if let Some(end) = args.get("endFrame") {
        exec(session, "markers.markOut", json!({"frame": end}))?;
    }
    exec(session, if extract { "sequence.extract" } else { "sequence.lift" }, json!({}))
}

fn color(session: &mut Session, args: &Value) -> RunResult {
    let mut apply = json!({"effect": "lumetri"});
    if let Some(clips) = args.get("clips") {
        apply["clips"] = clips.clone();
    } else if let Some(clip) = args.get("clip") {
        apply["clips"] = json!([clip]);
    }
    exec(session, "effects.apply", apply.clone())?;
    let clips: Vec<u64> = if let Some(arr) = args.get("clips").and_then(Value::as_array) {
        arr.iter().filter_map(Value::as_u64).collect()
    } else if let Some(c) = args.get("clip").and_then(Value::as_u64) {
        vec![c]
    } else {
        let insp = exec(session, "sequence.inspect", json!({}))?;
        insp.get("selection").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default()
    };
    const KEYS: [&str; 10] = ["exposure", "contrast", "saturation", "temperature", "tint", "highlights", "shadows", "whites", "blacks", "vibrance"];
    let mut set = 0u32;
    for clip in clips {
        for key in KEYS {
            if let Some(value) = args.get(key) {
                exec(session, "effects.setParam", json!({"clip": clip, "effect": "lumetri", "param": key, "value": value}))?;
                set += 1;
            }
        }
    }
    Ok(json!({"effect": "lumetri", "paramsSet": set}))
}

fn audio_fade(session: &mut Session, args: &Value) -> RunResult {
    let mut params = json!({
        "effect": args.get("effect").and_then(Value::as_str).unwrap_or("exponential_fade"),
        "edge": args.get("edge").and_then(Value::as_str).unwrap_or("in"),
    });
    if let Some(clip) = args.get("clip") {
        params["clip"] = clip.clone();
    }
    if let Some(frames) = args.get("frames") {
        params["frames"] = frames.clone();
    }
    exec(session, "sequence.applyAudioTransition", params)
}

fn capture(session: &mut Session, args: &Value) -> RunResult {
    let seconds = args.get("seconds").and_then(Value::as_f64);
    let max_side = args.get("maxSide").and_then(Value::as_u64).unwrap_or(960) as u32;
    let t = seconds.map(Tick::from_seconds_f64).unwrap_or_else(|| session.playhead());
    let (w, h) = session.active_sequence().map(|q| (q.settings.width, q.settings.height)).ok_or_else(|| "no sequence is open".to_string())?;
    let scale = render_scale(max_side, w, h);
    let img = session.try_render_program_at(scale, t).map_err(|e| e.to_string())?;
    let png = filmcraft_automation::png_rgba(img.w as u32, img.h as u32, img.over_black_rgba8(), max_side).map_err(|e| e.to_string())?;
    let path = args.get("path").and_then(Value::as_str);
    if let Some(path) = path {
        if let Some(parent) = std::path::Path::new(path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|e| format!("{path}: {e}"))?;
        }
        std::fs::write(path, &png).map_err(|e| format!("{path}: {e}"))?;
    }
    let png_base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png);
    Ok(json!({
        "width": img.w,
        "height": img.h,
        "seconds": seconds,
        "path": path,
        "pngBase64": png_base64,
    }))
}

fn render_scale(max_side: u32, w: u32, h: u32) -> f32 {
    if max_side == 0 || w.max(h) <= max_side { 1.0 } else { max_side as f32 / (w.max(h) as f32) }
}

pub fn state_query(session: &mut Session, _args: &Value) -> RunResult {
    let project = exec(session, "project.inspect", json!({}))?;
    let sequence = exec(session, "sequence.inspect", json!({})).ok();
    Ok(json!({"project": project, "sequence": sequence, "toolCount": tools().len()}))
}

fn effect_list(args: &Value) -> RunResult {
    let query = args.get("query").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
    let kind = args.get("kind").and_then(Value::as_str).unwrap_or("");
    let mut effects = Vec::new();
    for effect in filmcraft_project::effect_defs() {
        let kind_name = format!("{:?}", effect.kind);
        if !kind.is_empty() && !kind_name.eq_ignore_ascii_case(kind) {
            continue;
        }
        if !query.is_empty() && !effect.id.to_ascii_lowercase().contains(&query) && !effect.name.to_ascii_lowercase().contains(&query) {
            continue;
        }
        effects.push(json!({
            "id": effect.id,
            "name": effect.name,
            "kind": kind_name,
            "category": effect.category,
            "params": effect.params.iter().map(|p| p.id).collect::<Vec<_>>(),
        }));
    }
    let count = effects.len();
    Ok(json!({"count": count, "effects": effects}))
}

fn command_list(session: &mut Session, args: &Value) -> RunResult {
    let list = exec(session, "command.list", json!({}))?;
    let filter = args.get("filter").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
    let only = args.get("enabledOnly").and_then(Value::as_bool).unwrap_or(false);
    let commands: Vec<Value> = list
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| {
            let id = c.get("id").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
            let label = c.get("label").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
            (filter.is_empty() || id.contains(&filter) || label.contains(&filter)) && (!only || c.get("enabled").and_then(Value::as_bool) == Some(true))
        })
        .collect();
    Ok(Value::Array(commands))
}

pub fn tools_markdown() -> String {
    let mut out = String::new();
    out.push_str("# FilmCraft MCP tools\n\n");
    out.push_str(&format!(
        "{} named tools. Each row is wired to the FilmCraft engine (a command id, the program renderer, or the demuxer). `command_run` still reaches every other engine command.\n\n",
        tools().len()
    ));
    out.push_str("| Tool | Calls | Read-only | Arguments |\n|---|---|---|---|\n");
    for tool in tools() {
        let args = if tool.args.is_empty() {
            "—".to_string()
        } else {
            tool.args
                .iter()
                .map(|a| {
                    let req = if a.required { "" } else { "?" };
                    format!("`{}{}`", a.name, req)
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let ro = if tool.read_only { "yes" } else { "no" };
        out.push_str(&format!("| `{}` | {} | {} | {} |\n", tool.name, tool.engine().replace('|', "\\|"), ro, args));
    }
    out.push_str("\n`?` marks an optional argument. Time arguments accept `seconds`, `frame`, `time` (ticks; 254016000000 = 1 second) or `timecode` when the engine command does. On `title_add` and `shape_add`, `seconds` is the clip duration and `time` is the start in ticks (the playhead, if omitted).\n\n");
    out.push_str("## What each tool does\n\n");
    for tool in tools() {
        out.push_str(&format!("- **`{}`** — {}\n", tool.name, tool.description));
    }
    out
}
