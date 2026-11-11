# Kitah design rules

One file for every lesson and every screen (kid sign-in, lesson stages, race board, projector, teacher app).
A new screen follows these rules; a rule changes only when Aviv changes it, and then here first.

## Design language

**Status: waiting for Aviv's pick.** Nothing below this line chooses a look for him. He asked to brainstorm and pick
the design language, the icon set and the logo together, from options, never chosen alone. Three candidates, each
shown whole (colours, letters, icons, two logo ideas, the class flow and two live demos) at
[mockup/signin/](https://vaknin.github.io/nature-words/mockup/signin/) (source: mockup/signin/template.html, built by
tools/mockup-build.mjs):

| | Look | Feel | Headings + text | Icons | Colours |
|---|---|---|---|---|---|
| A | Sticker Pop | Thick outlines, offset shadows, like stickers on a laptop lid | Fredoka + Rubik | Phosphor duotone (MIT): 10 animals + 6 nature glyphs | cream #fff6ea, ink #2a1f4d, coral #ff5d5d, violet #7a4dff, sun #ffc93c, teal #16b8a6 |
| B | Field Trip | An explorer's notebook: paper, line icons, stamps; the calmest | Secular One + Varela Round | Tabler outline (MIT): 12 animals + 4 plants | paper #f3efe2, forest #173b2f, moss #2f8f5b, sky #2d6fb8, clay #e3793a, straw #e8c547 |
| C | Arcade Night | A game's start screen: dark, neon, bold silhouettes; the most "game" | Karantina + Rubik | game-icons.net (CC BY 3.0, every author credited): 16 real animals | night #16122e, white #f3f0ff, pink #ff4fa3, cyan #38e1e8, lime #c4f542, amber #ffb347 |

When he picks (or mixes), the chosen row replaces this table, with his words and the page they stand on.

Decided, whichever look wins:

- **Tone: 5th grade, 10-11 years.** Not too childish, not too grown-up (his words; the evidence and the table of what
  each means is in research/kid-ux.md, section Tone). One heavy rounded or bold face for headings, a plain sans for
  text, body at least 16 px. One dark ink colour plus 2-3 saturated accents, never pure primaries. Rounded cards,
  thick outlines or offset shadows. Short imperative wording: "Tap your name", "Pick your buddy". Animal glyphs are
  the kid's own buddy, never a narrator talking down to them.
- **No emoji, anywhere**: text, buttons, titles, alt text, attributes. They render differently on every device and
  read as babyish. Pictures are open-source icon glyphs only (see Icons). Avoid the copyright, registered and
  trademark signs too: they count as emoji characters in the checks.
- **English and Hebrew text.** Every instruction a kid reads is simple English with the Hebrew beside it; Hebrew text
  sits in its own element marked `lang="he" dir="rtl"`. Every font chosen must carry both scripts (all five above do).
- **English voices only.** Recorded or spoken audio is English; Hebrew is on screen, never spoken.
- **A full screen button with a hint** on every kid and projector screen: the button toggles full screen and the hint
  beside it says F11 works too and Esc leaves.
- **Marked do-not-translate**: `<html translate="no" class="notranslate">` and `<meta name="google"
  content="notranslate">`, so the browser never offers to translate the English the kids are learning.
- **Names**: a kid's first name, or first name plus family initial when two share it ("Maya R.", "Maya S."). No full
  names, no emails.
- **Class only, for now**: one 5th-grade class signs in during the lesson, from the class code on the projector. No
  sign-in outside the lesson and no link out to another platform on any kid screen.
- **Guests can play**: a kid who is not signed in plays every stage; only signed-in kids appear on the leaderboard, and
  a guest is told so in one calm line, without a lock or red.
- **Secret buddy**: an animal from a 4x4 grid, then one of 6 named colours (red, orange, yellow, green, blue, purple,
  each with its English and Hebrew word): 96 choices. Made as a short ceremony (pick, paint, reveal, once more from a
  reshuffled grid, saved). The grid reshuffles every time; the buddy is never shown after sign-in.
- **Laptops only**: screens are built and checked at 1366x768, the class laptops (his word: laptop only).
- **Self-contained pages**: fonts and glyphs are inlined; a page loads nothing from outside at run time.

## Icons

- Glyphs come from one open-source SVG icon set per look (the candidates: Phosphor, MIT; Tabler, MIT; game-icons.net,
  CC BY 3.0). One set across the whole product once he picks; no mixing sets on a screen.
- Inline only the glyphs a page uses, as `<symbol>` elements with `<use>` references, coloured by `currentColor`.
- Credit the set's license on the page. CC BY sets credit every icon's author by name (the list the build reads is
  tools/game-icons-authors.txt).
- Every glyph a kid must recognise or remember carries its word (English) under it; colour is never the only cue
  (about 1 boy in a class of 30 is red-green colour blind).
- Show a set as it really is: if it lacks 16 distinct animals, the grid fills with nature glyphs from the same set
  and the mockup says so.

## Motion

- Motion explains a change; it never blocks a tap. Durations come from research/kid-ux.md (NN/g 100-400 ms, Material 3
  tokens):
  - A thing leaving: 250 ms, emphasized-accelerate `cubic-bezier(.3,0,.8,.15)`.
  - Neighbours closing a gap: 200 ms (FLIP, transform only).
  - Things arriving: up to 350 ms, emphasized-decelerate `cubic-bezier(.05,.7,.1,1)`.
  - A celebration bounce (squash and stretch): up to 450 ms.
- **Taken name**: the name stops working on the first tap, pops with a comic "POP!" burst for 250 ms, is removed on its
  own `animationend` (events from pseudo-elements and children ignored), then the list closes up in 200 ms.
- **Reduced motion fallback, always**: every animation sits under a `prefers-reduced-motion: reduce` rule that turns it
  off, and the script checks the same query, so the change happens at once on the same tap (a taken name leaves
  immediately; a buddy is painted without the bounce).
- Animate only transform and opacity, so the class's laptops stay smooth.
- Every motion rule is checked in a real browser (test/mockup.mjs): the animation starts, the element is gone within
  its duration plus a measured margin, and with reduced motion it is gone at once.
