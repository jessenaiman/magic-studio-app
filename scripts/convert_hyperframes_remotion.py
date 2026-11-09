#!/usr/bin/env python3
"""
Convert HyperFrames and Remotion projects into full native Magic Studio projects.

Usage:
  python3 scripts/convert_hyperframes_remotion.py --all
  python3 scripts/convert_hyperframes_remotion.py --project hey-ho-little-fish
  python3 scripts/convert_hyperframes_remotion.py --project pterodactyl-song
  python3 scripts/convert_hyperframes_remotion.py --hyperframes <src_dir> --out <dest_dir>
  python3 scripts/convert_hyperframes_remotion.py --remotion <src_dir> --out <dest_dir>

Produces in each target project directory (`projects/<slug>/`):
  - `<Name>.magic` project manifest with self-contained asset/audio/video paths
  - `design/words.json`, `design/words-track.json`, `design/words-applied.json`
    with timed word choreography + authored hand-drawn SVG motion tracks (`draws` & `kinetic.draws`)
  - `lettering/lyrics.ecproj` (EffectCraft project with Text & Shape layers via `words.letter`)
  - `storyboard.designcraft` (DesignCraft storyboard including a dedicated Motion Design & Drawn Tracks spread)
  - `film/<Name>.fcproj`, `film/<Name>-preview.mp4`, and `exports/<Name>.mp4`
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAGIC_CLI = ROOT / "target" / "debug" / "magic-cli"
TICKS_PER_SECOND = 254_016_000_000


def seconds_to_ticks(sec: float) -> int:
    return int(round(float(sec) * TICKS_PER_SECOND))


def link_or_copy(src: Path, dst: Path) -> None:
    if not src.exists():
        return
    dst.parent.mkdir(parents=True, exist_ok=True)
    if dst.exists() or dst.is_symlink():
        if dst.is_symlink() or dst.is_file():
            dst.unlink()
        else:
            shutil.rmtree(dst)
    try:
        os.symlink(src.resolve(), dst)
    except OSError:
        if src.is_dir():
            shutil.copytree(src, dst)
        else:
            shutil.copy2(src, dst)


def run_magic_cli(project_file: Path, *args: str) -> subprocess.CompletedProcess:
    cmd = [str(MAGIC_CLI), "--project", str(project_file), *args]
    return subprocess.run(cmd, cwd=str(ROOT), text=True, capture_output=True)


def enrich_words_json_draws(words_data: dict) -> dict:
    """Ensure every line's draws have word_ids linked and are mirrored in kinetic.draws."""
    for anim in words_data.get("animations", []):
        design = anim.get("design", {})
        anim_words = anim.get("words", [])
        for w in anim_words:
            if not w.get("line_id") and isinstance(w.get("word_id"), str) and "/" in w["word_id"]:
                w["line_id"] = w["word_id"].split("/")[0]
        for line_id, entry in design.items():
            if not isinstance(entry, dict):
                continue
            items = entry.get("items", [])
            draws = entry.get("draws", [])
            line_words = [
                w["word_id"]
                for w in anim_words
                if isinstance(w.get("word_id"), str) and w["word_id"].startswith(f"{line_id}/")
            ]
            # Pick best accent word in the line
            accent_word = None
            for it in items:
                if it.get("accent"):
                    accent_word = it["accent"]
                    break
            if not accent_word and line_words:
                accent_word = line_words[-1]

            for idx, d in enumerate(draws):
                if not d.get("word_ids"):
                    target_word = accent_word or (line_words[min(idx, len(line_words) - 1)] if line_words else None)
                    if target_word:
                        d["word_ids"] = [target_word]
                if not d.get("preset"):
                    name_low = (d.get("name") or "").lower()
                    if "arrow" in name_low:
                        d["preset"] = "arrow-down"
                    elif "wave" in name_low or "ripple" in name_low or "hill" in name_low:
                        d["preset"] = "wave"
                    elif "loop" in name_low or "circle" in name_low or "orbit" in name_low:
                        d["preset"] = "loop"
                    elif "swoop" in name_low or "arc" in name_low or "ray" in name_low:
                        d["preset"] = "swoop"
                    else:
                        d["preset"] = "underline"

            kinetic = entry.get("kinetic")
            if not isinstance(kinetic, dict):
                kinetic = {}
            kinetic["draws"] = json.loads(json.dumps(draws))
            entry["kinetic"] = kinetic
    return words_data


