"""Minimal Pillow-based drawing toolkit for the Pacenotes architecture diagrams.

No graphviz, no mermaid, no browser: every diagram in this folder is laid out by
explicit coordinates in Python and rendered with Pillow, so it can be regenerated
on any machine that has the Python runtime and nothing else.

Conventions used by every diagram module:
  * coordinates are top-left based, in pixels;
  * text is measured with the real font, so boxes are sized to their content;
  * arrows are polylines with an arrowhead on the last segment.
"""

import math
import os

from PIL import Image, ImageDraw, ImageFont

FONT_DIR = os.path.join(os.environ.get("WINDIR", r"C:\Windows"), "Fonts")

# Fallbacks, tried in order, so the script also runs outside Windows.
FONT_FALLBACKS = {
    "segoeui.ttf": ["segoeui.ttf", "DejaVuSans.ttf", "arial.ttf"],
    "seguisb.ttf": ["seguisb.ttf", "segoeui.ttf", "DejaVuSans.ttf"],
    "segoeuib.ttf": ["segoeuib.ttf", "segoeui.ttf", "DejaVuSans-Bold.ttf"],
    "consola.ttf": ["consola.ttf", "DejaVuSansMono.ttf", "cour.ttf"],
    "consolab.ttf": ["consolab.ttf", "consola.ttf", "DejaVuSansMono-Bold.ttf"],
    "segoeuii.ttf": ["segoeuii.ttf", "segoeui.ttf"],
}


class Ink:
    """The palette. One place, so every diagram shares a visual language."""

    page = "#FFFFFF"
    ink = "#1B2430"
    muted = "#5B6675"
    faint = "#8A94A4"
    rule = "#C9D2DE"
    line = "#6F7B8B"

    zone_fill = "#F7F9FC"
    zone_edge = "#D3DCE7"

    server = "#2B5F8F"
    server_fill = "#E8F0F9"
    auth = "#6C3F8F"
    auth_fill = "#F1EAF8"
    protocol = "#2F7A5C"
    protocol_fill = "#E7F4EE"
    client = "#A7642C"
    client_fill = "#FBF0E4"
    disk = "#4A5568"
    disk_fill = "#EEF1F5"

    action_fill = "#EAF1FA"
    action_edge = "#7FA0C6"
    decision_fill = "#FFF6E3"
    decision_edge = "#D9A94B"
    note_fill = "#FFFBE6"
    note_edge = "#D6C26B"
    ok = "#2F7A5C"
    warn = "#A7642C"
    bad = "#A63A3A"
    link = "#8E3F5B"


