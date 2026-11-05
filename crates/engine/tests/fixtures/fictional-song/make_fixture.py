#!/usr/bin/env python3
"""Regenerate the fictional-song fixture.

The fixture is a made-up song with the same shape as a real project: a canonical
lyrics sheet (the printed words) and a transcript of a recording (the same words,
sung through the song's as-sung structure). Nothing here is a real song or a real
recording.

Writes:
  fictional-song.lyrics.json   the canonical sheet
  fictional-song.transcript.json  the recording's timed paragraphs and words
  build-storyboard.txt          the script `magic-cli storyboard run` applies
"""
import json
import pathlib

HERE = pathlib.Path(__file__).parent

TITLE = "The Lighthouse at Kettleness Bay"
ARTIST = "Fictional Folk — test fixture"
ALBUM = "Test Album (no such record)"

# The printed words: verse_1, verse_2, chorus. One line each, in order.
LINES = [
    ("v1-l1", "verse_1", "The bay is wide and the water is grey,"),
    ("v1-l2", "verse_1", "and the boats are small and far away."),
    ("v1-l3", "verse_1", "I climb the cliff with the wind in my coat,"),
    ("v1-l4", "verse_1", "counting the steps I have walked by the note."),
    ("v2-l1", "verse_2", "The harbour hums with an iron chain,"),
    ("v2-l2", "verse_2", "and the gulls cry out the weather's refrain."),
    ("v2-l3", "verse_2", "The storm comes up like a wall of glass,"),
    ("v2-l4", "verse_2", "and every lamp in the town goes dark as it passes."),
    ("ch-l1", "chorus", "And I light the lantern, I light it again,"),
    ("ch-l2", "chorus", "I light it for the boats and for the rain."),
    ("ch-l3", "chorus", "Turn my face to the weather, turn my face to the sea,"),
    ("ch-l4", "chorus", "keep a light on the headland and a light on me."),
]

PRINTED = ["verse_1", "verse_2", "chorus"]
# The recording sings the chorus twice. The storyboard covers the printed
# structure once through; the animatic replays the chorus because the record does.
AS_SUNG = ["verse_1", "verse_2", "chorus", "chorus"]

# A line's place in the song: (start, end, seconds) in the first pass.
TIMES = [
    (2.20, 4.10),
    (4.60, 6.40),
    (7.00, 9.20),
    (9.70, 11.60),
    (12.20, 14.00),
    (14.50, 16.40),
    (17.00, 19.30),
    (19.80, 21.60),
    (22.20, 24.00),
    (24.40, 26.20),
    (26.80, 28.90),
    (29.40, 31.20),
]

# Where each scene happens, and what it shows. One scene per place.
SCENES = [
    {"id": "S01", "title": "Kettleness Bay", "section": "The wide water",
     "lines": (0, 2), "location": "Kettleness Bay", "art": "scene-01-bay.png",
     "purpose": "The song opens on the water: wide, grey, and too far to help.",
     "staging": ["the bay fills the frame", "boats small and far off",
                  "no lamp lit yet"]},
    {"id": "S02", "title": "The Cliff Path", "section": "Counting the steps",
     "lines": (2, 4), "location": "The Cliff Path", "art": "scene-02-cliff.png",
     "purpose": "The keeper climbs, counting: the path is the song's measure.",
     "staging": ["the path climbs left to right", "wind in the coat",
                  "the bay behind, smaller now"]},
    {"id": "S03", "title": "The Harbour", "section": "Iron chain",
     "lines": (4, 6), "location": "The Harbour", "art": "scene-03-harbour.png",
     "purpose": "The harbour answers the bay, with iron and gulls.",
     "staging": ["chain in the foreground", "gulls crossing",
                  "lamps still unlit"]},
    {"id": "S04", "title": "The Storm", "section": "A wall of glass",
     "lines": (6, 8), "location": "The Storm", "art": "scene-04-storm.png",
     "purpose": "The weather takes the town; every lamp goes out.",
     "staging": ["the storm edge on the right", "lamps going dark one by one",
                  "no light anywhere yet"]},
    {"id": "S05", "title": "The Lantern Room", "section": "Light it again",
     "lines": (8, 12), "location": "The Lantern Room", "art": "scene-05-lantern.png",
     "purpose": "The keeper lights the lantern for the boats: the song's turn.",
     "staging": ["the lantern beam turning", "the glass around it",
                  "the first light in the film"]},
]


