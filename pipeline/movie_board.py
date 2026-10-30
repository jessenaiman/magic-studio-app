#!/usr/bin/env python3
"""Build the editable Magic Studio storyboard from its single DeckCraft template."""

import argparse, json, math, shutil, subprocess, shlex, re, hashlib, sys
from pathlib import Path

_REPO_ROOT = str(Path(__file__).resolve().parents[1])
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)


def canonical_scenes(scene_map):
    doc = json.loads(Path(scene_map).read_text())
    scenes = []
    previous = 0.0
    targets = set()
    for packet in doc.get("scenes", []):
        start, end = packet["start"], packet["end"]
        if not (
            math.isfinite(start)
            and math.isfinite(end)
            and 0 <= start < end
            and start >= previous - 0.001
        ):
            raise ValueError(
                "Canonical scenes need finite ordered non-overlapping recording times"
            )
        target = packet["target"]
        if target in targets:
            raise ValueError("Duplicate canonical scene target: " + target)
        targets.add(target)
        previous = end
        # Keep each mapped occurrence/correction; do not independently regroup caption paragraphs.
        scenes.append(
            dict(
                packet,
                id=target,
                start_seconds=start,
                end_seconds=end,
                lyric_text=" ".join(
                    str(line.get("lyric") if "lyric" in line else line.get("text", ""))
                    for line in packet.get("lines", [])
                ),
            )
        )
    if not scenes:
        raise ValueError("No canonical scenes; run storyboard.init first")
    return scenes


def normalize_scene_content(packets, expected_fields):
    """Map existing labelled source text; preserve unknowns and full native slide notes."""
    aliases = {
        "reference_prompt": ("reference_prompt", "image_prompt"),
        "animation_prompt": ("animation_prompt",),
        "layer_stack": ("layer_stack", "continuity", "linked_assets"),
        "action": ("action", "action_staging", "staging"),
        "animated_text": ("animated_text",),
        "drawing_cues": ("drawing_cues", "drawing", "teaching_cues"),
    }
    targets = {
        "reference_prompt": "image_prompt",
        "layer_stack": "continuity",
        "drawing_cues": "drawing",
    }
    notes_only = {
        "skills",
        "build_steps",
        "acceptance_checks",
        "plan_vs_actual",
        "visual_review",
    }
    report = dict(
        expected_fields=list(expected_fields),
        mapped=[],
        missing=[],
        unmapped=[],
        retained_in_slide_notes=[],
        fit_review=[],
    )
    content = {}
    for packet in packets:
        source = packet.get("sections") or {}
        sections = {}
        used = set()
        normalized = {
            re.sub(r"[^a-z0-9]+", "_", k.lower()).strip("_"): (k, v)
            for k, v in source.items()
        }
        for field, names in aliases.items():
            role = "user.scene." + targets.get(field, field)
            if role not in expected_fields:
                continue
            found = next(
                (normalized[n] for n in names if n in normalized and normalized[n][1]),
                None,
            )
            if found:
                label, text = found
                used.add(label)
            elif field == "action":
                plans = [
                    n[len("Scene plan: ") :]
                    for n in packet.get("notes", [])
                    if n.startswith("Scene plan: ")
                ]
                label = "Notes / Scene plan"
                text = "\n".join(plans)
            else:
                label = field
                text = ""
            if text:
                sections[field] = text
                report["mapped"].append(
                    dict(scene=packet["id"], source=label, target=role)
                )
                if len(text) > 220:
                    report["fit_review"].append(
                        dict(
                            scene=packet["id"],
                            field=role,
                            characters=len(text),
                            reason="Full source wording retained; review native shrink-to-fit at viewing size",
                        )
                    )
            else:
                report["missing"].append(dict(scene=packet["id"], field=role))
        for label, text in source.items():
            if label in used:
                continue
            key = re.sub(r"[^a-z0-9]+", "_", label.lower()).strip("_")
            item = dict(scene=packet["id"], source=label, text=text)
            report["retained_in_slide_notes"].append(
                dict(scene=packet["id"], source=label, characters=len(text))
            ) if key in notes_only else report["unmapped"].append(item)
        content[packet["id"]] = dict(
            sections=sections,
            notes=packet.get("notes", []),
            source=packet.get("source"),
            source_sections=source,
            lines=packet.get("lines", []),
            title=packet.get("title"),
        )
    report["needs_correction"] = bool(report["unmapped"])
    return content, report


def template_schema(template, out):
    from storyboard_layout import Deck, inspect_pages, note

    d = Deck(template.parent, out)
    try:
        d.cmd("file.open", path=str(template))
        return {
            note(sl)["kind"]: [
                dict(role=sh["altText"], rect=sh["box"])
                for sh in sl["shapes"]
                if sh.get("altText", "").startswith("user.")
            ]
            for sl in inspect_pages(d)
        }
    finally:
        d.close()


