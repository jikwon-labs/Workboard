#!/usr/bin/env python3
# Makes Resources/EmojiJoiners.ttf: a tiny font in which the characters that
# only join or change an emoji (U+FE0E/U+FE0F variation selectors, the zero
# width joiner, skin tones, tag letters) are empty and take no space. egui's
# own emoji font gives each of them a full emoji-wide box, so a red heart
# (U+2764 U+FE0F) or a thumbs up with a skin tone took two emoji of room.
# src/app.rs (setup_fonts) puts this font before the emoji fonts.
#
# Needs fontTools (pip install fonttools). Run from the repository root:
#     python3 scripts/make-emoji-joiners-font.py
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

chars = [0xFE0E, 0xFE0F, 0x200D]
chars += list(range(0x1F3FB, 0x1F400))  # skin tones
chars += list(range(0xE0020, 0xE0080))  # tag letters (flags of England etc.)

fb = FontBuilder(1000, isTTF=True)
fb.setupGlyphOrder([".notdef", "empty"])
fb.setupCharacterMap({c: "empty" for c in chars})
empty = TTGlyphPen(None).glyph()
fb.setupGlyf({".notdef": empty, "empty": TTGlyphPen(None).glyph()})
fb.setupHorizontalMetrics({".notdef": (0, 0), "empty": (0, 0)})
fb.setupHorizontalHeader(ascent=800, descent=-200)
fb.setupNameTable({"familyName": "Workboard Emoji Joiners", "styleName": "Regular"})
fb.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
fb.setupPost()
fb.save("Resources/EmojiJoiners.ttf")