def sheet():
    return {
        "version": "1.0",
        "title": TITLE,
        "album": ALBUM,
        "artist": ARTIST,
        "source": {
            "path": str(HERE / "fictional-song.lyrics.json"),
            "description": "Fictional test fixture; no such song exists",
        },
        "artist_note_verbatim": "A fictional song written to exercise the storyboard tooling.",
        "actions_as_printed": {
            "verse_1": "wide water; climb the path",
            "verse_2": "harbour; storm",
            "chorus": "light the lantern",
        },
        "structure_as_printed": PRINTED,
        "lines": [
            {"id": lid, "section": section, "text": text}
            for lid, section, text in LINES
        ],
        "structure_as_sung": AS_SUNG,
        "structure_as_sung_source": (
            "Fictional: the chorus is sung twice in the recording."
        ),
    }


def sections_in_order():
    """Each line's index within its section, in printed order."""
    order = {}
    for i, (_, section, _) in enumerate(LINES):
        order.setdefault(section, []).append(i)
    return order


def transcript():
    """The recording: every printed line, sung through the as-sung structure."""
    order = sections_in_order()
    paragraphs = []
    words = []
    wid = 0
    # A gap between the first pass and the repeat, as a song would have.
    offset = 0.0
    for pass_no, section in enumerate(AS_SUNG):
        for i in order[section]:
            start, end = TIMES[i]
            shift = 0.0 if pass_no == 0 else offset
            text = LINES[i][2]
            paragraphs.append({"start": round(start + shift, 2), "end": round(end + shift, 2),
                               "text": text})
            # Words, spread evenly across the line.
            parts = text.split()
            span = (end - start) / max(len(parts), 1)
            for k, part in enumerate(parts):
                words.append({
                    "id": f"w{wid}",
                    "start": round(start + shift + k * span, 2),
                    "end": round(start + shift + (k + 1) * span, 2),
                    "text": part,
                    "timing_method": "fixture: even spread across the line",
                })
                wid += 1
        if pass_no == 0:
            offset = 41.0 - TIMES[0][0]
    return {"paragraphs": paragraphs, "words": words}