def align_imported_sections(packets, words, source, evidence=None):
    """Repair cuts through an exact known lyric line; never estimate word times."""
    if not source:
        return []
    root = Path(source)
    plan = root / "artifacts/scene_plan.json"
    script = root / "artifacts/script.json"
    if not plan.is_file() or not script.is_file():
        return []
    scenes = json.loads(plan.read_text()).get("scenes", [])
    sections = {s["id"]: s for s in json.loads(script.read_text()).get("sections", [])}
    line_source = root / "hyperframes/film-data.js"
    linked = {}
    if line_source.is_file():
        raw = line_source.read_text().strip()
        if raw.startswith("window.FISH_DATA = "):
            linked = {
                l["id"]: l
                for l in json.loads(raw.split("=", 1)[1].strip().removesuffix(";")).get(
                    "lyrics", []
                )
            }
    if len(scenes) != len(packets):
        if evidence is not None:
            evidence.append(
                dict(
                    status="unassessed",
                    reason="Supplied scene plan and storyboard scene counts differ",
                )
            )
        return []
    token = lambda text: re.sub(r"[^\w]+", "", text.casefold())
    tokens = [token(w["text"]) for w in words]
    changes = []
    for i, (packet, original) in enumerate(zip(packets, scenes)):
        lyrics = [
            sections[k]
            for k in original.get("script_section_id", "").split(",")
            if k in sections
            and sections[k].get("lyrics", True)
            and sections[k].get("text") != "[instrumental]"
        ]
        if not lyrics:
            if evidence is not None and i:
                evidence.append(
                    dict(
                        scene_id=packet["id"],
                        status="unassessed",
                        reason="No supplied lyric section opening",
                    )
                )
            continue
        packet["source_lines"] = [
            dict(
                id=l["id"],
                text=linked.get(l["id"], l)["text"],
                source=str(line_source if l["id"] in linked else script),
            )
            for l in lyrics
        ]
        label = lyrics[0].get("label", "").split(" line")[0]
        if label.lower().startswith("chorus"):
            occurrence = re.match(r"ch(\d+)", lyrics[0]["id"])
            packet["kind"] = "chorus"
            packet["section_label"] = "Chorus" + (
                " " + occurrence[1] if occurrence else ""
            )
        elif label.lower().startswith("verse"):
            packet["kind"] = "verse"
            packet["section_label"] = label
        if not i:
            continue
        expected = [token(t) for t in lyrics[0]["text"].split()]
        hits = [
            n
            for n in range(len(words) - len(expected) + 1)
            if tokens[n : n + len(expected)] == expected
            and packets[i - 1]["start_seconds"]
            <= words[n]["start"]
            < packet["end_seconds"]
        ]
        if evidence is not None:
            evidence.append(
                dict(
                    scene_id=packet["id"],
                    status="assessed" if len(hits) == 1 else "unassessed",
                    source_line=lyrics[0]["id"],
                    reason="Unique exact native-word match"
                    if len(hits) == 1
                    else f"Expected one native-word match; found {len(hits)}",
                )
            )
        if len(hits) != 1:
            continue
        at = hits[0]
        onset = words[at]["start"]
        end = words[at + len(expected) - 1]["end"]
        cut = packet["start_seconds"]
        if onset < cut:
            packets[i - 1]["end_seconds"] = onset
            packet["start_seconds"] = onset
            changes.append(
                dict(
                    scene=packet["id"],
                    previous_scene=packets[i - 1]["id"],
                    old_boundary=cut,
                    boundary=onset,
                    word=words[at]["id"],
                    source_line=lyrics[0]["id"],
                    convention="next source line onset; half-open intervals",
                )
            )
    return changes


def source_line_breaks(words, lines):
    normalize = lambda text: [re.sub(r"[^\w]+", "", t.casefold()) for t in text.split()]
    tokens = [normalize(w["text"])[0] for w in words]
    mapped = []
    used = []
    for line in lines:
        expected = normalize(line["text"])
        hits = [
            i
            for i in range(len(words) - len(expected) + 1)
            if tokens[i : i + len(expected)] == expected
        ]
        if len(hits) != 1:
            return []
        selected = words[hits[0] : hits[0] + len(expected)]
        used.extend(w["id"] for w in selected)
        mapped.append(
            dict(
                id=line["id"],
                source=line["source"],
                text=" ".join(w["text"] for w in selected),
                word_ids=[w["id"] for w in selected],
                start=selected[0]["start"],
                end=selected[-1]["end"],
            )
        )
    return mapped if used == [w["id"] for w in words] else []


def chorus_phrases(words):
    """Four display phrases: repeated opening, response, opening, response."""
    tokens = [re.sub(r"[^\w]+", "", w["text"].casefold()) for w in words]
    candidates = []
    for repeat in range(2, len(words) - 2):
        prefix = 0
        while (
            prefix < repeat
            and repeat + prefix < len(words)
            and tokens[prefix] == tokens[repeat + prefix]
        ):
            prefix += 1
        if prefix >= 2 and prefix < repeat and repeat + prefix < len(words):
            candidates.append((prefix, repeat))
    if not candidates:
        raise ValueError(
            "Four chorus panels require an unambiguous repeated opening in the native words"
        )
    prefix, repeat = max(candidates)
    cuts = [0, prefix, repeat, repeat + prefix, len(words)]
    return [
        dict(
            text=" ".join(w["text"] for w in words[a:b]),
            word_ids=[w["id"] for w in words[a:b]],
            start=words[a]["start"],
            end=words[b - 1]["end"],
        )
        for a, b in zip(cuts, cuts[1:])
    ]


