# Kid UX for 5th grade: tone, secret picture, icons, taken name, guests (research, 2026-11-11)

His words (page 42423, points 1-2, quoted whole):

> 1)the secret animals don't look really good. perhaps find an open-source list or something that we can use, because currently it's quite terrible. the design itself is not appealing towards 5th-graders, the emojis are horrible, use icons and glyphs rather than emojis. the whole design (ux&ui) is lacking, unprofessional. the thread should research best-practices and design principles first. not too childish, not too mature. choosing your password is the fun part, it's supposed to be an experience, fun. yet secure. also we need to disable attempting to login to an already-logged in account, so after a name is taken in the login screen, it disappears from the available names list immediately, pops-out, perhaps with a cute animation of fading or something like that, like a comic
>
> 2)I'm not really sure I like that part: "The Classroom post carries the class link (…/?c=482913), so at home the kid skips the code and lands on the names."
> are we certain we only want to use classroom? I'm not sure I want that. to simplify the design, for now let's drop signing-in after class, for now it'll be class-only, kind of like an MVP, only with 5th-grade, one class. not for multiple students.

Also on that page: "I don't like the "Kitah" logo. always consult with me before choosing a design language, icons or logos so we can iterate and brainstorm and choose a language together."

Builds on research/student-signin.md (16-picture grid, 3 wrong tries then a 60 s pause, a hard cap of 6 wrong tries per lesson, teacher reset, shuffle every time). Not redone here. Each source says whether I opened it or only saw it in search results. Scope: one 5th-grade class (10-11 years), class only, shared Asus laptops at 1366x768.

## Tone

What the evidence says:

- Age bands: NN/g tested 125 children aged 3-12 over three studies (2001, 2010, 2018; Study 1 had 55 children aged 6-11, about a third in Israel) and says design for at least three bands, with 9-12 as its own band; children judge age fit harshly (a 6-year-old called a site "for babies" because of cartoons and trains), and content one grade above or below their level drew negative reactions. Older children scan rather than read; body text 12 pt for older children vs 14 pt for young ones ([NN/g, Sherwin and Nielsen](https://www.nngroup.com/articles/childrens-websites-usability-issues/), 2019-01-13, opened).
- 11-year-olds used the mouse easily; dragging and small targets are hard for younger kids ([NN/g physical development](https://www.nngroup.com/articles/children-ux-physical-development/), date not shown, seen in search). So our kids handle normal controls; we do not need oversized toy buttons beyond the 120 px picture tiles already set.
- Tweens "are scanning through content fast", "like bite-sized" content and are "picky users"; cute is fine if "balanced with weird and quirky" ([The Next Web, Anderskou of Hello Monday](https://thenextweb.com/news/a-hands-on-approach-to-designing-for-kids), 2018-09-12, opened).
- A design guide lists "primary colors, cartoonish UI, patronizing tone" as babyish for tweens, who are "visually literate and highly sensitive to tone" ([Gapsy Studio](https://gapsystudio.com/blog/ux-design-for-kids), date not shown, seen in search only; a design agency, not a study).
- Example from a product they use: Duolingo pairs a heavy rounded headline face (Feather Bold, built from the owl's shapes) with a rounded sans for body text ([Creative Bloq](https://www.creativebloq.com/news/feather-bold), date not shown, seen in search; [Canny Creative](https://www.canny-creative.com/brand-breakdown/brand/duolingo/), seen in search). Kahoot gives kids a two-word "kind tiger" style nickname from 800 combinations because typed names were inappropriate and slow ([Kahoot blog](https://kahoot.com/blog/2017/11/09/generate-funny-nicknames-players-live-kahoots/), 2017-11-09, opened): an animal plus one describing word is a shape these kids already enjoy.

What that means for us (each is a rule the mockup follows):

| Part | Too childish | Too grown-up | Our target |
|---|---|---|---|
| Type | Comic Sans, bubble letters everywhere | Thin light sans, small grey text | One heavy rounded or bold display face for headings, a plain sans for text, body at least 16 CSS px (about 12 pt, the NN/g figure for older kids) |
| Colour | Pure primary red/blue/yellow on white | Grey, beige, one muted accent | One dark ink colour plus 2-3 saturated but not primary accents |
| Shapes | Wobbly cartoon frames, bevels, skeuomorphic toys | Flat hairline boxes | Rounded cards, thick outlines or offset shadows (sticker / game feel) |
| Wording | "Yay! Great job, sweetie!" | Instructions in long sentences | Short imperative: "Tap your name", "Pick your buddy" (his rule: simple English, Hebrew beside it) |
| Characters | Babyish mascots talking to the kid | None at all | Animal glyphs as the kid's own buddy, never a narrator |
| Emoji | Out: renderings differ by platform; one emoji read as "blissfully happy" on Google was "ready to fight" on Apple, and people disagreed on sentiment about 25% of the time even on the same image ([Miller et al., ICWSM 2016](https://ojs.aaai.org/index.php/ICWSM/article/view/14757), seen in search) | | Open-source SVG glyphs only (see Icon sets) |

Fonts, all with Hebrew and Latin and the SIL Open Font License (checked in each family's METADATA.pb in [google/fonts](https://github.com/google/fonts/tree/main/ofl), opened 2026-11-11): Fredoka, Rubik, Varela Round, Secular One, Karantina, Assistant.

## Secret picture

His pick stands: 16 animals in a 4x4 grid (page 41389), each kid picks their own (decision 1237). The question is how to make choosing it fun and still hard to guess.

Blind-guess odds against one named kid (chance = tries / combinations; cap of 6 wrong tries from research/student-signin.md):

| Scheme | Combinations | 1 guess | 3 tries (first pause) | 6 tries (lock) | Taps to sign in |
|---|---|---|---|---|---|
| 1 animal of 16 (today) | 16 | 6.25% | 18.8% | 37.5% | 1 |
| Animal of 16, then its colour of 6 | 16 x 6 = 96 | 1.04% | 3.1% | 6.25% | 2 |
| 2 different animals, any order | C(16,2) = 120 | 0.83% | 2.5% | 5.0% | 2 |
| 2 animals in order (repeat allowed) | 16 x 16 = 256 | 0.39% | 1.2% | 2.3% | 2 |
| 3 animals in order | 4096 | 0.02% | 0.07% | 0.15% | 3 |
| PassTiles: 5 tiles of 48 (Assal) | C(48,5) = 1,712,304 (about 21 bits) | near 0 | near 0 | near 0 | 5 |

Recall and time:
- Children recalled passwords made of distinct objects best; 25 children aged 7-12 (mean 9.5) took 20.5 to 60.6 s per PassTiles login of 5 picks (about 4-12 s a pick, computed) and memorised in 29.5-32.6 s; the median child got 4-5 of 5 tiles right first time, and overall success was "much lower than needed for real deployment" ([Assal, Imran, Chiasson](https://ar5iv.labs.arxiv.org/html/1610.09743), 2016, opened). So every extra pick costs about 4-12 s and some recall; 5 picks is too many.
- Pictures beat text for 40 children aged 6-11 (KidsPic, a picture sequence; from research/student-signin.md).
- Ordered pairs add an error that does not exist for one picture: the right two animals in the wrong order fail. No child study gives that error rate; I found none.
- Retrieval right after learning strengthens memory more than looking again ([Roediger and Karpicke, Psychological Science](https://pubmed.ncbi.nlm.nih.gov/16507066/), 2006, abstract read through the PubMed API): so the ceremony ends with the kid doing it once more.
- Colour: red-green colour blindness affects about 8% of boys and 0.5% of girls ([Wikipedia, Color blindness](https://en.wikipedia.org/wiki/Color_blindness), opened), so about 1 boy in a class of 30 (15 boys x 8% = 1.2, computed). Every colour swatch carries its word (English and Hebrew), so the kid can remember "purple", not a shade.
- Why 6 colours: red, orange, yellow, green, blue, purple are the chromatic basic colour words in both English and Hebrew (English has 11 basic colour terms, [Berlin and Kay 1969](https://en.wikipedia.org/wiki/Basic_Color_Terms:_Their_Universality_and_Evolution), opened); pink and brown are left out because on a laptop screen they sit close to red, purple and orange. That cut is my judgment, to be tried with the class.

Recommendation: animal, then its colour: "make your secret buddy". 96 combinations cut a 6-try guesser from 37.5% to 6.25%, it costs one extra tap (about 4-12 s by Assal's per-pick time, to be measured in class), and the kid remembers one thing with a name ("purple fox", the same animal-plus-word shape as Kahoot's nicknames) instead of two pictures in an order. The ceremony, each step one screen:
1. "Pick your buddy": 16 animals pop in, shuffled.
2. "Paint it": tap a colour; the buddy bounces into that colour (squash and stretch, the first of Disney's [12 principles of animation](https://en.wikipedia.org/wiki/Twelve_basic_principles_of_animation), opened).
3. Reveal: "Your secret buddy: Purple Fox. Shh, keep it secret."
4. "Do it once more" from a reshuffled grid (testing effect above); only then saved.
Security notes kept from research/student-signin.md: reshuffle every time, no hover highlight on sign-in, same "checking" screen for right and wrong, the buddy is never shown after sign-in.

## Icon sets

Checked by downloading each package from npm on 2026-11-11 and listing its files (opened):

| Set | License | Style | Glyphs | Real animals in the set | Nature | Controls (fullscreen, sound, trophy, lock, check, play) |
|---|---|---|---|---|---|---|
| [Phosphor](https://github.com/phosphor-icons/core) 2.1.1 | MIT | Rounded, 6 weights incl. duotone and fill | 1,512 per weight | 10: cat, dog, rabbit, cow, bird, fish, butterfly, horse, shrimp, beetle | tree, leaf, cactus, tulip, acorn, sun | all |
| [Tabler](https://github.com/tabler/tabler-icons) 3.51.0 | MIT | 2 px outline, 24 px grid | 5,233 outline + 1,054 filled | 12: cat, dog, pig, horse, deer, fish, butterfly, snail, spider, bat, bug, cricket (+ dragon) | leaf, flower, cactus, seedling, mountain | all |
| [Lucide](https://github.com/lucide-icons/lucide) 1.72.0 | ISC | 2 px outline (Feather fork) | 2,356 | 13: cat, dog, rabbit, panda, bird, fish, rat, shrimp, snail, squirrel, turtle, worm, bug | yes | all |
| [game-icons.net](https://game-icons.net/about.html) (via Iconify 1.2.4) | CC BY 3.0: credit each author | Bold filled silhouettes, game look | 4,134 | 30+: incl. cat, fox, panda, frog, pig, owl, lion, koala, penguin, monkey, cow, rabbit, turtle, elephant, octopus, hedgehog | yes | trophy, expand, speaker, padlock, check, play |

Also checked and dropped: Mingcute (Apache-2.0, 12 animals), Fluent (MIT, 5 animals), Material Symbols (Apache-2.0, 3-5 animals), Hugeicons free (MIT, about 11 animals).

Plain truth: only game-icons has 16 distinct animals. With Phosphor, Tabler or Lucide the 16 tiles would be about 10-13 animals plus nature glyphs, or the grid drops to 12 (the fallback in research/student-signin.md). The mockup shows each set as it really is, so he can weigh this. No emoji anywhere.

## Taken name

He wants a taken name to leave the list at once, popping out like a comic.

- Durations: NN/g puts UI animation at 100-400 ms, says 500 ms "starts to feel like a real drag", and gives about 200-250 ms for something disappearing ([NN/g, Laubheimer](https://www.nngroup.com/articles/animation-duration/), 2020-02-09, opened). Material 3's tokens: short4 = 200 ms, medium1 = 250 ms, medium2 = 300 ms; exits use emphasized-accelerate, cubic-bezier(0.3, 0, 0.8, 0.15) (read from the [@material/web](https://www.npmjs.com/package/@material/web) 2.5.0 package, motion tokens file, opened).
- Pick: the pop is 250 ms (NN/g's disappear figure, M3 medium1): grow to 1.15 and squash (anticipation, then squash and stretch, from the [12 principles](https://en.wikipedia.org/wiki/Twelve_basic_principles_of_animation)), then shrink to nothing with a comic burst, using emphasized-accelerate. The names below close the gap in 200 ms (M3 short4). The name stops being tappable on the first frame, before the pop starts, so "at once" holds even while it plays.
- Reduced motion: WCAG 2.3.3 asks that motion triggered by interaction can be turned off; technique C39 uses the prefers-reduced-motion media query ([W3C Understanding 2.3.3](https://www.w3.org/WAI/WCAG21/Understanding/animation-from-interactions), opened via search summary; [W3C C39](https://www.w3.org/WAI/WCAG22/Techniques/css/C39.html), seen in search). With it on, the name is removed in the same tap, no pop.

## Guests

His question: should kids who are not signed in reach the site? Home's pick sent to him: yes.

- Evidence: in classroom leaderboard studies, top-ranked students gained while lower-ranked ones lost motivation (45 students, [JOTSE 2025](https://jotse.org/index.php/jotse/article/view/3321), seen in search); a preprint recommends limiting the public display of bottom rankings ([HKU](https://hub.hku.hk/bitstream/10722/368255/1/content.pdf), seen in search). His own words make the race optional and competitive (decision 1164).
- Design: a guest plays every stage of the lesson. Only the race leaderboard (his top 10) needs a name. At race end a guest sees the same celebration and their own score, plus one neutral line: "Guest scores are not on the board. Sign in next lesson to join it." No lock icon, no red, no "you can't".
- Shown without shame: the board lists only signed-in names, top 10 only, so nobody's low rank or guest status is shown to the class.

## Sources

Opened:
- https://www.nngroup.com/articles/childrens-websites-usability-issues/ (2019-01-13)
- https://thenextweb.com/news/a-hands-on-approach-to-designing-for-kids (2018-09-12)
- https://kahoot.com/blog/2017/11/09/generate-funny-nicknames-players-live-kahoots/ (2017-11-09)
- https://ar5iv.labs.arxiv.org/html/1610.09743 (2016)
- https://pubmed.ncbi.nlm.nih.gov/16507066/ (2006, abstract via the PubMed API)
- https://en.wikipedia.org/wiki/Color_blindness (read 2026-11-11)
- https://en.wikipedia.org/wiki/Basic_Color_Terms:_Their_Universality_and_Evolution (read 2026-11-11)
- https://en.wikipedia.org/wiki/Twelve_basic_principles_of_animation (read 2026-11-11)
- https://www.nngroup.com/articles/animation-duration/ (2020-02-09)
- https://game-icons.net/about.html (read 2026-11-11)
- npm packages @phosphor-icons/core 2.1.1, @tabler/icons 3.51.0, lucide-static 1.72.0, @iconify-json/game-icons 1.2.4, @material/web 2.5.0 (downloaded 2026-11-11)
- https://github.com/google/fonts/tree/main/ofl (font METADATA.pb files, read 2026-11-11)

Seen in search only:
- https://www.nngroup.com/articles/children-ux-physical-development/
- https://gapsystudio.com/blog/ux-design-for-kids
- https://www.creativebloq.com/news/feather-bold
- https://www.canny-creative.com/brand-breakdown/brand/duolingo/
- https://ojs.aaai.org/index.php/ICWSM/article/view/14757 (2016)
- https://www.w3.org/WAI/WCAG21/Understanding/animation-from-interactions
- https://www.w3.org/WAI/WCAG22/Techniques/css/C39.html
- https://jotse.org/index.php/jotse/article/view/3321 (2025)
- https://hub.hku.hk/bitstream/10722/368255/1/content.pdf

Where research is silent: no study of animal-plus-colour pictures for children; no error rate for ordered picture pairs in 10-11 year olds; sign-in seconds per pick are extrapolated from Assal's 5-pick logins and need measuring in class.