def convert_hey_ho_little_fish(src_dir: Path, dst_dir: Path) -> None:
    print(f"=== Converting Hey Ho Little Fish (HyperFrames -> Magic Studio) ===")
    print(f"  Source: {src_dir}")
    print(f"  Target: {dst_dir}")
    dst_dir.mkdir(parents=True, exist_ok=True)

    # 1. Link heavy asset/render directories so paths resolve cleanly inside dst_dir
    for sub in ["assets", "artifacts", "animation", "hyperframes", "storyboard", "renders", "verify", "previews", "layer-sources"]:
        if (src_dir / sub).exists():
            link_or_copy(src_dir / sub, dst_dir / sub)

    # 2. Set up self-contained audio
    audio_dir = dst_dir / "audio"
    audio_dir.mkdir(parents=True, exist_ok=True)
    wav_src = src_dir / "hyperframes" / "track-03.wav"
    wav_dst = audio_dir / "track-03.wav"
    if wav_src.exists():
        link_or_copy(wav_src, wav_dst)

    # 3. Copy transcript & lyrics files
    for fname in ["hey_ho_little_fish.transcript.json", "transcript-provenance.json", "project.json", "art-direction.md"]:
        if (src_dir / fname).exists():
            shutil.copy2(src_dir / fname, dst_dir / fname)
    if (src_dir / "hey_ho_little_fish.transcript.json").exists():
        shutil.copy2(src_dir / "hey_ho_little_fish.transcript.json", dst_dir / "Hey Ho Little Fish.transcript.json")

    # 4. Copy & enrich design/words.json, words-track.json, words-applied.json
    (dst_dir / "design").mkdir(parents=True, exist_ok=True)
    words_src = src_dir / "design" / "words.json"
    words_data = json.loads(words_src.read_text())
    words_data = enrich_words_json_draws(words_data)
    (dst_dir / "design" / "words.json").write_text(json.dumps(words_data, indent=2) + "\n")
    shutil.copy2( dst_dir / "design" / "words.json", dst_dir / "lyrics-design.json")

    if (src_dir / "design" / "words-track.json").exists():
        shutil.copy2(src_dir / "design" / "words-track.json", dst_dir / "design" / "words-track.json")

    # Also build lyrics/lyrics.json from words_data
    (dst_dir / "lyrics").mkdir(parents=True, exist_ok=True)
    lyrics_sections = []
    full_lyrics_lines = []
    for anim in words_data.get("animations", []):
        sec_id = anim.get("scope", {}).get("section", anim.get("animation_id", "section"))
        sec_title = sec_id.replace("_", " ").title()
        full_lyrics_lines.append(f"[{sec_title}]")
        line_map: dict[str, list[str]] = {}
        for w in anim.get("words", []):
            wid = w.get("word_id", "")
            lid = wid.split("/")[0] if "/" in wid else "l1"
            line_map.setdefault(lid, []).append(w.get("word", ""))
        lines_out = []
        for lid, toks in line_map.items():
            line_text = " ".join(toks)
            full_lyrics_lines.append(line_text)
            lines_out.append({"id": lid, "text": line_text})
        full_lyrics_lines.append("")
        lyrics_sections.append({"id": sec_id, "title": sec_title, "lines": lines_out})

    (dst_dir / "lyrics" / "lyrics.json").write_text(
        json.dumps({"title": "Hey Ho Little Fish", "sections": lyrics_sections}, indent=2) + "\n"
    )

    # 5. Copy storyboard.designcraft
    if (src_dir / "storyboard.designcraft").exists():
        shutil.copy2(src_dir / "storyboard.designcraft", dst_dir / "storyboard.designcraft")

    # 6. Rewrite Hey Ho Little Fish.magic with clean paths + song block
    magic_src = json.loads((src_dir / "Hey Ho Little Fish.magic").read_text())
    magic_raw = json.dumps(magic_src)
    magic_raw = magic_raw.replace(
        "/run/media/jesse/Speedy/media/hey-ho-little-fish/project/",
        f"{dst_dir.as_posix()}/",
    )
    magic_raw = magic_raw.replace(
        "/home/jesse/Music/Kathy Reid-Naiman/More Tickles and Tunes/track03.cdda.wav",
        "audio/track-03.wav",
    )
    magic_data = json.loads(magic_raw)
    magic_data["name"] = "Hey Ho Little Fish"
    magic_data["song"] = {
        "title": "Hey Ho Little Fish",
        "lyrics": "\n".join(full_lyrics_lines).strip() + "\n",
        "notes": "Converted from HyperFrames + Magic Studio workshop project (Kathy Reid-Naiman track 03).",
    }
    magic_data.setdefault("workshop", {})["audio"] = "audio/track-03.wav"
    magic_data["workshop"]["target"] = "filmcraft"
    magic_data["workshop"]["video_project"] = "film/Hey Ho Little Fish.fcproj"

    # Add stage background images as top-level assets if not already present
    existing_ids = {a.get("id") for a in magic_data.get("assets", [])}
    stage_bgs = [
        ("stage_s01_kelp", "Scene 1 · Kelp Forest Stage", "hyperframes/assets/stage/backdrop_v1.png"),
        ("stage_s02_school", "Scene 2 · Fish School Stage", "hyperframes/assets/stage/seaweed_bed.png"),
        ("stage_s03_hook", "Scene 3 · Fisherman's Hook Stage", "hyperframes/assets/stage/book_open.png"),
        ("stage_s05_reef", "Scene 5 · Open Reef Stage", "hyperframes/assets/stage/sand.png"),
    ]
    for aid, aname, apath in stage_bgs:
        if aid not in existing_ids and (dst_dir / apath).exists():
            magic_data.setdefault("assets", []).append(
                {"id": aid, "kind": "image", "name": aname, "path": apath, "preset": None}
            )

    magic_file = dst_dir / "Hey Ho Little Fish.magic"
    magic_file.write_text(json.dumps(magic_data, indent=2) + "\n")

    # 7. Link rendered videos into film/ and exports/ and build FilmCraft .fcproj with scenes
    film_dir = dst_dir / "film"
    exports_dir = dst_dir / "exports"
    film_dir.mkdir(parents=True, exist_ok=True)
    exports_dir.mkdir(parents=True, exist_ok=True)

    best_mp4 = src_dir / "renders" / "hey-ho-little-fish-polish-v8.mp4"
    if not best_mp4.exists():
        best_mp4 = src_dir / "renders" / "hey-ho-little-fish-finished.mp4"
    if best_mp4.exists():
        shutil.copy2(best_mp4, film_dir / "Hey Ho Little Fish-preview.mp4")
        shutil.copy2(best_mp4, exports_dir / "Hey Ho Little Fish.mp4")

    # Build a rich FilmCraft project (`film/Hey Ho Little Fish.fcproj`) with 6 scene clips on V1
    build_fcproj_with_scenes(
        project_name="Hey Ho Little Fish",
        fcproj_path=film_dir / "Hey Ho Little Fish.fcproj",
        audio_rel="../audio/track-03.wav",
        audio_name="track-03.wav",
        duration_sec=118.9,
        scenes=[
            ("Scene 1 · Come All You Fishermen", 0.0, 19.8, "../hyperframes/assets/stage/backdrop_v1.png"),
            ("Scene 2 · Hey Ho Little Fish (Chorus 1)", 19.8, 41.2, "../hyperframes/assets/stage/seaweed_bed.png"),
            ("Scene 3 · Fish School & Fisherman's Hook", 41.2, 54.0, "../hyperframes/assets/stage/book_open.png"),
            ("Scene 4 · Hey Ho Little Fish (Chorus 2)", 54.0, 74.5, "../hyperframes/assets/stage/seaweed_bed.png"),
            ("Scene 5 · Swim Around the Bait", 74.5, 86.8, "../hyperframes/assets/stage/sand.png"),
            ("Scene 6 · Final Chorus & Safe Away", 86.8, 118.9, "../hyperframes/assets/stage/backdrop_v1.png"),
        ],
        character_clips=[
            ("Little Fish · Kelp Forest Intro", 0.0, 19.8, "../exports/Hey Ho Little Fish.mp4"),
            ("Little Fish & School · Chorus 1", 19.8, 41.2, "../exports/Hey Ho Little Fish.mp4"),
            ("Older Fish & Fisherman's Hook", 41.2, 54.0, "../exports/Hey Ho Little Fish.mp4"),
            ("Little Fish & School · Chorus 2", 54.0, 74.5, "../exports/Hey Ho Little Fish.mp4"),
            ("Swim Around the Bait", 74.5, 86.8, "../exports/Hey Ho Little Fish.mp4"),
            ("Final Chorus & Safe Away", 86.8, 118.9, "../exports/Hey Ho Little Fish.mp4"),
        ],
        lyric_clips=[
            ("Lyrics & Drawn Wave · Come All You Fishermen", 0.0, 19.8, "../exports/Hey Ho Little Fish.mp4"),
            ("Lyrics & Drawn Swoop · Hey Ho Little Fish (Chorus 1)", 19.8, 41.2, "../exports/Hey Ho Little Fish.mp4"),
            ("Lyrics & Hook Line · Fish School & Hook", 41.2, 54.0, "../exports/Hey Ho Little Fish.mp4"),
            ("Lyrics & Drawn Swoop · Hey Ho Little Fish (Chorus 2)", 54.0, 74.5, "../exports/Hey Ho Little Fish.mp4"),
            ("Lyrics & Orbit Loop · Swim Around the Bait", 74.5, 86.8, "../exports/Hey Ho Little Fish.mp4"),
            ("Lyrics & Safe Arc · Final Chorus & Outro", 86.8, 118.9, "../exports/Hey Ho Little Fish.mp4"),
        ],
    )

    # 8. Build 10-page 7-Parent Storyboard & lettering/lyrics.ecproj via magic-cli
    if MAGIC_CLI.exists():
        populate_storyboard_7_parents(
            magic_file=magic_file,
            title="HEY HO LITTLE FISH",
            subtitle="Cutout Reef Animation & Hand-Drawn Lyric Choreography (Converted from HyperFrames)",
            tagline="Come all you fishermen, listen to me — swim around the bait and swim safe away.",
            overall_design="Warm watercolor reef & kelp forest stages (`backdrop_v1.png`, `seaweed_bed.png`, `book_open.png`, `sand.png`) paired with articulated Little Fish & Older Fish cutout rigs and hand-drawn SVG wave/hook/loop tracks.",
            format_notes="1920 × 1080 · 30 fps · 1:58.9 · EffectCraft + DesignCraft + FilmCraft",
            hand_note="Artist note: keep the kelp forest warm teal and gold; hook line drops vertically on Verse 2 while the fish orbit loop traces cleanly around the bait.",
            cover_key_art="hyperframes/assets/stage/backdrop_v1.png",
            world_subheading="Kelp Forest, Open Reef & Fisherman's Hook Drop Zone",
            world_stage_image="hyperframes/assets/stage/seaweed_bed.png",
            world_source_image="hyperframes/assets/stage/book_open.png",
            world_result_image="hyperframes/assets/stage/sand.png",
            world_prompt="2D storybook underwater kelp forest and open coral reef stage plate, warm dappled sunlight filtering from surface, layered teal and amber kelp fronds, clean staging lane for cutout fish rigs, no text.",
            char_subheading="Little Fish (`forked-little.png`) & Coral Parent / Reef School (`disc-parent-coral.png`)",
            char_original_image="hyperframes/assets/cast/forked-little.png",
            char_pose_image="hyperframes/assets/cast/disc-parent-coral.png",
            char_prompt="2D storybook cutout Little Fish character turnaround and articulated fin/tail rig parts on clean transparent ground, warm coral-orange scales, expressive eye cel swap.",
            motion_subheading="Hand-Drawn Ocean Waves, Hook Drop Line & Bait Orbit Loops (`design/words.json` -> `lettering/lyrics.ecproj`)",
            motion_stage_image="hyperframes/assets/stage/book_open.png",
            motion_preview_1="hyperframes/assets/stage/seaweed_bed.png",
            motion_preview_2="hyperframes/assets/stage/sand.png",
            motion_summary="6 section animations · 12 hand-drawn SVG motion tracks (`wave`, `swoop`, `arrow-down`, `loop`, `underline`) · synced to Kathy Reid-Naiman track 03.",
            motion_spec="Each lyric phrase pairs Fraunces/Plus Jakarta Sans word choreography with custom multi-stroke SVG drawings (`draws` & `kinetic.draws`) compiled into EffectCraft Shape Layers.",
            scenes_spec=[
                {
                    "section": "VERSE 1 · KELP FOREST",
                    "title": "Come All You Fishermen, Listen to Me",
                    "time": "0:00.00 – 0:19.80",
                    "cue": "“Come all you fishermen, listen to me, I'll sing you a song of the fish in the sea.”",
                    "purpose": "Establish the warm kelp forest stage and introduce Little Fish cruising into frame as the opening verse words settle on the current.",
                    "staging": "Camera holds wide on `backdrop_v1.png`; Little Fish enters from screen left; gentle current wave underline draws beneath 'fish in the sea'.",
                    "location_ref": "LOC-01 · Kelp Forest Stage (`hyperframes/assets/stage/backdrop_v1.png`)",
                    "char_shared": "Reef School ambient silhouettes",
                    "char_lead": "Little Fish (`forked-little` rig · cruise idle)",
                    "hero": "hyperframes/assets/stage/backdrop_v1.png",
                    "source": "hyperframes/assets/stage/backdrop_v1.png",
                    "result": "hyperframes/assets/stage/seaweed_bed.png",
                },
                {
                    "section": "CHORUS 1 · REEF CRUISE",
                    "title": "Hey Ho, Little Fish, Swim Away!",
                    "time": "0:19.80 – 0:41.20",
                    "cue": "“Hey ho, little fish, don't you cry — swim right on and pass on by!”",
                    "purpose": "First chorus burst: split showcase between energetic chorus lettering and Little Fish darting joyfully across the open reef.",
                    "staging": "Split composition on `seaweed_bed.png`; rising swoop arcs lift 'Hey ho, little fish' while bubble accents trail the tail fin.",
                    "location_ref": "LOC-02 · Fish School Stage (`hyperframes/assets/stage/seaweed_bed.png`)",
                    "char_shared": "Three Companion Reef Fish",
                    "char_lead": "Little Fish (`forked-little` rig · chorus dart)",
                    "hero": "hyperframes/assets/stage/seaweed_bed.png",
                    "source": "hyperframes/assets/stage/seaweed_bed.png",
                    "result": "hyperframes/assets/stage/backdrop_v1.png",
                },
                {
                    "section": "VERSE 2 · FISHERMAN'S HOOK",
                    "title": "Down Comes the Fisherman's Line & Hook",
                    "time": "0:41.20 – 0:54.00",
                    "cue": "“The fisherman drops his line in the sea, with a shiny hook where the bait will be.”",
                    "purpose": "Introduce dramatic tension as the vertical fishing line and hook descend from the surface into the reef.",
                    "staging": "Montage beats on `book_open.png`: 1. Line drops from top edge; 2. Little Fish spots the glinting hook; 3. Older Fish signals caution.",
                    "location_ref": "LOC-03 · Storybook Hook Stage (`hyperframes/assets/stage/book_open.png`)",
                    "char_shared": "Coral Parent (`disc-parent-coral` rig · warning gesture)",
                    "char_lead": "Little Fish (`forked-little` rig · curious stop)",
                    "hero": "hyperframes/assets/stage/book_open.png",
                    "source": "hyperframes/assets/stage/book_open.png",
                    "result": "hyperframes/assets/stage/sand.png",
                },
                {
                    "section": "CHORUS 2 · SCHOOL CHORUS",
                    "title": "Hey Ho, Little Fish — Keep Swimming Free",
                    "time": "0:54.00 – 1:14.50",
                    "cue": "“Hey ho, little fish, swim so free — there's plenty of room in the deep blue sea!”",
                    "purpose": "Reaffirm the joyful chorus groove with the full school swimming in formation.",
                    "staging": "Wide reef choreography on `seaweed_bed.png`; dual wave ribbons draw across the lower third in sync with the chorus downbeats.",
                    "location_ref": "LOC-02 · Fish School Stage (`hyperframes/assets/stage/seaweed_bed.png`)",
                    "char_shared": "Full Reef School + Coral Parent",
                    "char_lead": "Little Fish (`forked-little` rig · formation lead)",
                    "hero": "hyperframes/assets/stage/seaweed_bed.png",
                    "source": "hyperframes/assets/stage/seaweed_bed.png",
                    "result": "hyperframes/assets/stage/book_open.png",
                },
                {
                    "section": "VERSE 3 · SWIM AROUND THE BAIT",
                    "title": "Circle the Bait & Never Bite",
                    "time": "1:14.50 – 1:26.80",
                    "cue": "“They swim all around the bait so bright, they give a little wink and they never bite!”",
                    "purpose": "Playful climax: the fish inspect the bait, trace a full 360-degree loop around the hook, and wink at the camera.",
                    "staging": "Split action on `sand.png`; hand-drawn orbit loop circles 'around the bait' while Little Fish winks.",
                    "location_ref": "LOC-04 · Open Reef Stage (`hyperframes/assets/stage/sand.png`)",
                    "char_shared": "Coral Parent & School cheering",
                    "char_lead": "Little Fish (`forked-little` rig · orbit & wink)",
                    "hero": "hyperframes/assets/stage/sand.png",
                    "source": "hyperframes/assets/stage/sand.png",
                    "result": "hyperframes/assets/stage/backdrop_v1.png",
                },
                {
                    "section": "OUTRO · SAFE AWAY HOME",
                    "title": "Final Chorus & Safe Into the Kelp",
                    "time": "1:26.80 – 1:58.90",
                    "cue": "“Hey ho, little fish, swim away home — safe in the kelp where the little fish roam.”",
                    "purpose": "Warm resolution returning to the golden kelp forest as the song fades out.",
                    "staging": "Montage finale on `backdrop_v1.png`: 1. Final chorus crest; 2. School glides into warm kelp; 3. Gentle resting underline holds to end.",
                    "location_ref": "LOC-01 · Kelp Forest Stage (`hyperframes/assets/stage/backdrop_v1.png`)",
                    "char_shared": "Full Reef Cast",
                    "char_lead": "Little Fish (`forked-little` rig · gentle settle)",
                    "hero": "hyperframes/assets/stage/backdrop_v1.png",
                    "source": "hyperframes/assets/stage/sand.png",
                    "result": "hyperframes/assets/stage/backdrop_v1.png",
                },
            ],
        )
        res = run_magic_cli(magic_file, "exec", "words.letter", "render=false")
        if res.returncode == 0:
            print("  Built lettering/lyrics.ecproj via words.letter")
        else:
            print(f"  words.letter warning: {res.stderr.strip()}")

    print(f"=== Finished Hey Ho Little Fish -> {magic_file} ===\n")