def doctor(manifest, source):
    """Explicit, read-only check for scenes crossing supplied section onsets."""
    board = json.loads(Path(manifest).read_text())
    root = Path(source)
    result = dict(
        ok=True,
        project=board.get("project"),
        storyboard=str(manifest),
        healthy=None,
        status="insufficient_source_evidence",
        tasks=[],
        assessed=[],
        unassessed=[],
        scope_assessed="Scene cuts crossing an exact supplied section opening line",
        scope_unassessed=[
            "Musical structure without supplied section labels",
            "Timing accuracy in the recording",
            "Rendered typography, artwork, template and animation quality",
        ],
        diagnostic_only=True,
    )
    needed = [root / "artifacts/scene_plan.json", root / "artifacts/script.json"]
    if not all(p.is_file() for p in needed):
        result["unassessed"] = [
            dict(
                reason="No supplied section plan/script",
                missing=[str(p) for p in needed if not p.is_file()],
            )
        ]
        return result
    words_doc = json.loads(Path(board["source_transcript"]).read_text())
    words = words_doc if isinstance(words_doc, list) else words_doc["words"]
    words = [dict(w, id=w.get("id", f"w{i}")) for i, w in enumerate(words)]
    pages = [
        (i + 1, p["group"])
        for i, p in enumerate(board.get("page_plan", []))
        if p.get("group")
    ]
    packets = [
        dict(id=g["id"], start_seconds=g["span_start"], end_seconds=g["span_end"])
        for _, g in pages
    ]
    evidence = []
    proposals = align_imported_sections(packets, words, root, evidence)
    result["assessed"] = [e for e in evidence if e["status"] == "assessed"]
    result["unassessed"] = [e for e in evidence if e["status"] != "assessed"]
    for fix in proposals:
        page = next((i for i, g in pages if g["id"] == fix["previous_scene"]), None)
        affected = [
            w for w in words if fix["boundary"] <= w["start"] < fix["old_boundary"]
        ]
        result["tasks"].append(
            dict(
                id="section-boundary:" + fix["previous_scene"] + ":" + fix["scene"],
                priority=1,
                scene_id=fix["previous_scene"],
                page_id=page,
                next_scene_id=fix["scene"],
                suspected_flaw="Scene includes the opening of the next supplied section",
                source_word_ids=[w["id"] for w in affected],
                affected_lyrics=" ".join(w["text"] for w in affected),
                source_range=[affected[0]["start"], affected[-1]["end"]]
                if affected
                else [],
                expected=dict(boundary=fix["boundary"], convention=fix["convention"]),
                actual=dict(boundary=fix["old_boundary"]),
                evidence=dict(
                    script=str(needed[1]),
                    section_line=fix["source_line"],
                    native_transcript=board["source_transcript"],
                    onset_word=fix["word"],
                ),
                confidence="Exact native-word match to a supplied section line; not a musical inference",
                proposed_fix=f"Review {fix['previous_scene']} / page {page}, then set its end and {fix['scene']} start to {fix['boundary']}s; preserve every native word/timestamp",
            )
        )
    result["healthy"] = (
        False
        if result["tasks"]
        else None
        if result["unassessed"] or not result["assessed"]
        else True
    )
    result["status"] = (
        "issues_found"
        if result["tasks"]
        else "no_supported_issues"
        if result["healthy"]
        else "insufficient_source_evidence"
    )
    return result


_TEMPLATE_PLACEHOLDERS = (
    "Describe the story, characters",
    "Name this location and place",
    "Name the cast and reusable props",
    "Register existing artwork",
    "Place the scene reference",
    "See the four panels",
    "Describe the visible action",
    "Link cast, props and locations",
    "Describe the reference image",
    "Describe movement, timing",
    "Text, word IDs, reveal timing",
    "What is drawn, when it appears",
)


def _is_authored_text(val):
    if not isinstance(val, str):
        return False
    s = val.strip()
    if not s:
        return False
    return (
        not any(s.startswith(p) for p in _TEMPLATE_PLACEHOLDERS)
        and "\nRegister existing artwork" not in s
    )


