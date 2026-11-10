# Design pass and polish review

## Design pass

What changed across every kid screen, and why:

- **Fonts:** Fredoka (rounded, playful, has Hebrew) for headings, words and buttons; Rubik (clean, made for Hebrew
  and Latin) for running text and every Hebrew line. Both are embedded in the page, so a school laptop with no
  network still draws them. Playful for 10-11 year olds without baby shapes.
- **Header:** one clean row: the place's picture and name, the four-step path to the flag, one points pill (a star,
  the points, "Level N" only once earned, a thin bar toward the next level), and the race chip. Gone: "LV1", "x1"
  and the "🏆 🔒 after 🏁" text. The combo pill shows only from x2. The locked chip is a quiet trophy with a lock; a
  tap on it says in English and Hebrew when the race opens (it used to be a hover-only tooltip).
- **Hebrew under each instruction:** every instruction card has its Hebrew line right under the English, right-aligned
  and in Rubik, never inside a question or its answers.
- **Say game:** no Record, no Hear me, no microphone: the word plays, a sunny "Say it out loud!" pill (with its Hebrew)
  sits between the speaker and Next. Under a sentence, the Hebrew pill names the English word it translates
  ("dry = יבש"), so the picture is tied to its word.
- **Listen tile:** a big round speaker in the place's colour with a soft ring. Cause of the empty white square in his
  screenshot: the tile shared the class name "big" with the picture box, whose white background covered the tile's
  colour and hid the white icon. The tile now has its own class, and the smoke test checks the icon's size and colour
  on every listen round.
- **Pictures:** every picture box carries the word it shows, and the smoke test compares each picture with its own
  word on every round of River, Forest and Mountain. In his Mountain screenshot the sentence was "The river goes to the
  sea.", so the river picture was the right one for that sentence's word; it read as wrong because nothing on screen
  tied the picture to the word. The word pill under the sentence now does.
- **Touch first:** every button at least 48x48 px and at least 8 px from its neighbours (the smoke test fails any that
  is not), feedback on press rather than hover, no double-tap or pinch zoom, no text selection or long-press menu,
  letters can be tapped or dragged (pointer events) onto the slots, and each word's own speaker is a separate button
  beside the word (inside the word, a finger hit it by mistake).
- **Race:** the end card holds the score on the left and the place's top 5 on the right; a top-5 score asks for a
  first name in a big field, then lights up the new row, then Try again and Back, all inside the card.

## Polish review: every screen at 1366x768

Taken by `node test/design.mjs screens` (touch emulated). Each was looked at; what was wrong and the fix.