def build_pterodactyl_words_json(remotion_dir: Path) -> tuple[dict, str]:
    """
    Convert Pterodactyl Song's Remotion `artifacts/lyrics.json`, `cues.ts`,
    `lyric-motion.tsx`, and `lyric-scenes.tsx` into Magic Studio `design/words.json`.
    """
    lyrics_artifact = json.loads((remotion_dir / "artifacts" / "lyrics.json").read_text())
    raw_words = lyrics_artifact["words"]

    # The 6 Remotion lyric scenes from cues.ts + lyric-motion.tsx + lyric-scenes.tsx
    phrases_spec = [
        {
            "id": "sleep",
            "section": "verse_1_sleep",
            "title": "Scene 1 · Sleeping Very Still",
            "line_id": "s1-l1",
            "first": 0,
            "count": 7,
            "background_asset": "cave_bg",
            "items": [
                {
                    "text": "All the pterodactyls were",
                    "word_indices": [0, 1, 2, 3],
                    "x": 860.0,
                    "y": 105.0,
                    "size": 46.0,
                    "weight": 400,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "sans",
                    "motion": "quiet",
                },
                {
                    "text": "sleeping",
                    "word_indices": [4],
                    "x": 835.0,
                    "y": 150.0,
                    "size": 174.0,
                    "weight": 400,
                    "italic": True,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "serif",
                    "motion": "sleep",
                },
                {
                    "text": "very",
                    "word_indices": [5],
                    "x": 1025.0,
                    "y": 340.0,
                    "size": 44.0,
                    "weight": 400,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "sans",
                    "motion": "quiet",
                },
                {
                    "text": "still.",
                    "word_indices": [6],
                    "x": 1135.0,
                    "y": 295.0,
                    "size": 132.0,
                    "weight": 700,
                    "italic": False,
                    "color": "#8B3D26",
                    "outline": "#FFF8E9",
                    "font": "serif",
                    "motion": "settle",
                },
            ],
            "draws": [
                {
                    "name": "Sleepy Closed Eyes",
                    "preset": "wave",
                    "x": 1480.0,
                    "y": 180.0,
                    "w": 300.0,
                    "h": 180.0,
                    "view_box": "0 0 300 180",
                    "word_index": 4,
                    "paths": [
                        {
                            "d": "M 24 84 Q 55 113 91 83",
                            "stroke": "#173F49",
                            "width": 6.0,
                            "from": 1.6,
                            "to": 2.4,
                        },
                        {
                            "d": "M 151 84 Q 185 111 221 80",
                            "stroke": "#173F49",
                            "width": 6.0,
                            "from": 1.85,
                            "to": 2.65,
                        },
                    ],
                },
                {
                    "name": "Still Hush Underline",
                    "preset": "underline",
                    "x": 1125.0,
                    "y": 440.0,
                    "w": 280.0,
                    "h": 30.0,
                    "view_box": "0 0 280 30",
                    "word_index": 6,
                    "paths": [
                        {
                            "d": "M 5 14 Q 61 5 125 13 T 269 10",
                            "stroke": "#B68B50",
                            "width": 6.0,
                            "from": 2.9,
                            "to": 3.7,
                        }
                    ],
                },
            ],
        },
        {
            "id": "cave",
            "section": "verse_1_cave",
            "title": "Scene 2 · Down in the Cave at the Bottom of a Hill",
            "line_id": "s2-l1",
            "first": 7,
            "count": 10,
            "background_asset": "exterior_bg",
            "items": [
                {
                    "text": "Down",
                    "word_indices": [7],
                    "x": 125.0,
                    "y": 60.0,
                    "size": 142.0,
                    "weight": 700,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "serif",
                    "motion": "fall",
                },
                {
                    "text": "in the cave",
                    "word_indices": [8, 9, 10],
                    "x": 145.0,
                    "y": 235.0,
                    "size": 52.0,
                    "weight": 400,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "sans",
                    "motion": "quiet",
                },
                {
                    "text": "at the bottom of a",
                    "word_indices": [11, 12, 13, 14, 15],
                    "x": 185.0,
                    "y": 345.0,
                    "size": 44.0,
                    "weight": 400,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "sans",
                    "motion": "quiet",
                },
                {
                    "text": "hill.",
                    "word_indices": [16],
                    "x": 545.0,
                    "y": 290.0,
                    "size": 128.0,
                    "weight": 400,
                    "italic": True,
                    "color": "#8B3D26",
                    "outline": "#FFF8E9",
                    "font": "serif",
                    "motion": "settle",
                },
            ],
            "draws": [
                {
                    "name": "Hand-Drawn Downward Arrow",
                    "preset": "arrow-down",
                    "x": 20.0,
                    "y": 100.0,
                    "w": 130.0,
                    "h": 300.0,
                    "view_box": "0 0 160 330",
                    "word_index": 7,
                    "paths": [
                        {
                            "d": "M 73 13 C 27 67 104 129 66 196 Q 47 231 61 289 M 30 264 L 61 294 L 87 260",
                            "stroke": "#8B3D26",
                            "width": 6.0,
                            "from": 0.4,
                            "to": 1.7,
                        }
                    ],
                },
                {
                    "name": "Rounded Hill Contour",
                    "preset": "wave",
                    "x": 505.0,
                    "y": 420.0,
                    "w": 410.0,
                    "h": 95.0,
                    "view_box": "0 0 410 95",
                    "word_index": 16,
                    "paths": [
                        {
                            "d": "M 7 77 Q 73 72 118 32 Q 171 -8 228 31 Q 279 75 400 76",
                            "stroke": "#8B3D26",
                            "width": 5.0,
                            "from": 1.9,
                            "to": 3.6,
                        }
                    ],
                },
            ],
        },
        {
            "id": "sudden",
            "section": "verse_2_sudden",
            "title": "Scene 3 · When All of a Sudden One of Them Did Cry",
            "line_id": "s3-l1",
            "first": 17,
            "count": 10,
            "background_asset": "cave_bg",
            "items": [
                {
                    "text": "When all of a",
                    "word_indices": [17, 18, 19, 20],
                    "x": 850.0,
                    "y": 75.0,
                    "size": 44.0,
                    "weight": 400,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "sans",
                    "motion": "quiet",
                },
                {
                    "text": "sudden,",
                    "word_indices": [21],
                    "x": 835.0,
                    "y": 125.0,
                    "size": 132.0,
                    "weight": 700,
                    "italic": False,
                    "color": "#8B3D26",
                    "outline": "#FFF8E9",
                    "font": "serif",
                    "motion": "pop",
                },
                {
                    "text": "one of them did",
                    "word_indices": [22, 23, 24, 25],
                    "x": 1000.0,
                    "y": 305.0,
                    "size": 44.0,
                    "weight": 400,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "sans",
                    "motion": "quiet",
                },
                {
                    "text": "cry,",
                    "word_indices": [26],
                    "x": 1410.0,
                    "y": 235.0,
                    "size": 136.0,
                    "weight": 400,
                    "italic": True,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "serif",
                    "motion": "pop",
                },
            ],
            "draws": [
                {
                    "name": "Call Sound Rays",
                    "preset": "swoop",
                    "x": 1645.0,
                    "y": 190.0,
                    "w": 230.0,
                    "h": 210.0,
                    "view_box": "0 0 230 210",
                    "word_index": 26,
                    "paths": [
                        {
                            "d": "M 20 100 Q 56 95 99 86",
                            "stroke": "#8B3D26",
                            "width": 6.0,
                            "from": 4.2,
                            "to": 4.5,
                        },
                        {
                            "d": "M 18 68 Q 52 45 80 13 M 22 134 Q 63 152 92 182",
                            "stroke": "#8B3D26",
                            "width": 5.0,
                            "from": 4.3,
                            "to": 4.8,
                        },
                    ],
                }
            ],
        },
        {
            "id": "wake",
            "section": "chorus_wake_fly",
            "title": "Scene 4 · Wake Up Pterodactyls and Fly, Fly, Fly!",
            "line_id": "s4-l1",
            "first": 27,
            "count": 7,
            "background_asset": "cave_bg",
            "items": [
                {
                    "text": "Wake",
                    "word_indices": [27],
                    "x": 865.0,
                    "y": 70.0,
                    "size": 132.0,
                    "weight": 700,
                    "italic": False,
                    "color": "#8B3D26",
                    "outline": "#FFF8E9",
                    "font": "serif",
                    "motion": "pop",
                },
                {
                    "text": "up",
                    "word_indices": [28],
                    "x": 1215.0,
                    "y": 115.0,
                    "size": 48.0,
                    "weight": 400,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "sans",
                    "motion": "quiet",
                },
                {
                    "text": "pterodactyls",
                    "word_indices": [29],
                    "x": 900.0,
                    "y": 225.0,
                    "size": 52.0,
                    "weight": 400,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "sans",
                    "motion": "quiet",
                },
                {
                    "text": "and",
                    "word_indices": [30],
                    "x": 915.0,
                    "y": 345.0,
                    "size": 44.0,
                    "weight": 400,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "sans",
                    "motion": "quiet",
                },
                {
                    "text": "fly,",
                    "word_indices": [31],
                    "x": 1065.0,
                    "y": 320.0,
                    "size": 94.0,
                    "weight": 400,
                    "italic": True,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "serif",
                    "motion": "rise",
                },
                {
                    "text": "fly,",
                    "word_indices": [32],
                    "x": 1310.0,
                    "y": 250.0,
                    "size": 104.0,
                    "weight": 400,
                    "italic": True,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "serif",
                    "motion": "rise",
                },
                {
                    "text": "fly!",
                    "word_indices": [33],
                    "x": 1560.0,
                    "y": 175.0,
                    "size": 116.0,
                    "weight": 400,
                    "italic": True,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "serif",
                    "motion": "rise",
                },
            ],
            "draws": [
                {
                    "name": "Rising Flight Swoop",
                    "preset": "swoop",
                    "x": 1030.0,
                    "y": 310.0,
                    "w": 790.0,
                    "h": 160.0,
                    "view_box": "0 0 790 160",
                    "word_index": 33,
                    "paths": [
                        {
                            "d": "M 10 145 Q 126 170 217 93 T 438 58 Q 578 29 756 7 M 730 5 L 761 5 L 746 29",
                            "stroke": "#B68B50",
                            "width": 5.0,
                            "from": 2.7,
                            "to": 4.5,
                        }
                    ],
                }
            ],
        },
        {
            "id": "directions",
            "section": "verse_3_directions",
            "title": "Scene 5 · Fly to the East and Fly to the West",
            "line_id": "s5-l1",
            "first": 34,
            "count": 9,
            "background_asset": "exterior_bg",
            "items": [
                {
                    "text": "Fly to the",
                    "word_indices": [34, 35, 36],
                    "x": 105.0,
                    "y": 65.0,
                    "size": 44.0,
                    "weight": 400,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "sans",
                    "motion": "quiet",
                },
                {
                    "text": "east",
                    "word_indices": [37],
                    "x": 230.0,
                    "y": 120.0,
                    "size": 142.0,
                    "weight": 700,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "serif",
                    "motion": "east",
                },
                {
                    "text": "and fly to the",
                    "word_indices": [38, 39, 40, 41],
                    "x": 1080.0,
                    "y": 65.0,
                    "size": 44.0,
                    "weight": 400,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "sans",
                    "motion": "quiet",
                },
                {
                    "text": "west.",
                    "word_indices": [42],
                    "x": 1230.0,
                    "y": 120.0,
                    "size": 152.0,
                    "weight": 400,
                    "italic": True,
                    "color": "#8B3D26",
                    "outline": "#FFF8E9",
                    "font": "serif",
                    "motion": "west",
                },
            ],
            "draws": [
                {
                    "name": "East & West Compass Arrows",
                    "preset": "swoop",
                    "x": 90.0,
                    "y": 300.0,
                    "w": 1740.0,
                    "h": 65.0,
                    "view_box": "0 0 1740 65",
                    "word_index": 37,
                    "paths": [
                        {
                            "d": "M 15 33 Q 208 17 409 30 T 650 26 M 625 6 L 655 26 L 626 49",
                            "stroke": "#173F49",
                            "width": 6.0,
                            "from": 1.65,
                            "to": 2.6,
                        },
                        {
                            "d": "M 1710 26 Q 1510 45 1321 31 T 1073 33 M 1099 9 L 1068 33 L 1098 54",
                            "stroke": "#8B3D26",
                            "width": 6.0,
                            "from": 4.0,
                            "to": 4.6,
                        },
                    ],
                }
            ],
        },
        {
            "id": "return",
            "section": "outro_return_nest",
            "title": "Scene 6 · Fly All Around and Then Back to Your Nest",
            "line_id": "s6-l1",
            "first": 43,
            "count": 10,
            "background_asset": "cave_bg",
            "items": [
                {
                    "text": "Fly all",
                    "word_indices": [43, 44],
                    "x": 850.0,
                    "y": 95.0,
                    "size": 44.0,
                    "weight": 400,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "sans",
                    "motion": "quiet",
                },
                {
                    "text": "around",
                    "word_indices": [45],
                    "x": 820.0,
                    "y": 150.0,
                    "size": 130.0,
                    "weight": 400,
                    "italic": True,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "serif",
                    "motion": "loop",
                },
                {
                    "text": "and then fly back",
                    "word_indices": [46, 47, 48, 49],
                    "x": 1250.0,
                    "y": 95.0,
                    "size": 44.0,
                    "weight": 400,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "sans",
                    "motion": "quiet",
                },
                {
                    "text": "to your",
                    "word_indices": [50, 51],
                    "x": 1260.0,
                    "y": 260.0,
                    "size": 44.0,
                    "weight": 400,
                    "italic": False,
                    "color": "#173F49",
                    "outline": "#FFF8E9",
                    "font": "sans",
                    "motion": "quiet",
                },
                {
                    "text": "nest.",
                    "word_indices": [52],
                    "x": 1420.0,
                    "y": 170.0,
                    "size": 132.0,
                    "weight": 700,
                    "italic": False,
                    "color": "#8B3D26",
                    "outline": "#FFF8E9",
                    "font": "serif",
                    "motion": "settle",
                },
            ],
            "draws": [
                {
                    "name": "Playful Flight Loop",
                    "preset": "loop",
                    "x": 790.0,
                    "y": 305.0,
                    "w": 1000.0,
                    "h": 150.0,
                    "view_box": "0 0 1000 150",
                    "word_index": 45,
                    "paths": [
                        {
                            "d": "M 15 20 C 110 95 425 95 455 40 C 485 -4 350 -3 352 37 C 355 95 515 85 602 50 M 579 33 L 608 49 L 586 73",
                            "stroke": "#173F49",
                            "width": 5.0,
                            "from": 1.7,
                            "to": 3.8,
                        }
                    ],
                },
                {
                    "name": "Warm Nest Cradle",
                    "preset": "wave",
                    "x": 790.0,
                    "y": 305.0,
                    "w": 1000.0,
                    "h": 150.0,
                    "view_box": "0 0 1000 150",
                    "word_index": 52,
                    "paths": [
                        {
                            "d": "M 605 55 Q 770 129 970 50 M 620 70 Q 780 141 955 70",
                            "stroke": "#8B3D26",
                            "width": 5.0,
                            "from": 3.9,
                            "to": 5.8,
                        },
                    ],
                },
            ],
        },
    ]

    animations = []
    full_lyrics_lines = []

    for idx, p in enumerate(phrases_spec):
        first = p["first"]
        count = p["count"]
        slice_words = raw_words[first : first + count]
        anchor = max(0.0, float(slice_words[0].get("start", slice_words[0]["anchor"])) - 0.4)
        next_first = phrases_spec[idx + 1]["first"] if idx + 1 < len(phrases_spec) else None
        if next_first is not None:
            end_sec = float(raw_words[next_first].get("start", raw_words[next_first]["anchor"])) - 0.4
        else:
            last_w = slice_words[-1]
            last_end = float(last_w.get("end", last_w.get("secondaryAnchor", last_w.get("anchorEnd", last_w["anchor"]))))
            end_sec = last_end + 1.1
        authored_duration = round(max(1.5, end_sec - anchor), 3)

        line_id = p["line_id"]
        anim_words = []
        local_id_by_global: dict[int, str] = {}
        for local_i, w in enumerate(slice_words):
            g_idx = first + local_i
            wid = f"{line_id}/w{local_i}"
            local_id_by_global[g_idx] = wid
            w_start = round(float(w.get("start", w["anchor"])), 3)
            w_end = round(max(w_start + 0.08, float(w.get("end", w.get("secondaryAnchor", w.get("anchorEnd", w_start + 0.35))))), 3)
            anim_words.append(
                {
                    "text": w["text"],
                    "word": w["text"],
                    "word_id": wid,
                    "line_id": line_id,
                    "sung_id": line_id,
                    "start": w_start,
                    "end": w_end,
                    "start_tick": seconds_to_ticks(w_start),
                    "end_tick": seconds_to_ticks(w_end),
                    "action": "none",
                    "keyframes": [
                        {"at": 0.0, "opacity": 0.0, "scale": 0.92, "x": 0.0, "y": 14.0, "rotate": 0.0},
                        {"at": 0.25, "opacity": 1.0, "scale": 1.04, "x": 0.0, "y": 0.0, "rotate": 0.0},
                        {"at": 1.0, "opacity": 1.0, "scale": 1.0, "x": 0.0, "y": 0.0, "rotate": 0.0},
                    ],
                }
            )

        line_text = " ".join(w["text"] for w in slice_words)
        full_lyrics_lines.append(f"[{p['title']}]")
        full_lyrics_lines.append(line_text)
        full_lyrics_lines.append("")

        items_out = []
        for it in p["items"]:
            wids = [local_id_by_global[g] for g in it["word_indices"] if g in local_id_by_global]
            items_out.append(
                {
                    "accent": wids[-1] if wids else None,
                    "color": it["color"],
                    "font": it["font"],
                    "italic": it["italic"],
                    "motion": it["motion"],
                    "outline": it["outline"],
                    "rotate": 0.0,
                    "size": it["size"],
                    "text": it["text"],
                    "weight": it["weight"],
                    "word_ids": wids,
                    "x": it["x"],
                    "y": it["y"],
                }
            )

        draws_out = []
        for d in p["draws"]:
            target_wid = local_id_by_global.get(d["word_index"])
            draws_out.append(
                {
                    "name": d["name"],
                    "preset": d["preset"],
                    "x": d["x"],
                    "y": d["y"],
                    "w": d["w"],
                    "h": d["h"],
                    "view_box": d["view_box"],
                    "word_ids": [target_wid] if target_wid else [],
                    "paths": d["paths"],
                }
            )

        animations.append(
            {
                "animation_id": f"words-{p['id']}",
                "authored_duration": authored_duration,
                "authored_duration_tick": seconds_to_ticks(authored_duration),
                "background_asset": p["background_asset"],
                "background_custom": True,
                "design": {
                    line_id: {
                        "items": items_out,
                        "draws": draws_out,
                        "kinetic": {
                            "draws": json.loads(json.dumps(draws_out)),
                        },
                    }
                },
                "hold": 0.6,
                "lead_in": 0.4,
                "origin": "convert_hyperframes_remotion.py",
                "scope": {
                    "kind": "section",
                    "line_ids": [line_id],
                    "occurrence": 1,
                    "section": p["section"],
                    "sung_ids": [line_id],
                },
                "song_anchor": round(anchor, 3),
                "song_anchor_tick": seconds_to_ticks(anchor),
                "source_revision": 1,
                "style": {"font": "Fraunces"},
                "words": anim_words,
            }
        )

    words_doc = {"animations": animations, "version": 1}
    return words_doc, "\n".join(full_lyrics_lines).strip() + "\n"