_BG_NOTES = {
    "barn-open": "Main Lyric Backdrop (words-verse-1-1, 0.00–8.80s) & Scene 2–3 plate: Open red-brown cut-paper barn with warm interior glow.",
    "meadow-day": "Alternate Lyric Backdrop (words-verse-2-1, 8.40–20.64s) & Scene 5–6 plate: Sunlit painted meadow with felt flower stage.",
    "barn-closed": "Scene 1 Intro Plate (0.00–1.80s): Dawn at the closed red-brown cut-paper barn doors with a warm crack of light.",
    "meadow-golden": "Scene 4 Sky & Scene 8 Golden-Hour Rest Plate (15.48–20.64s): Warm evening meadow & far-barn sky plates.",
    "far-barn-sky": "Scene 4 Mid-Air Zoom Plate (6.50–8.80s): Open sky with distant barn for the BZZZZZZ close-up.",
    "bunnies-on-ground": "Main Lyric Backdrop (words-verse-1-1, 0.39–12.1s) & Scene 1–3 plate: Stitched wool-felt bunnies on burlap/kraft ground by twig fence.",
    "rabbit-hole-underground": "Scene 4–6 Cutaway Burrow Plate (12.1–24.6s): Torn-paper grass above & corrugated-cardboard UNDERGROUND burrow below.",
    "garden-lettuce-carrots": "Alternate Lyric Backdrop (words-verse-2-1, 24.6–33.9s) & Scene 7–9 plate: Crinkled tissue-paper lettuce & painted carrot tops in soil.",
    "farmer-looking-burrow": "Scene 10–12 Finale Plate (33.9–47.5s): Collage straw-hat/denim farmer looking around while cotton tails dive underground.",
}
_IMAGE_EXTS = (".png", ".jpg", ".jpeg", ".webp")


def _read_words_inventory(source_root):
    words_path = source_root / "design/words.json"
    words_doc = json.loads(words_path.read_text()) if words_path.is_file() else {}
    lyric_bgs, draw_names = {}, []
    for anim in words_doc.get("animations") or []:
        bg = anim.get("background_asset")
        sec = anim.get("scope", {}).get("section", anim.get("animation_id", ""))
        if bg:
            lyric_bgs[bg] = f"{anim.get('animation_id')} ({sec})"
        for d in anim.get("kinetic", {}).get("draws", []):
            name = d.get("drawing", {}).get("name")
            if name and name not in draw_names:
                draw_names.append(name)
    return lyric_bgs, draw_names


def _scan_background_files(bg_root):
    found = {}
    if not bg_root.is_dir():
        return found
    for entry in sorted(bg_root.iterdir()):
        if entry.is_dir():
            img = next(
                (p for p in sorted(entry.iterdir()) if p.suffix.lower() in _IMAGE_EXTS),
                None,
            )
            if img:
                found[entry.name] = img
        elif entry.suffix.lower() in _IMAGE_EXTS:
            found[entry.stem] = entry
    return found


def _discover_locations(source_root, lyric_bgs):
    found_bgs = _scan_background_files(source_root / "assets/backgrounds")
    ordered = [k for k in lyric_bgs if k in found_bgs] + [
        k for k in found_bgs if k not in lyric_bgs
    ]
    preferred = [
        k
        for k in (
            "barn-open",
            "meadow-day",
            "barn-closed",
            "meadow-golden",
            "bunnies-on-ground",
            "garden-lettuce-carrots",
            "farmer-looking-burrow",
            "rabbit-hole-underground",
        )
        if k in found_bgs
    ]
    chosen = preferred[:4] if len(preferred) >= 4 else ordered[:4]
    locations = []
    for bg_id in chosen:
        img = found_bgs[bg_id]
        fallback = (
            f"Mapped lyric backdrop for {lyric_bgs[bg_id]}"
            if bg_id in lyric_bgs
            else f"Storyboard background plate ({bg_id})"
        )
        note = _BG_NOTES.get(bg_id, fallback)
        locations.append(
            dict(
                id=bg_id,
                name=bg_id.replace("-", " ").title(),
                file=str(img.resolve()),
                notes=f"{note}\n{img.name}",
                notes_exact=True,
            )
        )
    return locations


def _first_existing_file(source_root, rel_paths):
    for rel in rel_paths:
        candidate = source_root / rel
        if candidate.is_file():
            return candidate
    return None