def build_script():
    """The script `magic-cli storyboard run` applies.

    The lyrics are not typed here: `storyboard.lyrics` inserts them from the canonical sheet
    and the transcript, and locks them. The sheet and the transcript are read-only sources and
    this script never writes to them.

    Nothing is hardcoded to a page index: the first scene page is found through
    `storyboard.pages`, and each added page is found through the reference to the
    `storyboard.add` that created it.
    """
    out = [
        "# The fictional song as a storyboard: one scene page per place.",
        "# Written by make_fixture.py; run with:",
        "#   magic-cli storyboard --document <file> run build-storyboard.txt",
        "storyboard.pages {}",
    ]
    # The template has one scene page; the rest are added.
    for _ in SCENES[1:]:
        out.append('storyboard.add {"archetype": "scene"}')
    # The lyrics come from the source, locked.
    out.append(
        "storyboard.lyrics " + json.dumps({
            "lyrics": str(HERE / "fictional-song.lyrics.json"),
            "transcript": str(HERE / "fictional-song.transcript.json"),
            "per": 2,
        })
    )
    # The creative fields, per scene page.
    step = 1 + len(SCENES) - 1  # the adds, then the lyrics
    for n, scene in enumerate(SCENES):
        fields = {
            "scene_id": "SCENE %02d" % (n + 1),
            "title": scene["title"],
            "section": scene["section"],
            "purpose": scene["purpose"],
            "staging_notes": "\n".join(scene["staging"]),
            "location_ref": f"{scene['location']} — original plate",
            "revision": "REV 01",
            "hand_note": "action: " + scene["section"].lower(),
        }
        images = {"hero_image": str(HERE / "art" / scene["art"])}
        page = "$0.pages.3.page" if n == 0 else "$%d.page" % n
        out.append(
            "storyboard.set "
            + json.dumps({"page": page, "fields": fields, "images": images},
                         ensure_ascii=False)
        )
    # The book's own pages: cover, look development, character study.
    out.append(
        "storyboard.set "
        + json.dumps({
            "page": "$0.pages.0.page",
            "fields": {
                "cover_kicker": "TEST RECORD / STORYBOARD",
                "cover_title": TITLE,
                "cover_subtitle": "a fictional song · an animatic",
                "cover_badge": "PRODUCTION BOOK",
                "cover_tagline": "A made-up song, to prove the storyboard is not one song's.",
                "cover_hand_note": "Kathy Reid-Naiman's folk style, imagined.",
                "cover_overall_design": "Two-colour linocut: navy and ochre on cream paper.",
                "cover_format_notes": "Short film (~40 s), 16:9, 1920x1080.",
                "cover_key_art_caption": "key art · the headland and the bay",
                "cover_revision": "REV 01",
                "cover_footer_note": "Every reference keeps its source and its prompt.",
            },
            "images": {"cover_key_art": str(HERE / "art" / "cover-keyart.png")},
        }, ensure_ascii=False)
    )
    out.append(
        "storyboard.set "
        + json.dumps({
            "page": "$0.pages.1.page",
            "fields": {
                "world_running_title": "TEST RECORD / STORYBOARD",
                "world_heading": "THE HEADLAND / L01",
                "world_subheading": "One bay, one tower, one light",
                "world_badge": "LOOK DEVELOPMENT",
                "world_stage_caption": "Kettleness Bay / original source",
                "world_material_scale": "Two inks on cream: deep navy, warm ochre. "
                                        "Carved gouge lines, flat shapes, no gradients.",
                "world_hand_note": "Keep the approved source with its prompt.",
                "world_register": "L01  Kettleness Bay\nSource: stage-kettleness-bay.png\n"
                                  "Original: 1672 x 941 px",
                "world_location_prompt": "Two-colour linocut of a lighthouse on a bare headland "
                                         "at the edge of a wide bay, seen from the water; low "
                                         "horizon; heavy carved sea; one small fishing boat; a "
                                         "storm front entering from the right as dense hatching.",
                "world_revision": "v1 → keeper added to the headland; the stage unchanged.",
                "world_source_image_caption": "SOURCE : stage-kettleness-bay",
                "world_result_image_caption": "RESULT : the lantern lit",
                "world_match_note": "Same framing, same two inks; the keeper is the only change.",
                "world_fit_note": "Fit proportionally. Approve any crop. Never stretch.",
            },
            "images": {
                "world_stage_image": str(HERE / "art" / "stage-kettleness-bay.png"),
                "world_source_image": str(HERE / "art" / "stage-kettleness-bay.png"),
                "world_result_image": str(HERE / "art" / "scene-05-lantern.png"),
            },
        }, ensure_ascii=False)
    )
    out.append(
        "storyboard.set "
        + json.dumps({
            "page": "$0.pages.2.page",
            "fields": {
                "char_running_title": "TEST RECORD / STORYBOARD",
                "char_heading": "THE KEEPER / C01",
                "char_subheading": "Original to animation pose",
                "char_badge": "CHARACTER STUDY",
                "char_character_tag": "C01 · THE KEEPER",
                "char_original_image_caption": "original drawing",
                "char_pose_image_caption": "saved result",
                "char_hand_note": "Same design. Clear limbs. Room to move.",
                "char_silhouette_rules": "SILHOUETTE RULES\nlimbs fully visible\n"
                                         "no limb overlaps another\nclear gap between legs\n"
                                         "clear gap beside each arm\nnothing touches canvas edge",
                "char_exact_prompt": "Two-colour linocut of a lighthouse keeper, full body, front "
                                     "view, standing, animation-ready stance, on a cream ground.",
                "char_prompt_foot": "Fictional test fixture",
                "char_asset_geometry": "2:3 canvas · full body · feet on a baseline\n"
                                       "Clean silhouette: limbs separated, no overlaps.",
            },
            "images": {
                "char_original_image": str(HERE / "art" / "keeper-original.png"),
                "char_pose_image": str(HERE / "art" / "keeper-pose.png"),
            },
        }, ensure_ascii=False)
    )
    return "\n".join(out) + "\n"


def main():
    (HERE / "fictional-song.lyrics.json").write_text(
        json.dumps(sheet(), indent=2, ensure_ascii=False) + "\n")
    (HERE / "fictional-song.transcript.json").write_text(
        json.dumps(transcript(), indent=2, ensure_ascii=False) + "\n")
    (HERE / "build-storyboard.txt").write_text(build_script())
    print("wrote lyrics sheet, transcript and build script to", HERE)


if __name__ == "__main__":
    main()