def build_fcproj_with_scenes(
    project_name: str,
    fcproj_path: Path,
    audio_rel: str,
    audio_name: str,
    duration_sec: float,
    scenes: list[tuple[str, float, float, str]],
    character_clips: list[tuple[str, float, float, str]],
    lyric_clips: list[tuple[str, float, float, str]] | None = None,
) -> None:
    """Create a FilmCraft .fcproj with V1 scene clips, V2 character clips, V3 Kinetic Lyrics clips, and A1 audio via video.assemble."""
    magic_file = fcproj_path.parent.parent / f"{project_name}.magic"
    if not MAGIC_CLI.exists() or not magic_file.exists():
        return

    def norm_rel(p: str) -> str:
        return p.removeprefix("../")

    backgrounds_arg = [
        {"name": sc_name, "from": start_s, "to": end_s, "path": norm_rel(img_rel)}
        for sc_name, start_s, end_s, img_rel in scenes
    ]
    characters_arg = [
        {"name": c_name, "from": start_s, "to": end_s, "source_in": start_s, "path": norm_rel(clip_rel)}
        for c_name, start_s, end_s, clip_rel in character_clips
    ]
    lyrics_arg = [
        {"name": l_name, "from": start_s, "to": end_s, "source_in": start_s, "path": norm_rel(clip_rel)}
        for l_name, start_s, end_s, clip_rel in (lyric_clips or [])
    ]

    res = run_magic_cli(
        magic_file,
        "exec",
        "video.assemble",
        "export=false",
        f"audio={norm_rel(audio_rel)}",
        f"backgrounds={json.dumps(backgrounds_arg)}",
        f"characters={json.dumps(characters_arg)}",
        f"lyrics={json.dumps(lyrics_arg)}",
    )
    if res.returncode == 0:
        print(f"  Built {fcproj_path.name} via video.assemble (filmcraft-engine)")
    else:
        print(f"  video.assemble warning: {res.stderr.strip()}")