def _discover_assets(source_root, draw_names):
    short_draws = ", ".join(n.replace("Ink Action: ", "") for n in draw_names[:4])
    is_bunnies = (source_root / "assets/characters/bunny/bunny-assembled.png").is_file()
    specs = (
        [
            (
                "bunny",
                "Little Felt Bunnies (Cutout Rig)",
                ("assets/characters/bunny/bunny-assembled.png",),
                "Blanket-stitched oatmeal wool-felt bunny puppet with gingham inner ears, button eyes & 3D cotton-pom tail.",
            ),
            (
                "garden-kit",
                "Garden Vocabulary Prop Kit",
                ("assets/characters/garden/garden-assembled.png",),
                "Mixed-media props: crinkled tissue CRISPY LETTUCE, painted CRUNCHY CARROT TOPS, cardboard RABBIT HOLE & denim FARMER.",
            ),
            (
                "storyboard-grid",
                "Story Progression Grid",
                ("assets/references/storyboard-grid-v1.png",),
                "6 handmade mixed-media story beats (story-1..6.jpg): Sitting on Ground, Heard Farmer, Hop & Scurry, Underground, Garden, Burrow.",
            ),
            (
                "teaching-cues",
                "Ink & Chalk Learning Cues",
                ("assets/references/teaching-cues-v1.png",),
                f"10 animated ink/chalk vocabulary drawings (z-index 77: {short_draws}) + hide-and-seek play cues (teach-1..6.jpg).",
            ),
        ]
        if is_bunnies
        else [
            (
                "bee",
                "Bumble Bee (Cutout Rig)",
                (
                    "assets/characters/bee/bee-assembled.png",
                    "assets/bee-rest-pose.png",
                    "assets/characters/bee/parts/body-clean.png",
                ),
                "Ochre felt bee rig (wing-l/r, sack-l/r, body, antenna-l/r, eyes-open/closed) · 5 Studio WebM tracks (z-index 10).",
            ),
            (
                "flower",
                "Meadow Flower (Cutout Rig)",
                (
                    "assets/characters/flower/flower-assembled.png",
                    "assets/characters/flower/parts/head.png",
                ),
                "Felt meadow flower rig (stem, head, petal-1..5, pollen centre) · Petals sway on independent phases in Verse 2.",
            ),
            (
                "storyboard-grid",
                "Story Progression Grid",
                ("assets/references/storyboard-grid-v1.png",),
                "6 painted gouache & cut-paper story beats (story-1..6.jpg): Dawn Barn, Exit with Sacks, BZZZ Zoom, Flower Loop, Pollen/Honey Dips, Golden Rest.",
            ),
            (
                "teaching-cues",
                "Ink-Art & Teaching Cues",
                ("assets/references/teaching-cues-v1.png",),
                f"7 lyric ink-art action drawings (z-index 77: {short_draws}) + parent-baby tickle cues (teach-1..6.jpg).",
            ),
        ]
    )
    assets = []
    for item_id, name, rel_paths, note in specs:
        img = _first_existing_file(source_root, rel_paths)
        if img:
            assets.append(
                dict(
                    id=item_id,
                    name=name,
                    file=str(img.resolve()),
                    notes=f"{note}\n{img.name}",
                    notes_exact=True,
                )
            )
    return assets[:4]


def _upgrade_assembled_character_cards(source_root, assets):
    mapping = {
        "bee": "assets/characters/bee/bee-assembled.png",
        "flower": "assets/characters/flower/flower-assembled.png",
        "bunny": "assets/characters/bunny/bunny-assembled.png",
        "garden-kit": "assets/characters/garden/garden-assembled.png",
    }
    for item in assets:
        rel = mapping.get(item.get("id"))
        if rel and (source_root / rel).is_file():
            item["file"] = str((source_root / rel).resolve())


def _fill_bunnies_overview(config, locs, draws_label):
    if not config.get("story"):
        config["story"] = (
            "Three blanket-stitched wool-felt bunnies sit still on burlap ground as the denim-and-plaid farmer walks by, "
            "hop and scurry to hide right underground in a corrugated-cardboard burrow, hop back one by one to eat "
            "crinkled tissue-paper crispy lettuce and painted crunchy carrot tops, and dive back underground. "
            "Paired with action-animating kinetic lyrics and 10 ink/chalk visual-learning drawings."
        )
    if not config.get("background_main") and locs:
        loc0 = locs[0]
        config["background_main"] = (
            f"{loc0['name']} ({Path(loc0['file']).name}) & Cutaway Burrow (rabbit-hole-underground.png): "
            "Mapped to Verse 1 lyrics (words-verse-1-1, 0.39–21.40s) & Scenes 1–6. Burlap/kraft ground, twig fence, "
            "and corrugated-cardboard UNDERGROUND burrow cross-section."
        )
    if not config.get("background_alternate") and len(locs) > 1:
        loc1 = locs[1]
        config["background_alternate"] = (
            f"{loc1['name']} ({Path(loc1['file']).name}) & Farmer Looking (farmer-looking-burrow.png): "
            "Mapped to Verse 2 lyrics (words-verse-2-1, 21.40–41.48s) & Scenes 7–12. Mixed-media vegetable garden "
            "with crinkled tissue-paper crispy lettuce, painted carrot tops, and golden-hour burrow finale."
        )
    if not config.get("cast"):
        config["cast"] = (
            "Cast & Mixed-Media Learning Props: (1) Oatmeal Wool-Felt Bunnies with gingham ears & 3D cotton-pom tails; "
            "(2) Garden Vocabulary Kit (crinkled tissue Crispy Lettuce, painted Crunchy Carrot Tops, corrugated-cardboard "
            f"Rabbit Hole & Underground burrow, straw-hat/denim Farmer); (3) 10 Ink/Chalk Learning Drawings ({draws_label})."
        )


