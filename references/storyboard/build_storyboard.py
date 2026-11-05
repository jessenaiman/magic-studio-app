#!/usr/bin/env python3
"""Generate the Magic Studio storyboard DesignCraft script.

  build_storyboard.py populated  -> nelly-storyboard.dcs
  build_storyboard.py template   -> studio-template.dcs

Every editable frame gets an object.label so magic-cli / designcraft-cli can
find it with object.findByLabel and fill it. Scenes are labelled with the
same names on every scene spread.
"""
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ASSETS = os.path.join(HERE, "assets")
NELLY = os.path.abspath(os.path.join(HERE, "../../media/merriweather/nelly"))
W, H = 1600, 1000          # one page == one open-book spread
PW = 800                   # page width (left page 0..800, right 800..1600)

INK, RUST, CREAM, KRAFT = "Ink", "Rust", "Cream", "Kraft"
SW = {
    "Ink": "#35291f", "Rust": "#9a3b2a", "Cream": "#fdf5e2", "Kraft": "#ecd9ab",
    "Tape": "#d8bb88", "Navy": "#27405e", "Slot": "#e3ddcd", "Film": "#2a2a27",
    "Leather": "#6b4630", "Pencil": "#6e665d", "Moss": "#5f7a4c", "Sun": "#e9a23b",
    "Spine0": "#000000", "Hair": "#c9a47a",
}
SERIF_D = "Fraunces"
SERIF = "Newsreader"
SANS = "Nunito"
HAND = "Caveat"