class Kit:
    def __init__(self, width, height, bg=Ink.page):
        self.width = width
        self.height = height
        self.img = Image.new("RGB", (width, height), bg)
        self.d = ImageDraw.Draw(self.img)
        self._fonts = {}
        self.boxes = {}

    # ------------------------------------------------------------------ text

    def font(self, name, size):
        key = (name, size)
        if key in self._fonts:
            return self._fonts[key]
        chosen = None
        for candidate in FONT_FALLBACKS.get(name, [name]):
            path = os.path.join(FONT_DIR, candidate)
            if os.path.exists(path):
                try:
                    chosen = ImageFont.truetype(path, size)
                    break
                except OSError:
                    continue
        if chosen is None:
            chosen = ImageFont.load_default()
        self._fonts[key] = chosen
        return chosen

    def reg(self, size):
        return self.font("segoeui.ttf", size)

    def semi(self, size):
        return self.font("seguisb.ttf", size)

    def bold(self, size):
        return self.font("segoeuib.ttf", size)

    def mono(self, size):
        return self.font("consola.ttf", size)

    def monob(self, size):
        return self.font("consolab.ttf", size)

    def tw(self, text, font):
        return self.d.textlength(text, font=font)

    def text(self, x, y, s, font, fill=Ink.ink, anchor="la"):
        self.d.text((x, y), s, font=font, fill=fill, anchor=anchor)

    def wrap(self, s, font, max_width):
        """Word wrap per line; falls back to hard breaks for long unbroken tokens."""
        if "\n" in s:
            out = []
            for line in s.split("\n"):
                out.extend(self.wrap(line, font, max_width))
            return out
        words = s.split(" ")
        lines, current = [], ""
        for word in words:
            trial = word if not current else current + " " + word
            if self.tw(trial, font) <= max_width:
                current = trial
                continue
            if current:
                lines.append(current)
            if self.tw(word, font) <= max_width:
                current = word
            else:
                chunk = ""
                for ch in word:
                    if self.tw(chunk + ch, font) <= max_width:
                        chunk += ch
                    else:
                        lines.append(chunk)
                        chunk = ch
                current = chunk
        if current:
            lines.append(current)
        return lines or [""]

    def paragraph(self, x, y, s, font, max_width, fill=Ink.ink, leading=1.35, anchor="la"):
        line_h = font.size * leading
        lines = self.wrap(s, font, max_width)
        for i, line in enumerate(lines):
            self.d.text((x, y + i * line_h), line, font=font, fill=fill, anchor=anchor)
        return len(lines) * line_h

    # ----------------------------------------------------------------- shapes

    def rrect(self, x, y, w, h, radius=10, fill=None, outline=None, width=1.4):
        self.d.rounded_rectangle(
            [x, y, x + w, y + h], radius=radius, fill=fill, outline=outline, width=int(round(width))
        )

    def rect(self, x, y, w, h, fill=None, outline=None, width=1.4):
        self.d.rectangle([x, y, x + w, y + h], fill=fill, outline=outline, width=int(round(width)))

    def poly(self, points, fill=None, outline=None, width=1.4):
        self.d.polygon(points, fill=fill)
        if outline:
            self.d.line(list(points) + [points[0]], fill=outline, width=int(round(width)), joint="curve")

    def _dash_segment(self, p0, p1, color, width, dash=7, gap=5):
        dx, dy = p1[0] - p0[0], p1[1] - p0[1]
        length = math.hypot(dx, dy)
        if length == 0:
            return
        ux, uy = dx / length, dy / length
        pos = 0.0
        while pos < length:
            end = min(pos + dash, length)
            self.d.line(
                [(p0[0] + ux * pos, p0[1] + uy * pos), (p0[0] + ux * end, p0[1] + uy * end)],
                fill=color,
                width=int(round(width)),
            )
            pos = end + gap

    def dashed(self, points, color=Ink.line, width=1.6, dash=8, gap=6):
        for i in range(len(points) - 1):
            self._dash_segment(points[i], points[i + 1], color, width, dash, gap)

    def arrowhead(self, p0, p1, color, size=10):
        angle = math.atan2(p1[1] - p0[1], p1[0] - p0[0])
        a = angle + math.radians(152)
        b = angle - math.radians(152)
        left = (p1[0] + size * math.cos(a), p1[1] + size * math.sin(a))
        right = (p1[0] + size * math.cos(b), p1[1] + size * math.sin(b))
        self.poly([p1, left, right], fill=color, outline=color, width=1)

    def arrow(
        self,
        points,
        color=Ink.line,
        width=1.9,
        dash=None,
        head=True,
        head_size=10,
        label=None,
        label_font=None,
        label_color=None,
        label_at=0.5,
        label_offset=(0, 0),
        label_align="mid",
    ):
        if dash:
            self.dashed(points, color, width, dash, dash * 0.7)
        else:
            self.d.line(points, fill=color, width=int(round(width)), joint="curve")
        if head:
            self.arrowhead(points[-2], points[-1], color, head_size)
        if label:
            font = label_font or self.reg(13)
            # point at the requested fraction along the whole polyline
            total = sum(math.hypot(points[i + 1][0] - points[i][0], points[i + 1][1] - points[i][1])
                        for i in range(len(points) - 1))
            want = total * label_at
            walked = 0.0
            pos = points[0]
            for i in range(len(points) - 1):
                seg = math.hypot(points[i + 1][0] - points[i][0], points[i + 1][1] - points[i][1])
                if walked + seg >= want and seg > 0:
                    t = (want - walked) / seg
                    pos = (points[i][0] + (points[i + 1][0] - points[i][0]) * t,
                           points[i][1] + (points[i + 1][1] - points[i][1]) * t)
                    break
                walked += seg
            self.tag(pos[0] + label_offset[0], pos[1] + label_offset[1], label, font,
                     label_color or color, align=label_align)

    def tag(self, x, y, s, font, color=Ink.ink, bg=None, pad=3, align="mid"):
        bg = bg or Ink.page
        w = self.tw(s, font)
        h = font.size * 1.32
        if align == "mid":
            left = x - w / 2
        elif align == "left":
            left = x + 2
        else:
            left = x - w - 2
        self.rrect(left - pad, y - h / 2, w + pad * 2, h, radius=4, fill=bg, outline=None)
        self.d.text((left, y - h / 2 + font.size * 0.12), s, font=font, fill=color)

    # ------------------------------------------------------------- structures

    def zone(self, x, y, w, h, title, subtitle=None, accent=Ink.server, fill=Ink.zone_fill,
             edge=Ink.zone_edge, dashed_border=False):
        if dashed_border:
            self.rrect(x, y, w, h, radius=14, fill=fill, outline=None)
            # dashed rounded border: approximate with a dashed rectangle
            self.dashed([(x + 12, y), (x + w - 12, y)], edge, 1.6, 9, 6)
            self.dashed([(x + w, y + 12), (x + w, y + h - 12)], edge, 1.6, 9, 6)
            self.dashed([(x + 12, y + h), (x + w - 12, y + h)], edge, 1.6, 9, 6)
            self.dashed([(x, y + 12), (x, y + h - 12)], edge, 1.6, 9, 6)
        else:
            self.rrect(x, y, w, h, radius=14, fill=fill, outline=edge, width=1.5)
        self.d.rectangle([x + 1, y + 1, x + 5, y + h - 1], fill=accent)
        self.text(x + 20, y + 14, title, self.semi(17), accent)
        if subtitle:
            self.text(x + 20, y + 38, subtitle, self.reg(12), Ink.muted)

    def er_entity(self, x, y, w, name, rows, subtitle=None, accent=Ink.server,
                  fill="#FFFFFF", derived=False, tag=None):
        """An entity box: name header, optional subtitle, one row per attribute.

        rows: (marker, attribute, type). marker is "PK", "FK", "PK,FK" or "".
        """
        header_h = 34
        sub_h = 18 if subtitle else 0
        row_h = 21
        h = header_h + sub_h + row_h * len(rows) + 8
        edge = accent
        if derived:
            self.rrect(x, y, w, h, radius=8, fill=fill, outline=None)
            self.dashed([(x + 8, y), (x + w - 8, y)], edge, 1.8, 8, 5)
            self.dashed([(x + w, y + 8), (x + w, y + h - 8)], edge, 1.8, 8, 5)
            self.dashed([(x + 8, y + h), (x + w - 8, y + h)], edge, 1.8, 8, 5)
            self.dashed([(x, y + 8), (x, y + h - 8)], edge, 1.8, 8, 5)
        else:
            self.rrect(x, y, w, h, radius=8, fill=fill, outline=edge, width=1.6)
        # header band
        self.d.rectangle([x + 1, y + 1, x + w - 1, y + header_h], fill=accent)
        self.text(x + 12, y + 8, name, self.monob(14), "#FFFFFF")
        if tag:
            self.text(x + w - 12, y + 9, tag, self.reg(11), "#FFFFFF", anchor="ra")
        yy = y + header_h + 1
        if subtitle:
            self.text(x + 12, yy + 2, subtitle, self.reg(11), Ink.faint)
            yy += sub_h
        for index, (marker, attr, kind) in enumerate(rows):
            if index % 2 == 1:
                self.d.rectangle([x + 1, yy, x + w - 1, yy + row_h], fill="#F6F8FB")
            if marker:
                self.text(x + 10, yy + 4, marker, self.bold(9), Ink.warn if "PK" in marker else Ink.server)
            self.text(x + 52, yy + 3, attr, self.mono(11.5), Ink.ink)
            if kind:
                self.text(x + w - 10, yy + 4, kind, self.mono(10), Ink.faint, anchor="ra")
            yy += row_h
        self.d.line([(x + 1, y + header_h + sub_h + 4), (x + w - 1, y + header_h + sub_h + 4)],
                    fill=accent, width=1)
        box = (x, y, w, h)
        self.boxes[name] = box
        return h

    def anchor(self, name, side, frac=0.5):
        x, y, w, h = self.boxes[name]
        if side == "left":
            return (x, y + h * frac)
        if side == "right":
            return (x + w, y + h * frac)
        if side == "top":
            return (x + w * frac, y)
        if side == "bottom":
            return (x + w * frac, y + h)

    def relation(self, a, side_a, b, side_b, frac_a=0.5, frac_b=0.5, label="", card_a="",
                 card_b="", color=Ink.line, dashed=False, route=None, label_offset=(0, 0),
                 label_at=0.5, font_size=12):
        """Draw a relationship and name it.

        Multiplicity is written INTO the label, as "role (1 -> 0..*)": hanging
        separate 1 / * markers off a 26 px gap puts them on top of the entity
        they belong to, and the gaps between stacked entities are that small.
        """
        p0 = self.anchor(a, side_a, frac_a)
        p1 = self.anchor(b, side_b, frac_b)
        points = route or self.route(p0, side_a, p1, side_b)
        text = label
        if card_a and card_b:
            text = (label + "  " if label else "") + "(%s \u2192 %s)" % (card_a, card_b)
        elif card_a or card_b:
            text = (label + "  " if label else "") + "(%s)" % (card_a or card_b)
        self.arrow(points, color=color, width=1.5, dash=6 if dashed else None, head=True,
                   head_size=9, label=text or None, label_font=self.reg(font_size),
                   label_at=label_at, label_offset=label_offset)

    def _cardinality(self, anchor, away, text, color):
        """Multiplicity written just OUTSIDE the entity, on the line's side."""
        dx, dy = anchor[0] - away[0], anchor[1] - away[1]
        length = math.hypot(dx, dy) or 1
        ux, uy = dx / length, dy / length
        x, y = anchor[0] - ux * 13, anchor[1] - uy * 13
        px, py = -uy, ux
        self.tag(x + px * 11, y + py * 11, text, self.bold(11), color, align="mid")

    def route(self, p0, side_a, p1, side_b):
        """Orthogonal 2-3 segment route between two anchors."""
        gap = 22
        out_a = {"left": (-1, 0), "right": (1, 0), "top": (0, -1), "bottom": (0, 1)}[side_a]
        in_b = {"left": (-1, 0), "right": (1, 0), "top": (0, -1), "bottom": (0, 1)}[side_b]
        a = (p0[0] + out_a[0] * gap, p0[1] + out_a[1] * gap)
        b = (p1[0] + in_b[0] * gap, p1[1] + in_b[1] * gap)
        horizontal_a = out_a[1] == 0
        horizontal_b = in_b[1] == 0
        if horizontal_a and horizontal_b:
            mid = (a[0] + b[0]) / 2 if abs(a[0] - b[0]) > 40 else a[0]
            return [p0, a, (mid, a[1]), (mid, b[1]), b, p1]
        if not horizontal_a and not horizontal_b:
            mid = (a[1] + b[1]) / 2 if abs(a[1] - b[1]) > 40 else a[1]
            return [p0, a, (a[0], mid), (b[0], mid), b, p1]
        if horizontal_a and not horizontal_b:
            return [p0, a, (b[0], a[1]), b, p1]
        return [p0, a, (a[0], b[1]), b, p1]

    # --------------------------------------------------------------- activity

    def start(self, cx, cy, r=11):
        self.d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=Ink.ink, outline=Ink.ink)

    def end(self, cx, cy, r=13):
        self.d.ellipse([cx - r, cy - r, cx + r, cy + r], fill="#FFFFFF", outline=Ink.ink, width=2)
        self.d.ellipse([cx - r + 5, cy - r + 5, cx + r - 5, cy + r - 5], fill=Ink.ink)

    def action(self, cx, y, w, text, tag=None, accent=Ink.action_edge, fill=Ink.action_fill,
               font_size=13, min_h=44, pad=(14, 11), emphasis=False):
        font = self.semi(font_size) if emphasis else self.reg(font_size)
        inner = w - pad[0] * 2
        lines = self.wrap(text, font, inner)
        h = max(min_h, len(lines) * font.size * 1.32 + pad[1] * 2 + (14 if tag else 0))
        x = cx - w / 2
        self.rrect(x, y, w, h, radius=9, fill=fill, outline=accent, width=1.5)
        ty = y + pad[1]
        if tag:
            self.text(x + 10, ty - 1, tag, self.bold(9), accent)
            ty += 14
        line_h = font.size * 1.32
        for i, line in enumerate(lines):
            self.text(cx, ty + i * line_h, line, font, Ink.ink, anchor="ma")
        return (x, y, w, h)

    def decision(self, cx, y, w, text, font_size=13, h=None, fill=Ink.decision_fill,
                 edge=Ink.decision_edge):
        font = self.reg(font_size)
        lines = self.wrap(text, font, w * 0.62)
        h = h or max(70, len(lines) * font.size * 1.3 + 40)
        x = cx - w / 2
        self.poly([(cx, y), (x + w, y + h / 2), (cx, y + h), (x, y + h / 2)],
                  fill=fill, outline=edge, width=1.8)
        line_h = font.size * 1.3
        top = y + h / 2 - (len(lines) * line_h) / 2
        for i, line in enumerate(lines):
            self.text(cx, top + i * line_h, line, font, Ink.ink, anchor="ma")
        return (x, y, w, h)

    def note(self, x, y, w, text, font_size=12, fill=Ink.note_fill, edge=Ink.note_edge):
        font = self.reg(font_size)
        lines = self.wrap(text, font, w - 24)
        h = len(lines) * font.size * 1.3 + 20
        self.d.polygon([(x, y), (x + w - 14, y), (x + w, y + 14), (x + w, y + h), (x, y + h)],
                       fill=fill, outline=edge)
        self.d.line([(x + w - 14, y), (x + w - 14, y + 14), (x + w, y + 14)], fill=edge, width=1)
        for i, line in enumerate(lines):
            self.text(x + 10, y + 9 + i * font.size * 1.3, line, font, Ink.ink)
        return (x, y, w, h)

    def fork_bar(self, cx, y, width=220, fill=Ink.ink):
        self.d.rectangle([cx - width / 2, y, cx + width / 2, y + 7], fill=fill)
        return y + 7

    # ------------------------------------------------------------------ chrome

    def title(self, x, y, text, subtitle=None, accent=Ink.ink):
        self.text(x, y, text, self.semi(27), accent)
        if subtitle:
            self.paragraph(x, y + 40, subtitle, self.reg(13), self.width - 2 * x, Ink.muted, 1.4)

    def legend(self, x, y, items, font_size=11.5, gap=22, columns=1):
        """items: (colour, label). Drawn as colour swatch + text."""
        font = self.reg(font_size)
        col = 0
        row = 0
        per_col = math.ceil(len(items) / columns) if columns > 1 else len(items)
        for index, (color, label) in enumerate(items):
            if index and index % per_col == 0:
                col += 1
                row = 0
            cx = x + col * 290
            cy = y + row * gap
            self.rrect(cx, cy + 2, 13, 13, radius=3, fill=color, outline=None)
            self.text(cx + 21, cy, label, font, Ink.muted)
            row += 1

    def save(self, path):
        self.img.save(path, optimize=True)
        return path