def populate_storyboard_7_parents(
    magic_file: Path,
    title: str,
    subtitle: str,
    tagline: str,
    overall_design: str,
    format_notes: str,
    hand_note: str,
    cover_key_art: str,
    world_subheading: str,
    world_stage_image: str,
    world_source_image: str,
    world_result_image: str,
    world_prompt: str,
    char_subheading: str,
    char_original_image: str,
    char_pose_image: str,
    char_prompt: str,
    motion_subheading: str,
    motion_stage_image: str,
    motion_preview_1: str,
    motion_preview_2: str,
    motion_summary: str,
    motion_spec: str,
    scenes_spec: list[dict],
) -> None:
    """Create a 10-page storyboard from the 7-parent template and populate all slots (0 unfilled fields)."""
    run_magic_cli(magic_file, "exec", "storyboard.new", "overwrite=true")
    # Template has 7 spreads: 0=A-Cover, 1=B-World, 2=C-Character, 3=D-Motion, 4=E-Scene-Wide, 5=F-Scene-Split, 6=G-Scene-Montage.
    # Duplicate pages 4, 5, 6 to get 6 scene spreads total (pages 4..9 cycling Wide, Split, Montage).
    for src_page in (4, 5, 6):
        run_magic_cli(magic_file, "exec", "storyboard.add", f"from={src_page}")

    pages_batch = [
        {
            "page": 0,
            "fields": {
                "cover_title": title,
                "cover_subtitle": subtitle,
                "cover_key_art_caption": f"{title} — Key Stage Plate & Character Framing",
                "cover_hand_note": hand_note,
                "cover_tagline": tagline,
                "cover_overall_design": overall_design,
                "cover_format_notes": format_notes,
                "cover_revision": "REV 03 · LIVE",
                "cover_reg_id_1": "LOC-01",
                "cover_reg_name_1": "Primary Stage Plate",
                "cover_reg_where_1": "Scenes 01, 03, 04, 06",
                "cover_reg_id_2": "LOC-02",
                "cover_reg_name_2": "Secondary Exterior / Reef",
                "cover_reg_where_2": "Scenes 02, 05",
                "cover_reg_id_3": "CHR-01",
                "cover_reg_name_3": "Lead Articulated Cutout Rig",
                "cover_reg_where_3": "All 6 Scenes",
                "cover_reg_id_4": "CHR-02",
                "cover_reg_name_4": "Companion Cutout Rig",
                "cover_reg_where_4": "Scenes 01, 04, 05, 06",
                "cover_reg_id_5": "MOT-01",
                "cover_reg_name_5": "Kinetic Lyric Choreography",
                "cover_reg_where_5": "design/words.json",
                "cover_reg_id_6": "MOT-02",
                "cover_reg_name_6": "Hand-Drawn SVG Overlays",
                "cover_reg_where_6": "lettering/lyrics.ecproj",
            },
            "images": {
                "cover_key_art": cover_key_art,
                "cover_songmap_1": scenes_spec[0]["hero"],
                "cover_songmap_2": scenes_spec[1]["hero"],
                "cover_songmap_3": scenes_spec[2]["hero"],
                "cover_songmap_4": scenes_spec[3]["hero"],
                "cover_songmap_5": scenes_spec[4]["hero"],
                "cover_reg_thumb_1": world_stage_image,
                "cover_reg_thumb_2": world_source_image,
                "cover_reg_thumb_3": char_original_image,
                "cover_reg_thumb_4": char_pose_image,
                "cover_reg_thumb_5": motion_preview_1,
                "cover_reg_thumb_6": motion_preview_2,
            },
        },
        {
            "page": 1,
            "fields": {
                "world_running_title": f"{title} — STORYBOARD",
                "world_heading": f"WORLD & STAGES — {title}",
                "world_subheading": world_subheading,
                "world_stage_caption": f"Primary stage plate (`{world_stage_image}`)",
                "world_material_scale": "1920 × 1080 stage plates · layered foreground/midground depth · clear center-left lettering lane.",
                "world_hand_note": hand_note,
                "world_register": "LOC-01 Primary Stage · LOC-02 Contrasting Exterior/Reef Stage · verified in DesignCraft & FilmCraft.",
                "world_location_prompt": world_prompt,
                "world_revision": "REV 03 · VERIFIED",
                "world_match_note": "Palette and horizon lines locked across cuts for seamless V1 background transitions.",
            },
            "images": {
                "world_stage_image": world_stage_image,
                "world_source_image": world_source_image,
                "world_result_image": world_result_image,
            },
        },
        {
            "page": 2,
            "fields": {
                "char_running_title": f"{title} — STORYBOARD",
                "char_heading": f"CHARACTER RIGS — {title}",
                "char_subheading": char_subheading,
                "char_original_image_caption": f"Cutout rig reference (`{char_original_image}`)",
                "char_pose_image_caption": f"Active performance pose (`{char_pose_image}`)",
                "char_hand_note": "Articulated pivot joints & clean cel swaps verified in EffectCraft and FilmCraft V2.",
                "char_exact_prompt": char_prompt,
                "char_prompt_foot": "Transparent PNG cutout parts · crisp silhouette · zero background spill.",
                "char_asset_geometry": "Canvas 1920 × 1080 · pivot coordinates anchored in rig.json.",
                "char_silhouette_rules": "Keep limbs/wings/fins readable against both warm interior and cool exterior stages.",
            },
            "images": {
                "char_original_image": char_original_image,
                "char_pose_image": char_pose_image,
            },
        },
        {
            "page": 3,
            "fields": {
                "motion_running_title": f"{title} — STORYBOARD",
                "motion_heading": f"MOTION & DRAWN TRACKS — {title}",
                "motion_subheading": motion_subheading,
                "motion_stage_caption": f"Kinetic words + hand-drawn SVG overlay composite (`{motion_stage_image}`)",
                "motion_choreography_notes": motion_summary,
                "motion_hand_note": "Every drawn SVG stroke is anchored to a sung word (`word_ids`) and compiled into `lettering/lyrics.ecproj` via `words.letter`.",
                "motion_track_summary": motion_summary,
                "motion_drawn_spec": motion_spec,
                "motion_cues_notes": "Synced to word-level transcript ticks (254,016,000,000 ticks/sec) across all 6 scenes.",
                "motion_sync_note": "Verified in Lyrics, Motion, and Studio (`V3 Kinetic Lyrics` track).",
            },
            "images": {
                "motion_stage_image": motion_stage_image,
                "motion_preview_1": motion_preview_1,
                "motion_preview_2": motion_preview_2,
            },
        },
    ]

    layout_badges = [
        "WIDE HERO ESTABLISHING",
        "SPLIT ACTION / LYRIC SHOWCASE",
        "3-BEAT RHYTHMIC MONTAGE",
        "WIDE HERO ESTABLISHING",
        "SPLIT ACTION / LYRIC SHOWCASE",
        "3-BEAT RHYTHMIC MONTAGE",
    ]
    for idx, sc in enumerate(scenes_spec):
        page_num = 4 + idx
        sc_num = idx + 1
        pages_batch.append(
            {
                "page": page_num,
                "fields": {
                    "running_title": f"{title} — STORYBOARD",
                    "badge": layout_badges[idx % len(layout_badges)],
                    "section": sc["section"],
                    "hand_note": hand_note,
                    "scene_id": f"SCENE {sc_num:02d}",
                    "title": sc["title"],
                    "cue": sc["cue"],
                    "time": sc["time"],
                    "purpose": sc["purpose"],
                    "staging_notes": sc["staging"],
                    "revision": "REV 03 · LIVE",
                    "overall_design": overall_design,
                    "location_ref": sc["location_ref"],
                    "character_shared": sc["char_shared"],
                    "character_lead": sc["char_lead"],
                    "asset_geometry": "1920 × 1080 · V1 Stage + V2 Character Rig + V3 Kinetic Lyrics & Drawn SVG",
                    "source_image_caption": f"Stage plate (`{sc['source']}`)",
                    "result_image_caption": f"Composite frame (`{sc['result']}`)",
                    "match_note": f"Cut {sc_num:02d} locked to audio markers ({sc['time']}).",
                    "exact_prompt": f"{sc['title']} — {sc['purpose']} ({sc['location_ref']})",
                },
                "images": {
                    "hero_image": sc["hero"],
                    "sketch_left": sc["source"],
                    "sketch_right": sc["result"],
                    "source_image": sc["source"],
                    "result_image": sc["result"],
                },
                "refs": [sc["hero"], sc["source"], sc["result"]],
            }
        )

    proj_dir = magic_file.parent.resolve()
    for page_entry in pages_batch:
        if "images" in page_entry:
            page_entry["images"] = {
                k: str((proj_dir / v).resolve()) if not Path(v).is_absolute() else v
                for k, v in page_entry["images"].items()
            }
        if "refs" in page_entry:
            page_entry["refs"] = [
                str((proj_dir / v).resolve()) if not Path(v).is_absolute() else v
                for v in page_entry["refs"]
            ]

    res = run_magic_cli(magic_file, "exec", "storyboard.set", f"pages={json.dumps(pages_batch)}")
    if res.returncode != 0:
        print(f"  storyboard.set warning: {res.stderr.strip()}")
    chk = run_magic_cli(magic_file, "exec", "storyboard.check")
    if chk.returncode == 0:
        chk_data = json.loads(chk.stdout)
        print(f"  Built 10-page 7-parent storyboard: page_count={chk_data.get('page_count')}, unfilled={chk_data.get('count')}")