class B:
    def __init__(self, pages):
        self.lines = []
        self.labels = []
        self.spread = 0
        self.cmd("file.new", {"width": W, "height": H, "pages": pages, "facingPages": False,
                              "margins": 0, "units": "points", "title": "Magic Studio Storyboard"})
        for n, c in SW.items():
            self.cmd("swatch.create", {"name": n, "color": c})
        self.cmd("swatch.create", {"name": "SpineGrad", "gradient": {"kind": "linear", "stops": [
            {"location": 0, "swatch": "Spine0", "opacity": 0},
            {"location": 50, "swatch": "Leather", "opacity": 55},
            {"location": 100, "swatch": "Spine0", "opacity": 0}]}})

    def cmd(self, name, params=None):
        params = params or {}
        if name == "frame.create":
            self._pending = (params.get("rect"), params.get("content") or "shape")
        elif name == "object.label":
            rect, kind = getattr(self, "_pending", (None, None))
            if self.spread < 3:      # cover / world / character: namespaced so scene labels stay unique
                params = dict(params, label=["cover_", "world_", "char_"][self.spread] + params["label"])
            self.labels.append({"spread": self.spread, "label": params["label"], "rect": rect, "kind": kind})
        self.lines.append({"command": name, "params": params})
        return len(self.lines) - 1

    def ref(self, i):
        return "$%d.id" % i

    # -- primitives ---------------------------------------------------
    def rect(self, r, fill=None, stroke=None, sw=0, label=None, shadow=None, shape="rectangle",
             opacity=None, dash=False):
        i = self.cmd("frame.create", {"spread": self.spread, "rect": list(r), "shape": shape})
        ids = [self.ref(i)]
        self._style(ids, fill, stroke, sw, label, shadow, opacity, dash)
        return i

    def _style(self, ids, fill, stroke, sw, label, shadow, opacity, dash=False):
        self.cmd("selection.set", {"ids": ids})
        if fill:
            self.cmd("object.fill", {"swatch": fill})
        else:
            self.cmd("object.fill", {"swatch": "[None]"})
        if stroke and sw:
            p = {"swatch": stroke, "weight": sw}
            if dash:
                p["type"] = {"kind": "dashed", "pattern": [6, 4]}
            self.cmd("object.stroke", p)
        else:
            self.cmd("object.stroke", {"swatch": "[None]", "weight": 0})
        if shadow:
            d, s, o = shadow
            self.cmd("object.dropShadow", {"on": True, "distance": d, "size": s, "opacity": o,
                                           "angle": 125, "globalLight": False, "ids": ids})
        if opacity is not None:
            self.cmd("object.opacity", {"opacity": opacity, "ids": ids})
        if label:
            self.cmd("object.label", {"label": label, "ids": ids})

    def text(self, r, s, font=SERIF, style=None, size=14, color=INK, label=None, align="left",
             leading=None, tracking=0, caps=False, valign="top"):
        if font == HAND:
            r = list(r)
            r[3] = max(r[3], r[1] + size * 1.3 * 1.25 * (s.count("\n") + 1) + 2)
            size = size * 1.3
            if leading:
                leading = leading * 1.2
        if font != SANS:
            s = s.replace("\u2192", "\u203a").replace("\u2248", "~")
        i = self.cmd("frame.create", {"spread": self.spread, "rect": list(r), "content": "text", "text": s})
        ids = [self.ref(i)]
        self.cmd("selection.set", {"ids": ids})
        self.cmd("object.fill", {"swatch": "[None]"})
        self.cmd("object.stroke", {"swatch": "[None]", "weight": 0})
        self.cmd("object.textFrameOptions", {"inset": 0, "verticalJustification": valign, "ids": ids})
        a = {"fontFamily": font, "size": size, "fill": color, "tracking": tracking}
        if style:
            a["fontStyle"] = style
        if leading:
            a["leading"] = {"kind": "points", "value": leading}
        if caps:
            a["capitalization"] = "allCaps"
        self.cmd("type.char", {"attrs": a})
        self.cmd("type.para", {"attrs": {"align": align}})
        if label:
            self.cmd("object.label", {"label": label, "ids": ids})
        return i

    def image(self, r, path, label=None, ph_text=None, shadow=None, fitmode="fillProportionally"):
        """A picture frame. In template mode (path None) an empty labelled slot with placeholder fill."""
        if path and os.path.exists(path):
            i = self.cmd("frame.create", {"spread": self.spread, "rect": list(r), "content": "graphic"})
            self.cmd("place.load", {"path": path})
            self.cmd("place.drop", {"x": (r[0] + r[2]) / 2, "y": (r[1] + r[3]) / 2, "frame": self.ref(i)})
            self.cmd("object.fit", {"mode": fitmode, "ids": [self.ref(i)]})
            ids = [self.ref(i)]
            self.cmd("selection.set", {"ids": ids})
            if shadow:
                d, s, o = shadow
                self.cmd("object.dropShadow", {"on": True, "distance": d, "size": s, "opacity": o,
                                               "angle": 125, "globalLight": False, "ids": ids})
            if label:
                self.cmd("object.label", {"label": label, "ids": ids})
            return i
        i = self.rect(r, fill="Slot", label=label, shadow=shadow)
        if ph_text:
            self.text([r[0] + 8, (r[1] + r[3]) / 2 - 22, r[2] - 8, (r[1] + r[3]) / 2 + 22], ph_text,
                      font=SERIF, style="Italic", size=13, color="Pencil", align="center", valign="center",
                      label=(label + "_hint") if label else None)
        return i

    def rotate(self, ids, deg):
        self.cmd("transform.rotate", {"angle": deg, "ids": [self.ref(i) if isinstance(i, int) else i for i in ids]})

    def line(self, a, b, color=RUST, w=1.2, label=None):
        i = self.cmd("line.create", {"spread": self.spread, "a": list(a), "b": list(b)})
        self.cmd("selection.set", {"ids": [self.ref(i)]})
        self.cmd("object.stroke", {"swatch": color, "weight": w})
        if label:
            self.cmd("object.label", {"label": label, "ids": [self.ref(i)]})
        return i

    def tape(self, cx, cy, w=110, h=28, rot=0, label=None):
        p = os.path.join(ASSETS, "washi-tape.png")
        r = [cx - w / 2, cy - h / 2, cx + w / 2, cy + h / 2]
        if os.path.exists(p):
            i = self.image(r, p, fitmode="fitProportionally")
        else:
            i = self.rect(r, fill="Tape", opacity=82)
        if rot:
            self.rotate([i], rot)
        return i

    def polaroid(self, r, path, label, caption, rot=0, ph=None, pad=10, bottom=34, tape_at=0.5):
        """Cream print card with image + handwritten caption + tape. Returns all ids."""
        card = [r[0], r[1], r[2], r[3]]
        c = self.rect(card, fill="Cream", shadow=(5, 9, 0.38), label=label + "_card")
        im = [r[0] + pad, r[1] + pad, r[2] - pad, r[3] - bottom]
        img = self.image(im, path, label=label, ph_text=ph)
        cap = self.text([r[0] + pad, r[3] - bottom + 6, r[2] - pad, r[3] - 4], caption, font=HAND, size=15,
                        color="Pencil", label=label + "_caption", align="center")
        t = self.tape(r[0] + (r[2] - r[0]) * tape_at, r[1] - 2, rot=-3)
        ids = [c, img, cap, t]
        if rot:
            self.rotate(ids, rot)
        return ids

    def taped_label(self, x, y, w, s, label, size=12, h=24, rot=0, font=SANS, style="Bold"):
        t1 = self.rect([x, y, x + w, y + h], fill="Tape", opacity=88, shadow=(1.5, 3, 0.25))
        t2 = self.text([x + 8, y + 3, x + w - 8, y + h - 2], s, font=font, style=style, size=size, color=INK,
                       label=label, valign="center")
        if rot:
            self.rotate([t1, t2], rot)
        return t2

    def stitch(self, r, inset=6, color="Hair", w=1.1):
        """Running-stitch border, like thread sewn into the paper."""
        self.rect([r[0] + inset, r[1] + inset, r[2] - inset, r[3] - inset], fill=None, stroke=color, sw=w, dash=True)

    def felt_label(self, x, y, w, s, label, fill="Moss", size=13, h=30, rot=0, color="Cream", font=SERIF_D, style="Bold"):
        """Felt-scrap label with a stitched edge."""
        f = self.rect([x, y, x + w, y + h], fill=fill, shadow=(2, 4, 0.3), shape="rectangle")
        st = self.rect([x + 4, y + 4, x + w - 4, y + h - 4], fill=None, stroke="Cream", sw=1, dash=True, opacity=70)
        t = self.text([x + 10, y + 2, x + w - 10, y + h - 2], s, font=font, style=style, size=size, color=color,
                      label=label, valign="center", align="center")
        if rot:
            self.rotate([f, st, t], rot)
        return t

    def crop_marks(self, r, color="Pencil", n=14, w=1.1):
        x0, y0, x1, y1 = r
        for (cx, cy, dx, dy) in [(x0, y0, 1, 1), (x1, y0, -1, 1), (x0, y1, 1, -1), (x1, y1, -1, -1)]:
            self.line((cx, cy), (cx + dx * n, cy), color, w)
            self.line((cx, cy), (cx, cy + dy * n), color, w)

    def arrow(self, x0, y, x1, color=RUST):
        self.line((x0, y), (x1, y), color, 1.8)
        self.line((x1 - 9, y - 6), (x1, y), color, 1.8)
        self.line((x1 - 9, y + 6), (x1, y), color, 1.8)

    def checkbox(self, x, y, s=11):
        self.rect([x, y, x + s, y + s], fill=None, stroke="Pencil", sw=1)

    def filmstrip(self, r, slots, label_prefix="ref", fill_paths=None, labels=None):
        """Dark film band with N picture slots (all sizes/N adapt)."""
        x0, y0, x1, y1 = r
        self.rect(r, fill="Film", label="contact_strip", shadow=(3, 6, 0.35))
        # sprocket holes
        hx = x0 + 8
        while hx < x1 - 12:
            self.rect([hx, y0 + 5, hx + 9, y0 + 11], fill="Cream", opacity=90)
            self.rect([hx, y1 - 11, hx + 9, y1 - 5], fill="Cream", opacity=90)
            hx += 20
        n = len(slots)
        gap = 6
        inner = [x0 + 10, y0 + 18, x1 - 10, y1 - 18]
        cw = (inner[2] - inner[0] - gap * (n - 1)) / n
        for k, rr in enumerate(slots):
            pass
        return inner

    # -- paper & page furniture ---------------------------------------
    def paper(self, page_no_left=None, page_no_right=None):
        paper = os.path.join(ASSETS, "kraft-paper.png")
        if os.path.exists(paper):
            self.image([0, 0, W, H], paper, label="paper_bg")
        else:
            self.rect([0, 0, W, H], fill="Kraft", label="paper_bg")
        # leather cover edge + spine shadow
        self.rect([0, 0, W, H], fill=None, stroke="Leather", sw=16, label="book_edge")
        self.rect([PW - 46, 0, PW + 46, H], fill="SpineGrad", label="spine_shadow")
        self.line((PW, 0), (PW, H), "Hair", 0.8)
        if page_no_left:
            self.text([50, 950, 150, 970], page_no_left, font=SANS, size=10, color="Pencil", label="page_no_left")
        if page_no_right:
            self.text([W - 150, 950, W - 50, 970], page_no_right, font=SANS, size=10, color="Pencil",
                      align="right", label="page_no_right")

    def header(self, left_t, left_sub, right_t, right_sub=None):
        self.text([50, 30, 520, 52], left_t, font=SANS, style="Bold", size=14, color=INK, tracking=180,
                  label="running_title")
        self.text([PW - 330, 33, PW - 50, 50], left_sub, font=SANS, size=10, color=INK, tracking=120,
                  align="right", label="running_sub")
        self.line((50, 62), (PW - 50, 62), RUST, 1.4)
        self.text([PW + 50, 30, W - 50, 52], right_t, font=SANS, style="Bold", size=14, color=INK, tracking=120,
                  label="right_running_title")
        self.line((PW + 50, 62), (W - 50, 62), RUST, 1.4)