| Screen | What was wrong | Fix |
|---|---|---|
| [screens/01-start-new.png](screens/01-start-new.png) | Nothing: Full screen beside Start, the hint with picture, English and Hebrew. No Teacher button. | none |
| [screens/02-start-continue.png](screens/02-start-continue.png) | Nothing: Continue in the kid's place colour, same Full screen hint. | none |
| [screens/03-quiz-listen-picture.png](screens/03-quiz-listen-picture.png) | Before the pass the speaker could show as an empty white square. | listen tile class of its own; checked in smoke |
| [screens/04-quiz-picture-word.png](screens/04-quiz-picture-word.png) | Nothing. | none |
| [screens/05-quiz-listen-word.png](screens/05-quiz-listen-word.png) | Six words wrapped five and one. | wider choice row: six on one line |
| [screens/06-quiz-sentence-gap.png](screens/06-quiz-sentence-gap.png) | The gap's underline was invisible in the quiz (its colour came from a place, and the quiz has none). | underline falls back to River blue |
| [screens/07-reveal-mountain.png](screens/07-reveal-mountain.png) | Nothing: "Start at Mountain! Well done!" with confetti and the Hebrew line. | none |
| [screens/08-river-1-say.png](screens/08-river-1-say.png) | Record and Hear me (and a microphone request). | say it out loud, then Next |
| [screens/09-river-2-listen.png](screens/09-river-2-listen.png) | Nothing after the tile fix. | none |
| [screens/10-river-3-picword.png](screens/10-river-3-picword.png) | Each word's speaker sat inside the word button: a tap on the middle of a short word played the sound instead of answering. | speaker is its own 52 px button beside the word |
| [screens/11-river-4-build.png](screens/11-river-4-build.png) | No sign of where the next letter goes. | next slot tinted in the place colour; tiles drag or tap |
| [screens/12-forest-1-say.png](screens/12-forest-1-say.png) | As River's say game. | as River |
| [screens/13-forest-2-listen.png](screens/13-forest-2-listen.png) | Six pictures wrapped into a second row that ran into the word cards. | six smaller pictures on one row |
| [screens/14-forest-3-picword.png](screens/14-forest-3-picword.png) | Nothing. | none |
| [screens/15-forest-4-build.png](screens/15-forest-4-build.png) | As River's build. | as River |
| [screens/16-mountain-1-say.png](screens/16-mountain-1-say.png) | His screenshot: a river picture on Mountain with nothing tying it to the sentence's word; cramped header; Record. | word pill "dry = יבש" under the sentence; clean header; say it out loud |
| [screens/17-mountain-2-listen.png](screens/17-mountain-2-listen.png) | His screenshot: the listen tile an empty white square; six words wrapped. | speaker tile fixed; six words on one row |
| [screens/18-mountain-3-gap.png](screens/18-mountain-3-gap.png) | Nothing. | none |
| [screens/19-mountain-4-build.png](screens/19-mountain-4-build.png) | Nothing beyond the build fixes. | as River |
| [screens/20-offer-up-forest.png](screens/20-offer-up-forest.png) | Nothing: the offer card with the next place's picture, Hebrew line, two buttons. | none |
| [screens/21-offer-down-river.png](screens/21-offer-down-river.png) | Nothing. | none |
| [screens/22-flag-in-header.png](screens/22-flag-in-header.png) | The flag button was 40 px tall. | 48 px, spaced from the points pill |
| [screens/23-locked-chip-tapped.png](screens/23-locked-chip-tapped.png) | The locked chip explained itself only on mouse hover. | a tap shows when the race opens, English and Hebrew |
| [screens/24-last-check.png](screens/24-last-check.png) | Six pictures ran into the word cards (as Forest). | one row |
| [screens/25-end-river-race-card.png](screens/25-end-river-race-card.png) | The race card's top score was a fixed "Teacher 12". | the place's real top score and its holder, from the class board |
| [screens/26-race-countdown.png](screens/26-race-countdown.png) | Nothing. | none |
| [screens/27-race-running.png](screens/27-race-running.png) | Nothing. | none |
| [screens/28-race-name-entry.png](screens/28-race-name-entry.png) | New screen. | big name field, Save and Skip, board beside it |
| [screens/29-race-board-new-row.png](screens/29-race-board-new-row.png) | His screenshot: Back spilled out of the race end card. | card with padding and a wrapping row; smoke checks every button lies inside the card |
| [screens/30-race-end-not-top5.png](screens/30-race-end-not-top5.png) | The Hebrew line said both "time's up" and "new best". | only the line that is true |
| [screens/31-race-end-offline.png](screens/31-race-end-offline.png) | New screen; the bar read "Top score –". | "Be the first!"; the board says it is this laptop's own |
| [screens/32-end-mountain-last-place.png](screens/32-end-mountain-last-place.png) | Nothing: no next-place button on Mountain, by design. | none |
| [screens/33-projector-1-welcome.png](screens/33-projector-1-welcome.png) | Nothing. | none |
| [screens/34-projector-2-meet-word.png](screens/34-projector-2-meet-word.png) | Nothing (the word hides until Space, by design). | none |
| [screens/35-projector-2-meet-word-shown.png](screens/35-projector-2-meet-word-shown.png) | Nothing. | none |
| [screens/36-projector-2-meet-all-pictures.png](screens/36-projector-2-meet-all-pictures.png) | Nothing. | none |
| [screens/37-projector-3-quiz-time.png](screens/37-projector-3-quiz-time.png) | The three place pictures were squeezed to dots (the pill's padding fell on them). | pictures take no pill padding |
| [screens/38-projector-4-play-and-race.png](screens/38-projector-4-play-and-race.png) | Nothing. | none |
| [screens/39-projector-5-lets-go-two.png](screens/39-projector-5-lets-go-two.png) | Nothing. | none |
| [screens/40-projector-5-lets-go-four.png](screens/40-projector-5-lets-go-four.png) | Nothing. | none |
| [screens/41-projector-6-whats-missing.png](screens/41-projector-6-whats-missing.png) | Nothing. | none |
| [screens/42-projector-6-one-hidden.png](screens/42-projector-6-one-hidden.png) | Nothing. | none |
| [screens/43-projector-6-summary.png](screens/43-projector-6-summary.png) | Nothing. | none |
| [screens/44-plan-top.png](screens/44-plan-top.png) | The plan looked like a different site (system font, plain header). | same fonts, colours and cards as the site, a green-blue title band |
| [screens/45-plan-stage-4.png](screens/45-plan-stage-4.png) | Its table still said "record 3 s; Hear me". | "say it out loud, then Next"; race boards described |

Every projector stage keeps its progress bar touch-friendly too: each stage segment is a 48 px tall target with a
10 px bar drawn inside it.