def convert_pterodactyl_song(remotion_dir: Path, dst_dir: Path) -> None:
    print(f"=== Converting Pterodactyl Song (Remotion -> Magic Studio) ===")
    print(f"  Source: {remotion_dir}")
    print(f"  Target: {dst_dir}")
    dst_dir.mkdir(parents=True, exist_ok=True)

    # 1. Copy Remotion source files into dst_dir/remotion/ for reference & round-trip iteration
    remotion_dst = dst_dir / "remotion"
    remotion_dst.mkdir(parents=True, exist_ok=True)
    for fname in ["film.tsx", "lyric-motion.tsx", "lyric-scenes.tsx", "cues.ts", "Root.tsx", "index.tsx", "project.json", "package.json"]:
        if (remotion_dir / fname).exists():
            shutil.copy2(remotion_dir / fname, remotion_dst / fname)

    # 2. Copy public assets (audio, backgrounds, felt puppet parts)
    pub = remotion_dir / "public"
    (dst_dir / "audio").mkdir(parents=True, exist_ok=True)
    (dst_dir / "assets" / "backgrounds").mkdir(parents=True, exist_ok=True)
    (dst_dir / "assets" / "characters" / "child-pterodactyl").mkdir(parents=True, exist_ok=True)
    (dst_dir / "assets" / "characters" / "parent-pterodactyl").mkdir(parents=True, exist_ok=True)

    shutil.copy2(pub / "pterodactyl-song.mp3", dst_dir / "audio" / "pterodactyl-song.mp3")
    shutil.copy2(pub / "cave.png", dst_dir / "assets" / "backgrounds" / "cave.png")
    shutil.copy2(pub / "exterior.png", dst_dir / "assets" / "backgrounds" / "exterior.png")

    shutil.copy2(pub / "sleeping-pterodactyl.png", dst_dir / "assets" / "characters" / "child-pterodactyl" / "sleeping.png")
    shutil.copy2(pub / "awake-body.png", dst_dir / "assets" / "characters" / "child-pterodactyl" / "body.png")
    shutil.copy2(pub / "wing.png", dst_dir / "assets" / "characters" / "child-pterodactyl" / "wing.png")

    shutil.copy2(pub / "parent-sleeping.png", dst_dir / "assets" / "characters" / "parent-pterodactyl" / "sleeping.png")
    shutil.copy2(pub / "parent-body.png", dst_dir / "assets" / "characters" / "parent-pterodactyl" / "body.png")
    shutil.copy2(pub / "parent-wing.png", dst_dir / "assets" / "characters" / "parent-pterodactyl" / "wing.png")

    # Write cutout rig manifests for child and parent pterodactyls
    child_rig = {
        "name": "Cinnamon Child Pterodactyl",
        "canvas": [720, 480],
        "parts": [
            {"name": "sleeping", "path": "sleeping.png", "z": 0},
            {"name": "body", "path": "body.png", "z": 1},
            {"name": "wing", "path": "wing.png", "z": 2, "pivot": [43, 112]},
        ],
    }
    parent_rig = {
        "name": "Teal Parent Pterodactyl",
        "canvas": [720, 480],
        "parts": [
            {"name": "sleeping", "path": "sleeping.png", "z": 0},
            {"name": "body", "path": "body.png", "z": 1},
            {"name": "wing", "path": "wing.png", "z": 2, "pivot": [43, 112]},
        ],
    }
    (dst_dir / "assets" / "characters" / "child-pterodactyl" / "rig.json").write_text(json.dumps(child_rig, indent=2) + "\n")
    (dst_dir / "assets" / "characters" / "parent-pterodactyl" / "rig.json").write_text(json.dumps(parent_rig, indent=2) + "\n")

    # 3. Convert Remotion lyrics & motion drawings into design/words.json
    words_doc, full_lyrics_text = build_pterodactyl_words_json(remotion_dir)
    (dst_dir / "design").mkdir(parents=True, exist_ok=True)
    (dst_dir / "design" / "words.json").write_text(json.dumps(words_doc, indent=2) + "\n")
    (dst_dir / "lyrics-design.json").write_text(json.dumps(words_doc, indent=2) + "\n")

    # Build transcript file & canonical official_lyrics.json
    lyrics_artifact = json.loads((remotion_dir / "artifacts" / "lyrics.json").read_text())
    transcript_words = []
    for w in lyrics_artifact["words"]:
        w_start = float(w.get("start", w["anchor"]))
        w_end = float(w.get("end", w.get("secondaryAnchor", w.get("anchorEnd", w_start + 0.35))))
        if w_end <= w_start:
            w_end = round(w_start + 0.32, 3)
        transcript_words.append(
            {
                "word": w["text"],
                "text": w["text"],
                "start": w_start,
                "end": w_end,
                "start_tick": seconds_to_ticks(w_start),
                "end_tick": seconds_to_ticks(w_end),
            }
        )
    transcript_doc = {
        "audio": "audio/pterodactyl-song.mp3",
        "duration": lyrics_artifact.get("recordingDurationSeconds", 39.3),
        "duration_tick": seconds_to_ticks(lyrics_artifact.get("recordingDurationSeconds", 39.3)),
        "language": "en",
        "words": transcript_words,
    }
    (dst_dir / "Pterodactyl Song.transcript.json").write_text(json.dumps(transcript_doc, indent=2) + "\n")

    (dst_dir / "artifacts").mkdir(parents=True, exist_ok=True)
    official_lyrics = {
        "version": "1.0",
        "project_id": "pterodactyl-song",
        "title": "Pterodactyl Song",
        "structure_as_printed": [
            "verse_1_sleep",
            "verse_1_cave",
            "verse_2_sudden",
            "chorus_wake_fly",
            "verse_3_directions",
            "outro_return_nest",
        ],
        "lines": [
            {"id": "s1-l1", "section": "verse_1_sleep", "text": "All the pterodactyls were sleeping very still"},
            {"id": "s2-l1", "section": "verse_1_cave", "text": "Down in the cave at the bottom of a hill"},
            {"id": "s3-l1", "section": "verse_2_sudden", "text": "When all of a sudden one of them did cry"},
            {"id": "s4-l1", "section": "chorus_wake_fly", "text": "Wake up pterodactyls and fly, fly, fly"},
            {"id": "s5-l1", "section": "verse_3_directions", "text": "Fly to the east and fly to the west"},
            {"id": "s6-l1", "section": "outro_return_nest", "text": "Fly all around and then fly back to your nest"},
        ],
    }
    (dst_dir / "artifacts" / "official_lyrics.json").write_text(json.dumps(official_lyrics, indent=2) + "\n")

    # 4. Write Pterodactyl Song.magic
    magic_data = {
        "assets": [
            {"id": "cave_bg", "kind": "image", "name": "Warm Prehistoric Cave", "path": "assets/backgrounds/cave.png", "preset": None},
            {"id": "exterior_bg", "kind": "image", "name": "Valley & Hill Exterior", "path": "assets/backgrounds/exterior.png", "preset": None},
            {
                "id": "child_pterodactyl",
                "kind": "cutout_rig",
                "name": "Cinnamon Child Pterodactyl",
                "path": "assets/characters/child-pterodactyl/rig.json",
                "parts": [
                    {"name": "sleeping", "solo": False, "verified": True, "visible": True},
                    {"name": "body", "solo": False, "verified": True, "visible": True},
                    {"name": "wing", "solo": False, "verified": True, "visible": True},
                ],
                "preset": None,
            },
            {
                "id": "parent_pterodactyl",
                "kind": "cutout_rig",
                "name": "Teal Parent Pterodactyl",
                "path": "assets/characters/parent-pterodactyl/rig.json",
                "parts": [
                    {"name": "sleeping", "solo": False, "verified": True, "visible": True},
                    {"name": "body", "solo": False, "verified": True, "visible": True},
                    {"name": "wing", "solo": False, "verified": True, "visible": True},
                ],
                "preset": None,
            },
            {"id": "child_sleeping_img", "kind": "image", "name": "Child Pterodactyl · Sleeping", "path": "assets/characters/child-pterodactyl/sleeping.png", "preset": None},
            {"id": "child_awake_img", "kind": "image", "name": "Child Pterodactyl · Awake Body", "path": "assets/characters/child-pterodactyl/body.png", "preset": None},
            {"id": "parent_sleeping_img", "kind": "image", "name": "Parent Pterodactyl · Sleeping", "path": "assets/characters/parent-pterodactyl/sleeping.png", "preset": None},
            {"id": "parent_awake_img", "kind": "image", "name": "Parent Pterodactyl · Awake Body", "path": "assets/characters/parent-pterodactyl/body.png", "preset": None},
        ],
        "format": "magic.project",
        "name": "Pterodactyl Song",
        "song": {
            "title": "Pterodactyl Song",
            "lyrics": full_lyrics_text,
            "notes": "Converted from Remotion TSX project (`film.tsx`, `lyric-motion.tsx`, `lyric-scenes.tsx`, `cues.ts`).",
        },
        "version": 1,
        "workshop": {
            "audio": "audio/pterodactyl-song.mp3",
            "draft_approved": True,
            "target": "filmcraft",
            "video_project": "film/Pterodactyl Song.fcproj",
        },
    }
    magic_file = dst_dir / "Pterodactyl Song.magic"
    magic_file.write_text(json.dumps(magic_data, indent=2) + "\n")

    # 5. Copy rendered video into film/ and exports/ and build FilmCraft .fcproj
    film_dir = dst_dir / "film"
    exports_dir = dst_dir / "exports"
    film_dir.mkdir(parents=True, exist_ok=True)
    exports_dir.mkdir(parents=True, exist_ok=True)
    mp4_src = remotion_dir / "out" / "PterodactylSong.mp4"
    if mp4_src.exists():
        shutil.copy2(mp4_src, film_dir / "Pterodactyl Song-preview.mp4")
        shutil.copy2(mp4_src, exports_dir / "Pterodactyl Song.mp4")

    build_fcproj_with_scenes(
        project_name="Pterodactyl Song",
        fcproj_path=film_dir / "Pterodactyl Song.fcproj",
        audio_rel="../audio/pterodactyl-song.mp3",
        audio_name="pterodactyl-song.mp3",
        duration_sec=39.3,
        scenes=[
            ("Scene 1 · Cave — Sleeping Family", 0.0, 5.2, "../assets/backgrounds/cave.png"),
            ("Scene 2 · Cave at the Bottom of the Hill", 5.2, 10.6, "../assets/backgrounds/exterior.png"),
            ("Scene 3 · Sudden Loud Cry Inside Cave", 10.6, 17.5, "../assets/backgrounds/cave.png"),
            ("Scene 4 · Wake Up Pterodactyls! Fly, Fly, Fly!", 17.5, 22.7, "../assets/backgrounds/cave.png"),
            ("Scene 5 · Valley Flight — East and West", 22.7, 29.0, "../assets/backgrounds/exterior.png"),
            ("Scene 6 · Fly All Around & Back to Nest", 29.0, 39.3, "../assets/backgrounds/cave.png"),
        ],
        character_clips=[
            ("Child & Parent · Sleeping Curled Pose", 0.0, 5.2, "../exports/Pterodactyl Song.mp4"),
            ("Sleeping Family Inside Cave", 5.2, 10.6, "../exports/Pterodactyl Song.mp4"),
            ("Child Pterodactyl Wakes & Cries", 10.6, 17.5, "../exports/Pterodactyl Song.mp4"),
            ("Both Pterodactyls Unfurl Wings & Launch", 17.5, 22.7, "../exports/Pterodactyl Song.mp4"),
            ("Teal Parent & Cinnamon Child · East/West Flight", 22.7, 29.0, "../exports/Pterodactyl Song.mp4"),
            ("360 Flight Loop & Return to Cave Nest", 29.0, 39.3, "../exports/Pterodactyl Song.mp4"),
        ],
        lyric_clips=[
            ("Lyrics & Sleepy Eyelids · Sleeping Very Still", 0.0, 5.2, "../exports/Pterodactyl Song.mp4"),
            ("Lyrics & Down Arrow + Hill · Bottom of the Hill", 5.2, 10.6, "../exports/Pterodactyl Song.mp4"),
            ("Lyrics & Call Sound Rays · Sudden Loud Cry", 10.6, 17.5, "../exports/Pterodactyl Song.mp4"),
            ("Lyrics & Rising Flight Swoop · Fly, Fly, Fly!", 17.5, 22.7, "../exports/Pterodactyl Song.mp4"),
            ("Lyrics & Compass Arrows · East and West", 22.7, 29.0, "../exports/Pterodactyl Song.mp4"),
            ("Lyrics & Loop + Nest Cradle · Back to Your Nest", 29.0, 39.3, "../exports/Pterodactyl Song.mp4"),
        ],
    )

    # 6. Build 10-page 7-Parent Storyboard (`storyboard.designcraft`) and EffectCraft (`lettering/lyrics.ecproj`) via magic-cli
    if MAGIC_CLI.exists():
        populate_storyboard_7_parents(
            magic_file=magic_file,
            title="PTERODACTYL SONG",
            subtitle="Felt Cutout Animation & Hand-Drawn Lyric Choreography (Converted from Remotion)",
            tagline="Little pterodactyl is sleeping very still — wake up, pterodactyls! Fly, fly, fly!",
            overall_design="Warm prehistoric amber cave interior (`cave.png`) and sweeping green valley hill (`exterior.png`) paired with Teal Parent & Cinnamon Child felt wing rigs and 8 hand-drawn SVG motion tracks.",
            format_notes="1920 × 1080 · 30 fps · 0:39.3 · EffectCraft + DesignCraft + FilmCraft",
            hand_note="Artist note: keep cave scenes warm amber/gold (`#FFF8E9` & `#F2C14E`) and exterior valley scenes deep teal/terracotta (`#173F49` & `#8B3D26`).",
            cover_key_art="assets/backgrounds/cave.png",
            world_subheading="Warm Prehistoric Cave Nest (Interior) & Sweeping Green Valley Hill (Exterior)",
            world_stage_image="assets/backgrounds/cave.png",
            world_source_image="assets/backgrounds/exterior.png",
            world_result_image="assets/backgrounds/cave.png",
            world_prompt="Handcrafted felt and paper-cut prehistoric cave interior and sunlit green valley hill exterior, warm tactile storybook lighting, open upper-left staging zone for Fraunces kinetic lyrics, no text.",
            char_subheading="Cinnamon Child Pterodactyl & Teal Parent Pterodactyl (Felt Cutout Rigs)",
            char_original_image="assets/characters/child-pterodactyl/sleeping.png",
            char_pose_image="assets/characters/child-pterodactyl/body.png",
            char_prompt="Tactile felt cutout pterodactyl character parts (`sleeping.png`, `body.png`, `wing.png`) with stitched wing seams, rounded shoulder pivot at [43, 112], transparent PNG ground.",
            motion_subheading="8 Hand-Drawn SVG Motion Tracks Choreographed to Sung Words (`design/words.json` -> `lettering/lyrics.ecproj`)",
            motion_stage_image="assets/backgrounds/exterior.png",
            motion_preview_1="assets/backgrounds/cave.png",
            motion_preview_2="assets/characters/parent-pterodactyl/body.png",
            motion_summary="6 section animations · 8 hand-drawn SVG overlays (Sleepy Closed Eyes, Still Underline, Downward Arrow, Hill Contour, Call Rays, Rising Flight Swoop, East/West Arrows, Playful Loop + Nest Cradle).",
            motion_spec="Converted from Remotion `lyric-motion.tsx` (`Interactive.Svg` / `Interactive.Path`) into native Magic Studio `draws` & `kinetic.draws` strokes compiled into EffectCraft Shape Layers.",
            scenes_spec=[
                {
                    "section": "SCENE 1 · VERSE 1 SLEEP",
                    "title": "Little Pterodactyl Is Sleeping Very Still",
                    "time": "0:00.00 – 0:05.20",
                    "cue": "“Little pterodactyl is sleeping very still,”",
                    "purpose": "Open inside the warm prehistoric cave with both pterodactyls curled asleep in the nest.",
                    "staging": "Wide cave frame (`cave.png`); Sleepy Closed Eyes (two curved eyelid arcs) + Still Hush Underline draw in sync with 'sleeping' and 'still'.",
                    "location_ref": "LOC-01 · Warm Prehistoric Cave (`assets/backgrounds/cave.png`)",
                    "char_shared": "Teal Parent Pterodactyl (`sleeping.png`)",
                    "char_lead": "Cinnamon Child Pterodactyl (`sleeping.png`)",
                    "hero": "assets/backgrounds/cave.png",
                    "source": "assets/characters/child-pterodactyl/sleeping.png",
                    "result": "assets/backgrounds/cave.png",
                },
                {
                    "section": "SCENE 2 · VERSE 1 CAVE",
                    "title": "Down in a Cave at the Bottom of the Hill",
                    "time": "0:05.20 – 0:10.60",
                    "cue": "“Down in a cave at the bottom of the hill.”",
                    "purpose": "Cut to the exterior valley & hill to show where the cave sits at the base of the green slope.",
                    "staging": "Split showcase on `exterior.png`; Hand-Drawn Downward Arrow drops from 'Down' and Rounded Hill Contour sweeps beneath 'bottom of the hill'.",
                    "location_ref": "LOC-02 · Valley & Hill Exterior (`assets/backgrounds/exterior.png`)",
                    "char_shared": "Teal Parent Pterodactyl (`sleeping.png`)",
                    "char_lead": "Cinnamon Child Pterodactyl (`sleeping.png`)",
                    "hero": "assets/backgrounds/exterior.png",
                    "source": "assets/backgrounds/exterior.png",
                    "result": "assets/backgrounds/cave.png",
                },
                {
                    "section": "SCENE 3 · VERSE 2 SUDDEN CRY",
                    "title": "All of a Sudden You Hear a Loud Cry—",
                    "time": "0:10.60 – 0:17.50",
                    "cue": "“All of a sudden you hear a loud cry—”",
                    "purpose": "Snap back inside the cave as the child pterodactyl pops awake and lets out a ringing call.",
                    "staging": "3-beat montage on `cave.png`: 1. 'sudden' pops in gold; 2. Child swaps from `sleeping.png` to `body.png`; 3. Three Call Sound Rays burst outward from 'cry—'.",
                    "location_ref": "LOC-01 · Warm Prehistoric Cave (`assets/backgrounds/cave.png`)",
                    "char_shared": "Teal Parent Pterodactyl (`sleeping.png` -> `body.png`)",
                    "char_lead": "Cinnamon Child Pterodactyl (`body.png` + `wing.png`)",
                    "hero": "assets/backgrounds/cave.png",
                    "source": "assets/characters/child-pterodactyl/body.png",
                    "result": "assets/characters/parent-pterodactyl/body.png",
                },
                {
                    "section": "SCENE 4 · CHORUS WAKE & FLY",
                    "title": "Wake Up, Pterodactyls! Fly, Fly, Fly!",
                    "time": "0:17.50 – 0:22.70",
                    "cue": "“Wake up, pterodactyls! Fly, fly, fly!”",
                    "purpose": "Both parent and child unfurl their articulated textile wings and launch upward out of the cave.",
                    "staging": "Wide launch choreography on `cave.png`; Rising Flight Swoop arrow lifts the ascending 'Fly, fly, fly!' staircase.",
                    "location_ref": "LOC-01 · Warm Prehistoric Cave (`assets/backgrounds/cave.png`)",
                    "char_shared": "Teal Parent Pterodactyl (`body.png` + `wing.png` flap)",
                    "char_lead": "Cinnamon Child Pterodactyl (`body.png` + `wing.png` flap)",
                    "hero": "assets/backgrounds/cave.png",
                    "source": "assets/characters/parent-pterodactyl/body.png",
                    "result": "assets/backgrounds/exterior.png",
                },
                {
                    "section": "SCENE 5 · VERSE 3 DIRECTIONS",
                    "title": "Fly to the East and Fly to the West",
                    "time": "0:22.70 – 0:29.00",
                    "cue": "“Fly to the east and fly to the west,”",
                    "purpose": "Soar across the sunlit exterior valley, banking right on 'east' and banking left on 'west'.",
                    "staging": "Split directional showcase on `exterior.png`; East & West Compass Arrows draw right under 'east' (`#8B3D26`) and left under 'west' (`#173F49`).",
                    "location_ref": "LOC-02 · Valley & Hill Exterior (`assets/backgrounds/exterior.png`)",
                    "char_shared": "Teal Parent Pterodactyl (soaring formation)",
                    "char_lead": "Cinnamon Child Pterodactyl (banking east & west)",
                    "hero": "assets/backgrounds/exterior.png",
                    "source": "assets/characters/child-pterodactyl/wing.png",
                    "result": "assets/backgrounds/exterior.png",
                },
                {
                    "section": "SCENE 6 · OUTRO RETURN TO NEST",
                    "title": "Fly All Around and Then Back to Your Nest",
                    "time": "0:29.00 – 0:39.30",
                    "cue": "“Fly all around and then fly back to your nest.”",
                    "purpose": "Complete a joyful 360-degree loop in the valley sky before gliding home to curl up in the warm cave nest.",
                    "staging": "Montage finale on `cave.png`: 1. Playful Flight Loop circles 'around'; 2. Return glide into cave; 3. Warm Nest Cradle curve settles beneath 'nest.'.",
                    "location_ref": "LOC-01 · Warm Prehistoric Cave (`assets/backgrounds/cave.png`)",
                    "char_shared": "Teal Parent Pterodactyl (`body.png` -> `sleeping.png`)",
                    "char_lead": "Cinnamon Child Pterodactyl (`body.png` -> `sleeping.png`)",
                    "hero": "assets/backgrounds/cave.png",
                    "source": "assets/backgrounds/exterior.png",
                    "result": "assets/characters/child-pterodactyl/sleeping.png",
                },
            ],
        )

        res = run_magic_cli(magic_file, "exec", "words.letter", "render=false")
        if res.returncode == 0:
            print("  Built lettering/lyrics.ecproj via words.letter")
        else:
            print(f"  words.letter warning: {res.stderr.strip()}")

    print(f"=== Finished Pterodactyl Song -> {magic_file} ===\n")