# ---------------------------------------------------------------------------
# spreads
# ---------------------------------------------------------------------------

def spread_cover(b, d):
    b.spread = d["spread"]
    b.paper("01", "02")
    # left: key art + title
    b.text([60, 58, 740, 82], d["kicker"], font=SANS, style="Bold", size=13, color="Rust", tracking=300,
           label="kicker")
    b.line((60, 90), (740, 90), RUST, 1.4)
    b.text([60, 100, 740, 222], d["title"], font=SERIF_D, style="Bold", size=54, color=INK, leading=58,
           label="title")
    b.text([60, 238, 740, 272], d["subtitle"], font=SERIF, style="Italic", size=22, color="Rust", label="subtitle")
    ids = []
    ids.append(b.rect([60, 300, 740, 300 + 680 * 9 / 16 + 56], fill="Cream", shadow=(6, 10, 0.4), label="key_art_card"))
    ids.append(b.rect([66, 306, 734, 300 + 680 * 9 / 16 + 50], fill=None, stroke="Hair", sw=1, dash=True))
    key = [72, 312, 728, 312 + 656 * 9 / 16]
    ids.append(b.image(key, d.get("key_art"), label="key_art", ph_text="[Key art 16:9]"))
    ids.append(b.text([72, key[3] + 8, 728, key[3] + 36], d["key_caption"], font=HAND, size=16, color="Pencil",
                      label="key_art_caption", align="center"))
    ids.append(b.tape(400, 296, 150, 30, rot=-2))
    b.rotate(ids, -1.2)
    b.felt_label(556, 58, 184, d["badge"], "badge", fill="Moss", size=12, h=32, rot=3)
    b.text([62, 872, 700, 940], d["hand_note"], font=HAND, size=19, color="Rust", label="hand_note", leading=22)

    # right: production overview
    b.text([PW + 50, 100, W - 50, 122], "PRODUCTION OVERVIEW", font=SANS, style="Bold", size=14, color="Rust",
           tracking=200, label="overview_heading")
    b.taped_label(W - 180, 96, 130, d["rev"], "revision", size=11, h=24, rot=-3)
    b.text([PW + 50, 145, W - 50, 215], d["tagline"], font=SERIF, size=20, color=INK, leading=28,
           label="tagline")
    # overall design D01 card
    b.rect([PW + 50, 228, W - 50, 336], fill="Cream", shadow=(4, 8, 0.3), label="d01_card")
    b.stitch([PW + 50, 228, W - 50, 336], 5)
    b.felt_label(PW + 62, 214, 170, "D01 / OVERALL DESIGN", "d01_tag", fill="Rust", size=11, h=24, rot=-2)
    b.text([PW + 70, 250, W - 70, 330], d["overall_design"], font=SERIF, size=14.5, color=INK, leading=20,
           label="overall_design")
    # short video format
    b.text([PW + 50, 354, W - 50, 376], "SHORT-VIDEO FORMAT", font=SANS, style="Bold", size=12, color="Rust",
           tracking=200, label="format_heading")
    b.line((PW + 50, 380), (W - 50, 380), RUST, 1)
    b.text([PW + 50, 388, W - 50, 450], d["format_notes"], font=SERIF, size=14.5, color=INK, leading=19,
           label="format_notes")
    # song map: the song in scenes, with the artist's printed actions
    b.text([PW + 50, 462, W - 50, 484], "THE SONG IN FIVE SCENES", font=SANS, style="Bold", size=12, color="Rust",
           tracking=200, label="songmap_heading")
    b.line((PW + 50, 488), (W - 50, 488), RUST, 1)
    n = len(d["songmap"])
    gap = 12
    tw = (700 - gap * (n - 1)) / n
    for k, (img, sid, action) in enumerate(d["songmap"]):
        sx = PW + 50 + k * (tw + gap)
        rot = [-2.2, 1.6, -1.2, 2.4, -1.8][k % 5]
        cid = b.rect([sx, 500, sx + tw, 500 + tw * 0.5625 + 62], fill="Cream", shadow=(3, 6, 0.32))
        iid = b.image([sx + 5, 505, sx + tw - 5, 505 + (tw - 10) * 0.5625], img, label="songmap_%d" % (k + 1))
        t1 = b.text([sx + 5, 505 + (tw - 10) * 0.5625 + 3, sx + tw - 5, 505 + (tw - 10) * 0.5625 + 54],
                    sid + "\n" + action, font=HAND, size=11.5, color="Rust", align="center", label="songmap_action_%d" % (k + 1))
        b.rotate([cid, iid, t1], rot)
    # master reference register
    b.text([PW + 50, 640, W - 50, 662], "MASTER REFERENCE REGISTER", font=SANS, style="Bold", size=12,
           color="Rust", tracking=200, label="register_heading")
    b.line((PW + 50, 666), (W - 50, 666), RUST, 1)
    y = 672
    for k, (rid, name, where, thumb) in enumerate(d["register"]):
        row = [PW + 50, y, W - 50, y + 36]
        b.rect(row, fill="Cream" if k % 2 == 0 else None, opacity=70 if k % 2 == 0 else None)
        b.felt_label(PW + 58, y + 7, 48, rid, "reg_id_%d" % (k + 1), fill=["Rust", "Navy", "Moss", "Sun", "Sun", "Sun"][k % 6], size=11, h=24)
        b.text([PW + 120, y + 5, PW + 440, y + 36], name, font=SERIF, style="Bold", size=14, color=INK,
               label="reg_name_%d" % (k + 1), valign="center")
        b.text([PW + 440, y + 5, W - 130, y + 36], where, font=SERIF, style="Italic", size=12, color="Pencil",
               label="reg_where_%d" % (k + 1), valign="center")
        b.image([W - 120, y + 2, W - 56, y + 36], thumb, label="reg_thumb_%d" % (k + 1))
        y += 37
    b.text([PW + 50, 900, W - 50, 940], d["footer_note"], font=HAND, size=17, color="Pencil", label="footer_note")