def _fill_overview_defaults(source_root, config, draw_names):
    locs = config.get("locations") or []
    draws_label = (
        ", ".join(n.replace("Ink Action: ", "") for n in draw_names)
        if draw_names
        else "Bee Out of Barn, Honey Sacks, Buzz Zigzag, Circle Palm, Tap Pollen, Honey Drop, Rub Tummy"
    )
    if (source_root / "assets/characters/bunny/bunny-assembled.png").is_file():
        _fill_bunnies_overview(config, locs, draws_label)
        return
    if not config.get("story"):
        config["story"] = (
            "A felt bumble bee wakes at the red-brown barn, tumbles out with a honey sack under each arm, "
            "zooms BZZZZZZ toward camera, circles a big meadow flower, dips twice for pollen and honey, "
            "and rests in golden light. Paired with big visual-learning kinetic lyrics (cascade layout, "
            "paper-grain overlay) and 7 ink-art action drawings for parent-and-baby play."
        )
    if not config.get("background_main") and locs:
        loc0 = locs[0]
        config["background_main"] = (
            f"{loc0['name']} ({Path(loc0['file']).name}): Mapped to Verse 1 lyrics "
            "(words-verse-1-1, 0.00–8.80s) & Scenes 1–4. Red-brown cut-paper barn with open double doors, "
            "warm interior light, painted hills, and foreground grass."
        )
    if not config.get("background_alternate") and len(locs) > 1:
        loc1 = locs[1]
        config["background_alternate"] = (
            f"{loc1['name']} ({Path(loc1['file']).name}) & Golden Meadow (meadow-golden.png): "
            "Mapped to Verse 2 lyrics (words-verse-2-1, 8.40–20.64s) & Scenes 5–8. Sunlit meadow with "
            "the central felt flower rig and golden-hour rest finale."
        )
    if not config.get("cast"):
        config["cast"] = (
            "Cast & Visual-Learning Props: (1) Ochre Felt Bumble Bee rig with burlap honey sacks under each arm "
            "(5 transparent WebM studio tracks in #view-studio only); (2) Felt Meadow Flower rig (stem, head, "
            f"5 swaying petals, pollen centre); (3) 7 SVG Ink-Art Action Drawings ({draws_label})."
        )


def _discover_project_inventory(source_root, packets, config):
    if not source_root or not source_root.is_dir():
        return
    lyric_bgs, draw_names = _read_words_inventory(source_root)
    if not config.get("locations"):
        locations = _discover_locations(source_root, lyric_bgs)
        if locations:
            config["locations"] = locations
    if not config.get("assets"):
        assets = _discover_assets(source_root, draw_names)
        if assets:
            config["assets"] = assets
    else:
        _upgrade_assembled_character_cards(source_root, config["assets"])
    _fill_overview_defaults(source_root, config, draw_names)