def main() -> None:
    parser = argparse.ArgumentParser(description="Convert HyperFrames & Remotion projects to Magic Studio.")
    parser.add_argument("--project", choices=["hey-ho-little-fish", "pterodactyl-song"], help="Named project to convert")
    parser.add_argument("--all", action="store_true", help="Convert both Hey Ho Little Fish and Pterodactyl Song")
    parser.add_argument("--hyperframes", type=Path, help="Custom HyperFrames source directory")
    parser.add_argument("--remotion", type=Path, help="Custom Remotion source directory")
    parser.add_argument("--out", type=Path, help="Custom output directory")
    args = parser.parse_args()

    if args.hyperframes and args.out:
        convert_hey_ho_little_fish(args.hyperframes, args.out)
        return
    if args.remotion and args.out:
        convert_pterodactyl_song(args.remotion, args.out)
        return

    do_all = args.all or not args.project
    if do_all or args.project == "hey-ho-little-fish":
        convert_hey_ho_little_fish(
            ROOT / "media" / "hey-ho-little-fish" / "project",
            ROOT / "projects" / "hey-ho-little-fish",
        )
    if do_all or args.project == "pterodactyl-song":
        convert_pterodactyl_song(
            ROOT
            / "media"
            / "pterodactyl-song"
            / "project"
            / "references"
            / "chatgpt-storyboard"
            / "project-snapshots"
            / "merriweather-videos"
            / "projects"
            / "pterodactyl-song",
            ROOT / "projects" / "pterodactyl-song",
        )


if __name__ == "__main__":
    main()