def spread_world(b, d):
    b.spread = d["spread"]
    b.paper("03", "04")
    b.header(d["running_title"], "look development", "LOOK DEVELOPMENT / SOURCE & RESULT")
    b.text([50, 78, 760, 150], d["heading"], font=SERIF_D, style="Bold", size=50, color=INK, label="heading")
    b.text([50, 150, 760, 190], d["subheading"], font=SERIF, style="Italic", size=24, color="Rust",
           label="subheading")
    b.felt_label(556, 94, 184, d["badge"], "badge", fill="Navy", size=12, h=32, rot=3)
    ids = []
    ids.append(b.rect([42, 215, 758, 215 + 716 * 9 / 16 + 54], fill="Cream", shadow=(6, 10, 0.4), label="stage_card"))
    fr = [54, 227, 746, 227 + 692 * 9 / 16]
    ids.append(b.image(fr, d["stage_image"], label="stage_image", ph_text="[Stage plate 16:9]"))
    ids.append(b.text([54, fr[3] + 8, 746, fr[3] + 34], d["stage_caption"], font=HAND, size=16, color="Pencil",
                      label="stage_caption"))
    ids.append(b.tape(400, 212, 150, 30, rot=1.5))
    b.rotate(ids, 0.6)
    b.text([50, 700, 760, 722], "MATERIAL & SCALE", font=SANS, style="Bold", size=12, color="Rust", tracking=200,
           label="material_heading")
    b.line((50, 726), (760, 726), RUST, 1)
    b.text([50, 738, 760, 860], d["material_scale"], font=SERIF, size=16.5, color=INK, leading=23.5, label="material_scale")
    b.text([50, 900, 760, 935], d["hand_note"], font=HAND, size=18, color="Rust", label="hand_note")
    # right page
    x = PW + 50
    b.text([x, 78, W - 50, 100], "REFERENCE REGISTER", font=SANS, style="Bold", size=12, color="Rust", tracking=200,
           label="register_heading")
    b.text([x, 108, W - 50, 190], d["register"], font=SERIF, size=16, color=INK, leading=23, label="register")
    b.line((x, 205), (W - 50, 205), RUST, 1)
    b.text([x, 218, W - 50, 240], "EXACT LOCATION PROMPT", font=SANS, style="Bold", size=12, color="Rust", tracking=200,
           label="location_prompt_heading")
    b.rect([x, 248, W - 50, 428], fill="Cream", shadow=(4, 8, 0.28), label="location_prompt_card")
    b.tape(x + 60, 248, 110, 26, rot=-3)
    b.text([x + 18, 262, W - 68, 420], d["location_prompt"], font=SERIF, size=16, color=INK, leading=23.5,
           label="location_prompt")
    b.text([x, 448, W - 50, 470], "REVISION / REASON", font=SANS, style="Bold", size=12, color="Rust", tracking=200,
           label="revision_heading")
    b.line((x, 474), (W - 50, 474), RUST, 1)
    b.text([x, 486, W - 50, 560], d["revision"], font=SERIF, size=16, color=INK, leading=23, label="revision")
    # source -> result comparison
    sp = b.polaroid([x - 6, 580, x + 330, 580 + 230], d["source_image"], "source_image", d["source_caption"], rot=-2.2,
                    ph="[Source]")
    rp = b.polaroid([x + 380, 592, x + 716, 592 + 230], d["result_image"], "result_image", d["result_caption"],
                    rot=2.4, ph="[Result]", tape_at=0.4)
    b.arrow(x + 336, 690, x + 378)
    b.crop_marks([x + 390, 602, x + 706, 780], n=12)
    b.text([x + 560, 560, x + 716, 580], "crop · 16:9", font=HAND, size=14, color="Pencil", label="crop_note",
           align="right")
    b.text([x, 850, W - 50, 880], d["match_note"], font=HAND, size=18, color="Rust", label="match_note")
    b.text([x, 912, W - 50, 936], d["fit_note"], font=SERIF, size=12, color="Rust", label="fit_note")


def spread_character(b, d):
    b.spread = d["spread"]
    b.paper("05", "06")
    b.header(d["running_title"], "character study", "EXACT PROMPT / " + d["prompt_id"])
    b.text([50, 78, 760, 150], d["heading"], font=SERIF_D, style="Bold", size=50, color=INK, label="heading")
    b.text([50, 150, 760, 190], d["subheading"], font=SERIF, style="Italic", size=24, color="Rust", label="subheading")
    b.felt_label(556, 94, 184, d["badge"], "badge", fill="Navy", size=12, h=32, rot=3)
    o = b.polaroid([48, 235, 380, 635], d["original_image"], "original_image", d["original_caption"], rot=-3.2,
                   ph="[Original drawing]", bottom=56, pad=14)
    r = b.polaroid([330, 275, 735, 800], d["pose_image"], "pose_image", d["pose_caption"], rot=2.6,
                   ph="[Animation-ready pose]", bottom=56, pad=14, tape_at=0.55)
    b.arrow(318, 470, 356)
    b.text([50, 835, 340, 900], d["hand_note"], font=HAND, size=14, color="Rust", label="hand_note")
    b.text([50, 900, 330, 935], d["character_tag"], font=SANS, style="Bold", size=10, color="Pencil", tracking=140,
           label="character_tag")
    # right: saved exact prompt card
    x = PW + 50
    b.rect([x - 10, 82, W - 40, 800], fill="Cream", shadow=(5, 9, 0.32), label="prompt_card")
    b.stitch([x - 10, 82, W - 40, 800], 7)
    b.tape(x + 90, 82, 150, 30, rot=-2)
    b.tape(W - 150, 82, 150, 30, rot=2)
    b.text([x + 24, 112, W - 76, 134], "SAVED EXACT PROMPT", font=SANS, style="Bold", size=13, color="Rust",
           tracking=200, label="prompt_heading")
    b.text([x + 24, 146, W - 76, 640], d["exact_prompt"], font=SERIF, size=15, color=INK, leading=22,
           label="exact_prompt")
    b.text([x + 24, 742, W - 76, 790], d["prompt_foot"], font=HAND, size=16, color="Pencil", label="prompt_foot")
    # geometry
    b.text([x, 825, W - 50, 847], "GEOMETRY TO PRESERVE", font=SANS, style="Bold", size=12, color="Rust", tracking=200,
           label="geometry_heading")
    b.line((x, 851), (W - 50, 851), RUST, 1)
    b.text([x, 860, W - 50, 940], d["geometry"], font=SERIF, size=13, color=INK, leading=19, label="asset_geometry")
    # canvas diagram: 2:3 canvas, body box ~1300px tall, baseline y=1440
    gx, gy, gw, gh = x + 24, 418, 172, 258
    b.rect([gx, gy, gx + gw, gy + gh], fill="Slot", stroke="Pencil", sw=1, label="geometry_canvas")
    base = gy + gh * 1440 / 1536
    b.image([gx + 8, base - gh * 1300 / 1536, gx + gw - 8, base], d.get("pose_image"), label="geometry_pose", fitmode="fitProportionally")
    b.line((gx - 14, base), (gx + gw + 14, base), "Rust", 1.2, label="geometry_baseline")
    top = base - gh * 1300 / 1536
    b.rect([gx + 8, top, gx + gw - 8, base], fill=None, stroke="Rust", sw=0.9, dash=True, label="geometry_body_box")
    b.text([gx + gw + 18, base - 10, gx + gw + 220, base + 8], "baseline y = 1440", font=HAND, size=15, color="Rust")
    b.text([gx + gw + 18, top - 2, gx + gw + 240, top + 16], "body ~1300 px", font=HAND, size=15, color="Rust")
    b.text([gx, gy + gh + 6, gx + gw, gy + gh + 24], "canvas 1024 \u00d7 1536 (2:3)", font=HAND, size=14, color="Pencil", align="center")
    b.text([gx + gw + 60, gy + 60, W - 80, gy + 230], d["silhouette_rules"], font=SERIF, size=13.5, color=INK, leading=21,
           label="silhouette_rules")