def build(
    transcript,
    source,
    out,
    title,
    scene_map,
    allow_timing_overrun=False,
    panels=None,
    template=None,
):
    # The transcript is already the CLI schema. No lyric reformatting or second editable document.
    from storyboard_layout import generate, SIZE, SCENE_ART_REGION

    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    packets = canonical_scenes(scene_map)
    metadata = json.loads(Path(scene_map).read_text())
    words_doc = json.loads(Path(transcript).read_text())
    words = words_doc if isinstance(words_doc, list) else words_doc["words"]
    words = [dict(w, id=w.get("id", f"w{i}")) for i, w in enumerate(words)]
    source_root = (
        Path(source)
        if source
        else Path(metadata["project"]).parent
        if metadata.get("project")
        else None
    )
    alignment = []  # Project creation owns timing; storyboard edits never realign recorded words.
    groups = []
    for packet in packets:
        selected = [
            w
            for w in words
            if packet["start_seconds"] - 0.001
            <= w["start"]
            < packet["end_seconds"] - 0.001
        ]
        group = dict(
            id=packet["id"],
            word_ids=[w["id"] for w in selected],
            span_start=packet["start_seconds"],
            span_end=packet["end_seconds"],
            kind=packet.get("kind", "provisional scene"),
            word_assignment="source-start timestamp; canonical scene cuts preserved",
            status=packet.get("status", "needs review"),
            notes=packet.get("notes") or [],
            reason="Canonical CLI scene span; review grouping without forced verse taxonomy.",
        )
        group["section_label"] = packet.get("section_label")
        group["lyric_lines"] = source_line_breaks(
            selected, packet.get("source_lines", [])
        )
        if panels == packet["id"] or packet.get("panels"):
            if group["kind"] != "chorus":
                raise ValueError(
                    "Four-line panel layout requires a source-labelled chorus"
                )
            group["panels"] = chorus_phrases(selected)
            for panel in group["panels"]:
                panel["line_id"] = next(
                    (
                        l["id"]
                        for l in group["lyric_lines"]
                        if set(panel["word_ids"]) <= set(l["word_ids"])
                    ),
                    None,
                )
        groups.append(group)
    config = dict(
        title=title,
        transcript=str(transcript),
        captions=metadata.get("captions"),
        allow_timing_overrun=allow_timing_overrun,
        duration_sec=metadata["duration_sec"],
        recording_duration_verified=metadata.get("recording_duration_verified", False),
        audio=metadata.get("audio"),
        project=metadata.get("project"),
        scene_content={
            p["id"]: dict(
                sections=p.get("sections", {}),
                notes=p.get("notes", []),
                source=p.get("source"),
                lines=p.get("lines", []),
            )
            for p in packets
        },
    )
    canonical_template = (
        Path(_REPO_ROOT) / "pipeline/templates/studio-template.deckcraft"
    )
    template = Path(template).resolve() if template else canonical_template.resolve()
    if template != canonical_template.resolve():
        raise ValueError(
            "Magic Studio uses only pipeline/templates/studio-template.deckcraft"
        )
    schema = template_schema(template, out)
    config["scene_content"], mapping = normalize_scene_content(
        packets, [f["role"] for f in schema["scene"]]
    )
    # Actual saved user fields take precedence over imported notes; lyrics remain native words.
    for packet in packets:
        for f in packet.get("text_fields", []):
            if f.get("role", "").startswith("user.scene.") and f.get("field") in [
                "action",
                "continuity",
                "image_prompt",
                "animation_prompt",
                "animated_text",
                "drawing",
            ]:
                key = {
                    "continuity": "layer_stack",
                    "image_prompt": "reference_prompt",
                    "drawing": "drawing_cues",
                }.get(f["field"], f["field"])
                if not config["scene_content"][packet["id"]]["sections"].get(
                    key
                ) and _is_authored_text(f.get("value")):
                    config["scene_content"][packet["id"]]["sections"][key] = f["value"]
                    role = "user.scene." + f["field"]
                    mapping["missing"] = [
                        m
                        for m in mapping["missing"]
                        if not (m["scene"] == packet["id"] and m["field"] == role)
                    ]
    config["fit_imported_text"] = True
    config["scene_images"] = {
        p["id"]: dict(file=p["reference"], name=Path(p["reference"]).name)
        for p in packets
        if p.get("reference") and Path(p["reference"]).is_file()
    }
    official = source_root / "artifacts/official_lyrics.json" if source_root else None
    if official and official.is_file():
        config["official_lyrics_source"] = str(official)
    mapping["timing_conflicts"] = [
        dict(
            scene=g["id"],
            word=w["id"],
            text=w["text"],
            start=w["start"],
            end=w["end"],
            scene_end=g["span_end"],
            resolution="Source timing preserved; word occurrence assigned by its original start",
        )
        for g in groups
        for w in words
        if w["id"] in g["word_ids"] and w["end"] > g["span_end"] + 0.001
    ]
    config["asset_links"] = metadata.get("assets", [])
    previous = metadata.get("previous_board") or {}
    content = metadata.get("page_content") or []
    config["page_content"] = {
        page["key"]: [
            f
            for f in content
            if f.get("page") == i + 1
            and f.get("role", "").startswith("user.")
            and _is_authored_text(f.get("value"))
        ]
        for i, page in enumerate(previous.get("page_plan", []))
        if page["kind"] != "scene"
        and not (
            page["kind"] == "locations" and metadata.get("backgrounds") is not None
        )
    }
    # Keep authored inventory pages; a scene edit must not replace them with a new asset search.
    for kind, key in [("assets", "assets"), ("locations", "locations")]:
        retained = [
            record
            for page in previous.get("page_plan", [])
            if page["kind"] == kind
            for record in page.get("sheet", [])
            if record.get("id") != "inventory" and record.get("file")
        ]
        if retained:
            config[key] = retained
    if metadata.get("backgrounds") is not None:
        config["locations"] = metadata["backgrounds"]
        for record in config["locations"]:
            for i, page in enumerate(previous.get("page_plan", [])):
                if page["kind"] != "locations":
                    continue
                for slot, old in enumerate(page.get("sheet", [])):
                    if old["id"] != record["id"]:
                        continue
                    field = next(
                        (
                            f
                            for f in content
                            if f.get("page") == i + 1
                            and f.get("field") == f"sheet.{slot}.notes"
                        ),
                        None,
                    )
                    if field and _is_authored_text(field.get("value")):
                        record.update(notes=field["value"], notes_exact=True)
    _discover_project_inventory(source_root, packets, config)
    overrun = max(w["end"] for w in words) - float(metadata["duration_sec"])
    if overrun > 0.001:
        config["timing_warning"] = (
            f"Source word end {max(w['end'] for w in words):.6f}s exceeds measured audio {metadata['duration_sec']:.6f}s by {overrun:.6f}s. Raw words/timestamps retained; timing is not validated."
        )
        mapping["timing_warning"] = config["timing_warning"]
    mapping["needs_correction"] = bool(mapping["unmapped"])
    mapping["template_file"] = str(template)
    mapping["template_schema"] = schema
    mapping["line_mapping"] = [
        dict(
            scene=g["id"],
            line_ids=[l["id"] for l in g["lyric_lines"]],
            status="exact source-word sequence"
            if g["lyric_lines"]
            else "unmapped native words retained; no line IDs invented",
        )
        for g in groups
    ]
    mapping["import_alignment"] = alignment
    mapping["source_words"] = len(words)
    mapping["scene_spans"] = [[p["start_seconds"], p["end_seconds"]] for p in packets]
    config["mapping"] = mapping
    (out / "mapping.json").write_text(json.dumps(mapping, indent=2))
    config_path = out / "template-inputs.json"
    config_path.write_text(json.dumps(config, indent=2))
    grouping = out / "scene-map-input.json"
    grouping.write_text(json.dumps(dict(groups=groups), indent=2))
    deck = out / "movie-board.deckcraft"
    try:
        result = generate(template, config_path, deck, grouping)
    except Exception as error:
        return dict(
            ok=False,
            status="draft blocked; active storyboard unchanged",
            error=str(error),
            template_file=str(template),
            mapping=mapping,
            config=str(config_path),
            scene_map=str(scene_map),
            correction="Resolve the reported source/template mismatch, then rerun the same CLI command; original data is retained",
        )
    pages = out / "pages"
    pages.mkdir(exist_ok=True)
    pdf_bin = shutil.which("pdfcraft-cli") or "pdfcraft-cli"
    for i in range(len(result["page_plan"])):
        subprocess.run(
            [
                pdf_bin,
                "render",
                result["pdf"],
                "--page",
                str(i + 1),
                "--dpi",
                "72",
                "--out",
                str(pages / f"slide-{i + 1:03}.png"),
            ],
            check=True,
            capture_output=True,
            text=True,
        )
    validation = result["validation"]
    page_list = validation["pages"]
    first_scene = next(i for i, page in enumerate(page_list) if page["kind"] == "scene")
    links = validation["link_regions"]
    result.update(
        mapping=mapping,
        template="deckcraft",
        template_file=str(template),
        scene_map=str(scene_map),
        scene_targets=[p["id"] for p in packets],
        scene_count=len(packets),
        first_scene_page=first_scene + 1,
        scene_titles=[
            f"{p['id']} · {p['start_seconds']:.3f}–{p['end_seconds']:.3f}"
            for p in packets
        ],
        previews=[str(p) for p in sorted(pages.glob("slide-*.png"))],
        page_size=SIZE,
        stage_rect=SCENE_ART_REGION,
        links=links,
        page_titles=[p["title"] for p in result["page_plan"]],
        title=title,
        editor="DeckCraft native document; PDF export",
        source_transcript=str(transcript),
        source_transcript_sha256=hashlib.sha256(
            Path(transcript).read_bytes()
        ).hexdigest(),
        source_producer_label=Path(transcript).parent.name,
        producer_executable_version="not recorded by native array",
        source_words=len(words),
        timing_valid=overrun <= 0.001,
        timing_warning=config.get("timing_warning"),
        next_commands=[
            shlex.join(["deckcraft-cli", "info", str(deck)]),
            shlex.join(
                [
                    "deckcraft-cli",
                    "run",
                    "--in",
                    str(deck),
                    "--print",
                    "--cmd",
                    'slide.inspect={"index":' + str(first_scene) + "}",
                ]
            ),
        ],
    )
    # Keep saved placements linked; remap only a full-frame overlay on a newly panelled scene.
    from storyboard_layout import fit_rect, panel_region, SCENE_ART_REGION

    placements = []
    for packet in packets:
        page = next(
            i + 1
            for i, p in enumerate(result["page_plan"])
            if p.get("group", {}).get("id") == packet["id"]
        )
        for original in packet.get("saved_placements", []):
            item = dict(original, page=page)
            if panels == packet["id"] and not packet.get("panels"):
                target = fit_rect(
                    SCENE_ART_REGION[2], SCENE_ART_REGION[3], panel_region(3)
                )
                scale = target[2] / SCENE_ART_REGION[2]
                r = item["rect"]
                item["rect"] = [
                    target[0] + (r[0] - 60) * scale,
                    target[1] + (r[1] - 220) * scale,
                    r[2] * scale,
                    r[3] * scale,
                ]
            placements.append(item)
    for original in previous.get("design", {}).get("placements", []):
        old_page = previous.get("page_plan", [])[original["page"] - 1]
        if old_page["kind"] == "scene" or (
            old_page["kind"] == "locations" and metadata.get("backgrounds") is not None
        ):
            continue
        page = next(
            (
                i + 1
                for i, p in enumerate(result["page_plan"])
                if p["key"] == old_page["key"]
            ),
            None,
        )
        if page:
            placements.append(dict(original, page=page))
    if placements:
        result["design"] = dict(placements=placements, dirty=False)
    (out / "manifest.json").write_text(json.dumps(result, indent=2))
    return result


if __name__ == "__main__":
    import sys

    if "--doctor" in sys.argv:
        p = argparse.ArgumentParser(
            description="Read-only source section boundary check"
        )
        p.add_argument("--doctor", required=True)
        p.add_argument("--source", required=True)
        a = p.parse_args()
        print(json.dumps(doctor(a.doctor, a.source)))
        raise SystemExit(0)
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--allow-timing-overrun", action="store_true")
    p.add_argument("--transcript", required=True)
    p.add_argument(
        "--template",
        default=str(Path(_REPO_ROOT) / "pipeline/templates/studio-template.deckcraft"),
    )
    p.add_argument("--scene-map", required=True)
    p.add_argument("--source")
    p.add_argument("--out", required=True)
    p.add_argument("--title", required=True)
    p.add_argument("--panels")
    a = p.parse_args()
    try:
        print(
            json.dumps(
                build(
                    a.transcript,
                    a.source,
                    a.out,
                    a.title,
                    a.scene_map,
                    a.allow_timing_overrun,
                    a.panels,
                    a.template,
                )
            )
        )
    except subprocess.CalledProcessError as e:
        raise SystemExit(e.stderr or e.stdout or str(e))
