# Nature Trails

A 40-45 minute 5th-grade English lesson on eight nature words (air, dry, forest, grass, mountain, nature, river, sea),
every kid on his own laptop through Google Classroom, with three trails of differentiated work.

- **Kids' link (the one Classroom post):** https://vaknin.github.io/nature-words/
- **Projector:** https://vaknin.github.io/nature-words/#teacher
- **Teacher's plan:** https://vaknin.github.io/nature-words/plan.html
- **What to post in Classroom:** [classroom.md](classroom.md)
- **Classroom guide for the class teacher (Word + PDF, Hebrew):** [Nature-Words-Classroom-Guide.pdf](teacher-doc/Nature-Words-Classroom-Guide.pdf) · [Nature-Words-Classroom-Guide.docx](teacher-doc/Nature-Words-Classroom-Guide.docx) (built from classroom.md: `uv run teacher-doc/build.py`, checked by `uv run teacher-doc/check.py`). It holds the one class link only: no projector or per-place link.
- **Lesson-plan form:** [lp/LP.docx](lp/LP.docx) · [lp/LP.pdf](lp/LP.pdf) (`uv run lp/fill_lp.py`)
- **Race boards:** one shared top 5 per place (River, Forest, Mountain) on ntfy.sh; **Reset boards** is on the projector page.
- Trails by hand: [#level-1](https://vaknin.github.io/nature-words/#level-1) River ·
  [#level-2](https://vaknin.github.io/nature-words/#level-2) Forest ·
  [#level-3](https://vaknin.github.io/nature-words/#level-3) Mountain
- Research behind it: [research/](research/) · design mockup: https://vaknin.github.io/nature-words/mockup/
- **Design notes:** [design/](design/) (the lesson design, the design review and its screenshots; the shared screen rules are in [DESIGN.md](DESIGN.md))

Test: `cd test && npm install && timeout 400 node smoke.mjs [URL]` (touch emulated, both laptop sizes and the two-laptop race board at once; never the class board). Design screens: `node design.mjs all [URL]`.