def spread_scene(b, d, template=False):
    b.spread = d["spread"]
    b.paper(d["page_left"], d["page_right"])
    # ---- left page --------------------------------------------------
    b.text([50, 28, 520, 52], d["running_title"], font=SERIF_D, style="Bold", size=20, color=INK, tracking=120,
           label="running_title")
    b.text([PW - 300, 32, PW - 50, 50], d["running_sub"], font=SANS, size=10, color=INK, tracking=100, align="right",
           label="running_sub")
    b.line((50, 62), (PW - 50, 62), RUST, 1.4)
    b.felt_label(52, 80, 196, d["badge"], "badge", fill=d.get("felt", "Moss"), size=14, h=34, rot=-1.5)
    b.text([262, 86, 560, 116], d["section"], font=SERIF_D, style="Bold", size=20, color="Rust", tracking=40,
           label="section", caps=True)
    b.text([560, 92, 755, 118], d["hand_note"], font=HAND, size=11, color="Pencil", label="hand_note", align="right")
    b.text([50, 138, 224, 176], d["scene_id"], font=SERIF_D, style="Bold", size=27, color=INK, tracking=20,
           label="scene_id", valign="center")
    b.line((226, 142), (226, 172), "Pencil", 1)
    b.text([240, 140, 755, 176], d["title"], font=SERIF, size=21, color=INK, label="title", valign="center")
    # hero (1920x1080 = 16:9)
    hero_card = [38, 190, 762, 190 + 724 * 0.5625 + 20]
    b.rect(hero_card, fill="Cream", shadow=(5, 9, 0.35), label="hero_card")
    b.stitch(hero_card, 4)
    hero = [48, 200, 752, 200 + 704 * 0.5625]
    b.image(hero, d["hero"], label="hero_image", ph_text="[Scene image · 1920×1080 · 16:9]")
    b.tape(180, 190, 110, 26, rot=-4)
    # audio cue
    y = 640
    b.text([50, y, 215, y + 24], "AUDIO / LYRIC CUE", font=SANS, style="Bold", size=12, color="Rust", tracking=100,
           label="cue_heading")
    b.line((222, y - 4), (222, y + 50), "Pencil", 1)
    n_ = len(d["cue"])
    long = n_ > 76
    csize = 19 if n_ <= 44 else (16.5 if n_ <= 76 else 15)
    b.text([236, y - 4, 755, y + (46 if long else 28)], d["cue"], font=SERIF, style="Italic", size=csize,
           color=INK, leading=20 if long else None, label="cue")
    ty = y + (48 if long else 30)
    b.text([236, ty, 755, ty + 18], d["time"], font=SERIF, size=12, color="Pencil", tracking=60, label="time")
    b.line((236, ty + 22), (480, ty + 22), RUST, 1.2)
    # purpose
    y = 722
    b.text([50, y, 215, y + 24], "PURPOSE / LESSON", font=SANS, style="Bold", size=12, color="Rust", tracking=100,
           label="purpose_heading")
    b.line((222, y - 4), (222, y + 50), "Pencil", 1)
    b.text([236, y - 4, 755, y + 54], d["purpose"], font=SERIF, style="Italic", size=17, color=INK, leading=24,
           label="purpose")
    # staging row
    sy = 800
    skl = [50, sy, 230, sy + 112]
    b.rect(skl, fill="Cream", stroke="Pencil", sw=0.8, label="sketch_left_card")
    b.image([skl[0] + 4, skl[1] + 4, skl[2] - 4, skl[3] - 4], d.get("sketch_left"), label="sketch_left", ph_text="[sketch]")
    b.crop_marks(skl, n=9)
    skr = [575, sy + 4, 750, sy + 108]
    b.image(skr, d.get("sketch_right"), label="sketch_right", ph_text="[sketch]")
    b.crop_marks(skr, n=9)
    b.text([262, sy - 2, 560, sy + 22], "STAGING CHECKLIST", font=SANS, style="Bold", size=10, color="Rust", tracking=140)
    for k in range(3):
        b.checkbox(262, sy + 27 + k * 28)
    b.text([282, sy + 20, 568, sy + 24 + 3 * 28], "\n".join(d["staging"]), font=HAND, size=13, color=INK,
           leading=28 / 1.3, label="staging_notes")
    # ---- right page -------------------------------------------------
    x = PW + 50
    b.taped_label(x - 6, 40, 210, "REFERENCE LIST", "ref_header", size=14, h=30, rot=-1)
    b.taped_label(W - 140, 40, 90, d["revision"], "revision", size=12, h=26, rot=2)
    b.text([x, 92, x + 140, 114], "OVERALL DESIGN", font=SANS, style="Bold", size=12, color="Rust", tracking=100,
           label="overall_heading")
    b.text([x + 155, 90, W - 50, 134], d["overall_design"], font=SERIF, size=13.5, color=INK, leading=17.5, label="overall_design")
    rows = [("Location / original", "location_ref"), ("Character / shared", "character_shared"),
            ("Character / individual", "character_lead"), ("Asset geometry / props", "asset_geometry")]
    y = 146
    for k, (tag, lab) in enumerate(rows):
        h = 38 if lab != "asset_geometry" else 38
        b.taped_label(x, y + 2, 142, tag, lab + "_tag", size=10, h=22)
        b.text([x + 156, y, W - 50, y + h], d[lab], font=SERIF, size=12.5, color=INK, leading=16.5, label=lab)
        b.line((x + 156, y + h + 2), (W - 50, y + h + 2), "Hair", 0.8)
        y += h + 8
    # source / result polaroids
    py = 352
    b.polaroid([x + 4, py, x + 326, py + 218], d["source_image"], "source_image", d["source_caption"], rot=d.get("src_rot", -3),
               ph="[Source]")
    b.polaroid([x + 352, py + 14, x + 690, py + 236], d["result_image"], "result_image", d["result_caption"], rot=d.get("res_rot", 3),
               ph="[Result]", tape_at=0.35)
    b.crop_marks([x + 362, py + 24, x + 680, py + 190], n=11)
    b.text([x + 330, py + 262, x + 690, py + 288], d["match_note"], font=HAND, size=16, color="Rust",
           label="match_note", align="right")
    b.text([x + 560, py - 8, x + 700, py + 12], "crop (16:9)", font=HAND, size=13, color="Pencil", align="right")
    # exact prompt card
    ey = 650
    b.rect([x - 4, ey, W - 44, ey + 138], fill="Cream", shadow=(4, 8, 0.28), label="exact_prompt_card")
    b.stitch([x - 4, ey, W - 44, ey + 138], 5)
    b.taped_label(x + 6, ey - 10, 120, "EXACT PROMPT", "exact_prompt_tag", size=11, h=24, rot=-1.5)
    b.tape(W - 70, ey + 4, 90, 24, rot=18)
    b.text([x + 14, ey + 22, W - 62, ey + 132], d["exact_prompt"], font=SERIF, size=13.5, color=INK, leading=18.5,
           label="exact_prompt")
    # adaptable contact strip: up to 6 slots (see studio-template-build.json for 3/4/6/grid layouts)
    cy = 800
    refs = d.get("refs") or []
    n = len(refs) if refs else 6
    strip = [x - 4, cy, W - 44, cy + 112]
    b.rect(strip, fill="Film", label="contact_strip", shadow=(3, 6, 0.35))
    hx = strip[0] + 8
    while hx < strip[2] - 10:
        b.rect([hx, cy + 4, hx + 8, cy + 9], fill="Cream", opacity=90, label="strip_hole")
        b.rect([hx, cy + 103, hx + 8, cy + 108], fill="Cream", opacity=90, label="strip_hole")
        hx += 18
    gap = 6
    iw = (strip[2] - strip[0] - 20 - gap * (n - 1)) / n
    for k in range(n):
        sx0 = strip[0] + 10 + k * (iw + gap)
        p = refs[k] if k < len(refs) else None
        b.image([sx0, cy + 14, sx0 + iw, cy + 98], p, label="ref_%d" % (k + 1), ph_text=None)
    for k in range(n, 12):   # spare slots, hidden until a scene needs them (scene_fill.py re-lays the strip)
        i = b.image([strip[0] + 10, cy + 14, strip[0] + 10 + iw, cy + 98], None, label="ref_%d" % (k + 1), ph_text=None)
        b.cmd("object.setFlags", {"ids": ["$%d.id" % (i)], "hidden": True})
    b.text([x, cy + 124, W - 50, cy + 144], d["strip_note"], font=HAND, size=14, color="Pencil", label="strip_note",
           align="right")


