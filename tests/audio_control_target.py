"""Locate the painted plus/minus glyph beneath an observed audio-level line.

The anchor comes from OCR of the current capture. Only the two level buttons'
column and next control row are inspected; no fixed vertical coordinate is used.
"""
from PIL import Image


def level_glyph(image_path, anchor_bottom, word):
    assert word in ['+', '−']
    with Image.open(image_path) as image:
        assert image.size == (720, 1440), 'Require verified portrait geometry'
        pixels = image.convert('RGB').load()
        region = (40, anchor_bottom + 1, 350, min(1300, anchor_bottom + 110))
        left, top, right, bottom = region
        foreground = {(x, y) for y in range(top, bottom) for x in range(left, right)
                      if min(pixels[x, y]) > 170}
    glyphs = {'+': [], '−': []}
    while foreground:
        pending = [foreground.pop()]
        component = set(pending)
        while pending:
            x, y = pending.pop()
            for dx, dy in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
                point = (x + dx, y + dy)
                if point in foreground:
                    foreground.remove(point)
                    component.add(point)
                    pending.append(point)
        x0, x1 = min(x for x, _ in component), max(x for x, _ in component)
        y0, y1 = min(y for _, y in component), max(y for _, y in component)
        width, height = x1 - x0 + 1, y1 - y0 + 1
        if not (12 <= width <= 32 and 1 <= height <= 32):
            continue
        horizontal = max(sum((x, y) in component for x in range(x0, x1 + 1)) for y in range(y0, y1 + 1))
        vertical = max(sum((x, y) in component for y in range(y0, y1 + 1)) for x in range(x0, x1 + 1))
        minus = height <= 6 and horizontal >= width - 1
        plus = .7 <= width / height <= 1.3 and height >= 12 and horizontal >= width - 1 and vertical >= height - 1
        glyph = dict(bounds=[x0, y0, width, height], point=[(x0 + x1) / 4, (y0 + y1) / 4])
        if minus: glyphs['−'].append(glyph)
        if plus: glyphs['+'].append(glyph)
    pairs = [(minus, plus) for minus in glyphs['−'] for plus in glyphs['+']
             if 50 <= plus['point'][0] - minus['point'][0] <= 90
             and abs(plus['point'][1] - minus['point'][1]) <= 2]
    assert len(pairs) == 1, dict(word=word, region=region, candidates=glyphs)
    chosen = pairs[0][0 if word == '−' else 1]
    return dict(word=word, region=region, **chosen)