def contact_layouts():
    """Adaptive contact-strip slot rectangles (spread coords) for N refs."""
    x0, x1 = PW + 50 - 4, W - 44
    out = {}
    for n in (1, 2, 3, 4, 5, 6):
        band = [x0, 800, x1, 912]
        gap, pad = 6, 10
        w = (x1 - x0 - 2 * pad - gap * (n - 1)) / n
        out[str(n)] = {"band": band, "slots": [[round(x0 + pad + k * (w + gap), 1), 814, round(x0 + pad + k * (w + gap) + w, 1), 898]
                                               for k in range(n)]}
    for n in (7, 8, 9, 10, 11, 12):
        band = [x0, 792, x1, 918]
        cols = 6 if n > 8 else 4
        rows = -(-n // cols)
        gap, pad = 6, 10
        w = (x1 - x0 - 2 * pad - gap * (cols - 1)) / cols
        h = (918 - 792 - 2 * 16 - gap * (rows - 1)) / rows
        slots = []
        for k in range(n):
            r_, c_ = divmod(k, cols)
            sx = x0 + pad + c_ * (w + gap)
            sy = 792 + 16 + r_ * (h + gap)
            slots.append([round(sx, 1), round(sy, 1), round(sx + w, 1), round(sy + h, 1)])
        out[str(n)] = {"band": band, "slots": slots}
    return out


CONTACT = {"max_slots": 12, "template_slots": 6, "frame_label": "contact_strip", "slot_labels": "ref_1..ref_N",
           "layouts": contact_layouts()}


# ---------------------------------------------------------------------------
# content
# ---------------------------------------------------------------------------

def a(p):
    return os.path.join(ASSETS, p)


def sk(sid, side):
    p = a("sk-%s-%s.png" % (sid.lower(), side))
    return p if os.path.exists(p) else a("sketch-nelly.png" if side == "left" else "sketch-waves.png")


def nel(p):
    return os.path.join(NELLY, p)


def load_cues():
    t = json.load(open(nel("nelly.transcript.json")))["paragraphs"]
    def tc(s):
        m, s2 = divmod(s, 60)
        return "%d:%05.2f" % (m, s2)
    def cue(i0, i1):
        txt = " / ".join(t[i]["text"].rstrip(",.") + ("" if k == i1 - i0 else "") for k, i in enumerate(range(i0, i1 + 1)))
        return "“%s”" % txt, "%s – %s" % (tc(t[i0]["start"]), tc(t[i1]["end"]))
    return t, cue


SCENES = [
    # id, title, section, bg, lyric lines, purpose, staging, note, printed action (handout), felt colour
    ("S01", "Ocean Stage", "Circle left", "stage_ocean/stage-ocean-v1.png", (0, 1),
     "Left is a direction we can move: Nelly circles left across the paper sea.",
     ["circle left (artist\u2019s)", "Nelly enters from page right", "keep waves and paper scale steady"],
     "first journey: circle left", "circle left", "Moss"),
    ("S02", "Mountain Top", "Turn the other way", "stage_mountain/stage-mountain-v1.png", (2, 3),
     "Going up and then coming back shows us how to turn and go the other way.",
     ["turn the other way (artist\u2019s)", "pause at the peak, then turn", "keep felt colours calm"],
     "up and over \u2014 then back to me", "turn the other way", "Navy"),
    ("S03", "Prairie", "Circle right", "stage_prairie/stage-prairie-v1.png", (4, 5),
     "Right is the opposite of left: Nelly circles right over wide open land.",
     ["circle right (artist\u2019s)", "horizon line stays low", "no extra props"],
     "big sky, long walk", "circle right", "Sun"),
    ("S04", "Desert Sands", "Stop", "stage_desert/stage-desert-v1.png", (6, 7),
     "Stopping is part of moving: Nelly comes back again, then stands still.",
     ["stop (artist\u2019s)", "Nelly returns to her start mark", "warm light only"],
     "come back again \u2014 and stop", "stop", "Rust"),
    ("S05", "Garden Jump", "Jump and turn", "stage_garden/stage-garden-v1.png", (8, 11),
     "Movement words become actions: jump up and down, then turn round and round.",
     ["jump, turn around, repeat (artist\u2019s)", "two small jumps, feet leave the ground", "slow turn, then repeat"],
     "jump \u2014 then spin!", "jump \u00b7 turn \u00b7 repeat", "Moss"),
]

OVERALL = ("Paper landscapes. Felt characters. Keep the handmade scale and material: tactile, delicate, playful. "
           "Soft studio light on a dimensional storybook stage.")

EXACT_NELLY = (
    "Draw ONE image on a 1024 x 1536 px canvas.\n"
    "Redraw the attached character exactly (same design, colours, clothing, materials) as one full-body front view, "
    "centred horizontally, about 1300 px tall, feet on a baseline at y = 1440.\n"
    "Animation-ready stance: standing upright, feet planted shoulder-width apart with a clear gap of background "
    "between the legs from the crotch to the floor, arms held slightly away from the body (about 20 degrees) with a "
    "clear gap between each arm and the body, hands open, head level. Every limb is fully visible and not "
    "overlapping another limb.\n"
    "Felt-plush storybook character art for 2D animation. Plain transparent background, no text, labels, frames, "
    "shadows or props. Nothing touches the canvas edge. Clean solid silhouette edge with no loose fibres or specks. "
    "Every drawing of the character keeps the same design, palette, proportions and scale.")

SCENE_PROMPT = ("Handmade felt-puppet storybook scene, 16:9. {who} on the open paper book stage, {loc}. Keep the stage "
                "exactly as in the reference: dimensional paper-cut set, tactile felt, soft studio light, shallow depth, "
                "no text, no extra characters.")


def scene_data(i, sc, cue, template, page_l, page_r):
    sid, title, section, bg, (l0, l1), purpose, staging, note, action, felt = sc
    q, tcode = cue(l0, l1)
    spread = 3 + i
    hero = nel("assets/backgrounds/" + bg)
    if sid == "S01" and os.path.exists(a("scene-ocean-nelly.png")):
        result = a("scene-ocean-nelly.png")
    else:
        result = a("scene-%s-nelly.png" % sid.lower())
    d = dict(
        spread=spread, page_left=page_l, page_right=page_r,
        running_title="NELLY / MAGIC STORYBOARD", running_sub="storyboard / scene study",
        badge="SCENE STUDY", section=section, hand_note="action: " + action,
        scene_id="SCENE " + sid[1:], title=title,
        hero=result if os.path.exists(result) else hero,
        cue=q, time=tcode, purpose=purpose, staging=staging,
        sketch_left=sk(sid, "left"), sketch_right=sk(sid, "right"),
        src_rot=[-3, 2.4, -2, 3, -2.6][i % 5], res_rot=[3, -2.2, 2.8, -3, 2.2][i % 5], felt=felt,
        revision="REV 01", overall_design=OVERALL,
        location_ref="%s — %s paper stage (L01 family), original plate %s" % (sid, title, os.path.basename(bg)),
        character_shared="Nelly — felt rabbit, yellow vest, red shorts; C01 shared across scenes.",
        character_lead="Nelly (lead): animation-ready pose C01, front view, 1024×1536.",
        asset_geometry="Canvas 1920×1080 (16:9). Nelly body ≈1300 px of 1536, feet y=1440; fit proportionally, never stretch.",
        source_image=hero, source_caption="SOURCE : " + title.upper(),
        result_image=result if os.path.exists(result) else None,
        result_caption="RESULT : SCENE %s (generated)" % sid[1:],
        match_note="match framing & angle — add Nelly",
        exact_prompt=SCENE_PROMPT.format(who="Nelly the felt rabbit (yellow vest, red shorts)",
                                         loc=title.lower() + " backdrop"),
        refs=[hero, result if os.path.exists(result) else hero, nel("assets/characters/nelly/reference.png"), nel("assets/characters/mouse/reference.png")],
        strip_note="contact strip — alt takes / expression / timing",
    )
    return d


def populated():
    t, cue = load_cues()
    nsp = 3 + len(SCENES)
    b = B(nsp)
    reg = [
        ("D01", "Overall design", "paper landscapes · felt characters", nel("assets/backgrounds/stage_book/stage-clean-book-v1.png")),
        ("L01", "Ocean Stage", "original background · 1672×941", nel("assets/backgrounds/stage_ocean/stage-ocean-v1.png")),
        ("C01", "Nelly", "character sheet → spread 3", nel("assets/characters/nelly/reference.png")),
        ("C02", "Mouse", "friend · shared cast", nel("assets/characters/mouse/reference.png")),
        ("C03", "Fox", "friend · shared cast", nel("assets/characters/fox/reference.png")),
        ("C04", "Bear", "friend · shared cast", nel("assets/characters/bear/reference.png")),
    ]
    spread_cover(b, dict(
        spread=0, kicker="MAGIC STUDIO / STORYBOARD", title="Nelly Go ’Cross\nthe Ocean",
        subtitle="a felt-puppet storybook stage · short educational video",
        key_art=a("cover-keyart.png"), key_caption="key art · Nelly crosses the paper ocean",
        badge="PRODUCTION BOOK", hand_note="\u201cThis is my first attempt at writing a singing game.\u201d \u2014 Kathy Reid-Naiman, More Tickles and Tunes",
        songmap=[(nel("assets/backgrounds/" + sc[3]) if not os.path.exists(a("scene-%s-nelly.png" % sc[0].lower())) else a("scene-%s-nelly.png" % sc[0].lower()), sc[0], sc[8]) for sc in SCENES],
        rev="REV 01", tagline="A singing-game journey from Kathy Reid-Naiman’s “More Tickles and Tunes”, told on a miniature paper stage.",
        overall_design=OVERALL + " D01 governs every scene: nothing glossy, nothing digital-flat.",
        format_notes="Short video (~1\u20132 min), 16:9, 1920\u00d71080. One scene per printed action; each teaches one idea "
                     "(direction, stop, jump). The five scenes cover the song\u2019s first pass (0:02\u20130:27); it repeats after the "
                     "instrumental break. Lyrics are as printed; timings are transcript estimates to confirm against the audio.",
        register=reg, footer_note="Every reference keeps its source, its prompt and its revision trail.",
    ))
    spread_world(b, dict(
        spread=1, running_title="NELLY / MAGIC STORYBOARD", heading="THE WORLD / L01", subheading="An ocean made of paper",
        badge="LOOK DEVELOPMENT", stage_image=nel("assets/backgrounds/stage_ocean/stage-ocean-v1.png"),
        stage_caption="Ocean Stage / original source · 1672 × 941 px",
        material_scale="Felt characters on paper-cut scenery standing on an open storybook. Palette: cobalt, sky blue, gingham cream, warm "
                       "walnut table. Scale: stage waves ≈ 1/3 of the book width; characters stand ~1 book-page tall.",
        hand_note="→ Keep the approved source with its prompt.",
        register="L01  Ocean Stage\nSource: stage-ocean-v1.png\nOriginal: 1672 × 941 px · Scene composition: 1920 × 1080 px",
        location_prompt="Dimensional paper-cut ocean stage on an open handmade storybook: layered cobalt and sky-blue waves with white "
                        "foam curls, one gingham swell, cream paper tabs holding the cut-outs upright. Walnut tabletop, plain "
                        "powder-blue backdrop, soft studio light, tactile, shallow depth, no text, no characters.",
        revision="v1 → composite: add Nelly on the open book; keep the stage unchanged. Selected result: scene-ocean-nelly.png. "
                 "Earlier prompt kept; exact revision appended.",
        source_image=nel("assets/backgrounds/stage_ocean/stage-ocean-v1.png"), source_caption="SOURCE : stage-ocean-v1",
        result_image=a("scene-ocean-nelly.png"), result_caption="RESULT : SCENE 01",
        match_note="→ New art starts from this source and its references.",
        fit_note="Fit proportionally. Approve any crop. Never stretch.",
    ))
    spread_character(b, dict(
        spread=2, running_title="NELLY / MAGIC STORYBOARD", heading="CHARACTER / C01", subheading="Original → animation pose",
        badge="CHARACTER STUDY", prompt_id="P01",
        original_image=nel("assets/characters/nelly/front/nelly-front-drawing.png"), original_caption="original drawing · 1024 × 1200 px",
        pose_image=nel("assets/characters/nelly/front/nelly-front-body-redraw.png"), pose_caption="saved result · 1024 × 1536 px",
        hand_note="→ Same design. Clear limbs. Room to move.", character_tag="C01 · NELLY · FELT RABBIT",
        silhouette_rules="SILHOUETTE RULES\nlimbs fully visible\nno limb overlaps another\nclear gap between legs\nclear gap beside each arm\nnothing touches canvas edge\nno loose fibres or specks", exact_prompt=EXACT_NELLY, prompt_foot="Original drawing → generated asset\nGenerated and attached · visual review still required",
        geometry="2:3 canvas · 1024 × 1536 px · requested body ≈ 1300 px · feet baseline y = 1440\n"
                 "Clean silhouette: limbs separated, no overlaps, nothing touches canvas edge.\nBody size and baseline are requests, not measured output.",
    ))
    pages = [("07", "08"), ("09", "10"), ("11", "12"), ("13", "14"), ("15", "16")]
    for i, sc in enumerate(SCENES):
        spread_scene(b, scene_data(i, sc, cue, False, *pages[i]))
    return b


def template():
    b = B(4)
    reg = [("[ID]", "[Reference name]", "[source / note]", None)] * 6
    spread_cover(b, dict(
        spread=0, kicker="[PROJECT] / STORYBOARD", title="[Project title]", subtitle="[subtitle / format]", key_art=None,
        key_caption="[key art caption]", badge="PRODUCTION BOOK", hand_note="[artist note, verbatim]", songmap=[(None, "S0%d" % k, "[action]") for k in range(1, 6)], rev="REV 00",
        tagline="[one-line premise]", overall_design="[D01 overall design prompt]", format_notes="[short-video format notes]",
        register=reg, footer_note="[footer note]"))
    spread_world(b, dict(
        spread=1, running_title="[PROJECT] / STORYBOARD", heading="THE WORLD / L01", subheading="[location tagline]",
        badge="LOOK DEVELOPMENT", stage_image=None, stage_caption="[location / original source]",
        material_scale="[materials, palette, scale]", hand_note="[handwritten note]", register="[L01 name / source / sizes]",
        location_prompt="[Exact location prompt, verbatim]", revision="[What changed, why, which result is selected.]",
        source_image=None, source_caption="SOURCE : [location]", result_image=None, result_caption="RESULT : [scene]",
        match_note="[how the result matches the source]", fit_note="Fit proportionally. Approve any crop. Never stretch."))
    spread_character(b, dict(
        spread=2, running_title="[PROJECT] / STORYBOARD", heading="CHARACTER / C01", subheading="Original → animation pose",
        badge="CHARACTER STUDY", prompt_id="P01", original_image=None, original_caption="original drawing · [w × h]",
        pose_image=None, pose_caption="saved result · 1024 × 1536 px", hand_note="[handwritten note]",
        character_tag="C01 · [NAME]", silhouette_rules="SILHOUETTE RULES\n[rule 1]\n[rule 2]\n[rule 3]", exact_prompt="[Saved exact prompt, verbatim]", prompt_foot="[provenance / review status]",
        geometry="2:3 canvas · 1024 × 1536 px · body ≈ 1300 px · feet baseline y = 1440\nClean silhouette: limbs separated, no overlaps, nothing touches canvas edge."))
    spread_scene(b, dict(
        spread=3, page_left="00", page_right="00", running_title="[PROJECT] / STORYBOARD", running_sub="storyboard / scene study",
        badge="SCENE STUDY", section="[Section]", hand_note="[handwritten note]", scene_id="SCENE [ID]", title="[Scene title]",
        hero=None, cue="“[Exact lyric / audio cue]”", time="[mm:ss – mm:ss]", purpose="[One sentence: what we see, learn or feel.]",
        staging=["[staging note 1]", "[staging note 2]", "[staging note 3]"], sketch_left=None, sketch_right=None,
        revision="REV 00", overall_design="[Overall design summary]", location_ref="[Location / original]",
        character_shared="[Shared character]", character_lead="[Individual character]", asset_geometry="[Canvas, body height, baseline, props]",
        source_image=None, source_caption="SOURCE : [location]", result_image=None, result_caption="RESULT : [scene] (generated)",
        match_note="[match note]", exact_prompt="[Approved prompt, unchanged. Exact revision appended with the resulting image.]",
        refs=[], strip_note="contact strip — add or remove slots as needed"))
    return b


if __name__ == "__main__":
    mode = sys.argv[1]
    b = populated() if mode == "populated" else template()
    out = os.path.join(HERE, "nelly-storyboard.dcs" if mode == "populated" else "studio-template.dcs")
    with open(out, "w") as f:
        json.dump(b.lines, f)
    if mode == "template":
        build = {
            "name": "Magic Studio storyboard master template",
            "page": {"width": W, "height": H, "note": "one page == one open-book spread (left page x 0-800, right page x 800-1600)"},
            "spreads": [{"index": i, "role": r} for i, r in enumerate(["cover", "world", "character", "scene"])],
            "scene_spread_index": 3,
            "labels": b.labels,
            "contact_strip": CONTACT,
            "workflow": [
                "scene_fill.py studio-template.designcraft scene.json out.designcraft  (duplicates the scene spread and fills every label)",
                "labels are object.label values; find with object.findByLabel; text frames take story.setText, picture frames place.load + place.drop + object.fit",
            ],
        }
        json.dump(build, open(os.path.join(HERE, "studio-template-build.json"), "w"), indent=2)
    print(out, len(b.lines), "steps")
