# /// script
# dependencies = ["python-docx"]
# ///
"""Fill Aviv's MoE lesson-plan form (LP.original.docx) from the final nature lesson.

Run: uv run lp/fill_lp.py   (reads ~/lesson-lp/LP.original.docx, never writes it)
Writes lp/LP.docx and ~/lesson-lp/LP.docx (identical bytes).
Source of the lesson: Fable's final plan (follow-1.answer.md), stages 4/8/5/18/4/4.
Layout, styles and fonts come from the template: text goes into existing
paragraphs/runs or into copies of them; choices are marked with real Word
highlighting, never by deleting the other options.
"""
import copy
import shutil
from pathlib import Path

import docx
from docx.oxml.ns import qn
from docx.oxml import OxmlElement

HOME = Path.home()
SRC = HOME / "lesson-lp" / "LP.original.docx"
HERE = Path(__file__).resolve().parent
OUT = HERE / "LP.docx"
OUT_HOME = HOME / "lesson-lp" / "LP.docx"
SITE = "https://vaknin.github.io/nature-words/"
HL = "yellow"  # the template's own highlight colour ("Highlight" labels)


# ---------- low-level helpers ----------
def plain_rpr(run):
    """Copy of a run's rPr without bold/underline/highlight, for filled-in text."""
    rpr = run._r.rPr
    if rpr is None:
        return None
    rpr = copy.deepcopy(rpr)
    for tag in ("w:b", "w:bCs", "w:u", "w:highlight", "w:i"):
        for el in rpr.findall(qn(tag)):
            rpr.remove(el)
    return rpr


def add_run(par, text, like=None, bold=False):
    r = OxmlElement("w:r")
    src = like if like is not None else (par.runs[0] if par.runs else None)
    rpr = plain_rpr(src) if src is not None else None
    if rpr is None:
        # empty paragraph: take the paragraph mark's run properties (font/size)
        ppr_rpr = par._p.pPr.find(qn("w:rPr")) if par._p.pPr is not None else None
        rpr = copy.deepcopy(ppr_rpr) if ppr_rpr is not None else OxmlElement("w:rPr")
        for tag in ("w:b", "w:bCs", "w:u", "w:highlight"):
            for el in rpr.findall(qn(tag)):
                rpr.remove(el)
    if bold:
        rpr.insert(0, OxmlElement("w:b"))
    r.append(rpr)
    t = OxmlElement("w:t")
    t.set(qn("xml:space"), "preserve")
    t.text = text
    r.append(t)
    par._p.append(r)
    return r


def clone_par_after(par, like_run=None):
    """New empty paragraph after `par` with the same paragraph properties."""
    new = copy.deepcopy(par._p)
    for child in list(new):
        if child.tag != qn("w:pPr"):
            new.remove(child)
    par._p.addnext(new)
    return docx.text.paragraph.Paragraph(new, par._parent)


def write_lines(par, lines, like=None):
    """Write lines[0] into `par` (appending), the rest into cloned paragraphs below."""
    like = like if like is not None else (par.runs[-1] if par.runs else None)
    add_run(par, lines[0].strip("*"), like, bold=lines[0].startswith("**"))
    cur = par
    for line in lines[1:]:
        cur = clone_par_after(cur)
        bold = line.startswith("**")
        add_run(cur, line.strip("*"), like, bold=bold)
    return cur


def set_par_text(par, text, like=None):
    like = like if like is not None else (par.runs[0] if par.runs else None)
    keep = copy.deepcopy(like._r) if like is not None else None
    for r in list(par.runs):
        r._r.getparent().remove(r._r)
    if keep is not None:
        for t in keep.findall(qn("w:t")):
            keep.remove(t)
        t = OxmlElement("w:t")
        t.set(qn("xml:space"), "preserve")
        t.text = text
        keep.append(t)
        par._p.append(keep)
    else:
        add_run(par, text)


def highlight_run(r):
    rpr = r.find(qn("w:rPr"))
    if rpr is None:
        rpr = OxmlElement("w:rPr")
        r.insert(0, rpr)
    for el in rpr.findall(qn("w:highlight")):
        rpr.remove(el)
    h = OxmlElement("w:highlight")
    h.set(qn("w:val"), HL)
    # schema order: highlight comes after sz/szCs, before u/vertAlign/lang
    after = [qn(x) for x in ("w:u", "w:effect", "w:bdr", "w:shd", "w:fitText",
                             "w:vertAlign", "w:rtl", "w:cs", "w:em", "w:lang",
                             "w:eastAsianLayout", "w:specVanish", "w:oMath")]
    for i, ch in enumerate(rpr):
        if ch.tag in after:
            rpr.insert(i, h)
            return
    rpr.append(h)


def highlight_sub(par, sub):
    """Highlight exactly `sub` inside par, splitting runs where needed."""
    text = "".join(r.text for r in par.runs)
    start = text.find(sub)
    assert start >= 0, (sub, text)
    end = start + len(sub)
    pos = 0
    for run in list(par.runs):
        r = run._r
        s, e = pos, pos + len(run.text)
        pos = e
        if e <= start or s >= end:
            continue
        a, b = max(start, s) - s, min(end, e) - s
        full = run.text
        pieces = [(full[:a], False), (full[a:b], True), (full[b:], False)]
        prev = r
        made = []
        for txt, hl in pieces:
            if not txt:
                continue
            n = copy.deepcopy(r)
            for t in n.findall(qn("w:t")):
                n.remove(t)
            t = OxmlElement("w:t")
            t.set(qn("xml:space"), "preserve")
            t.text = txt
            n.append(t)
            if hl:
                highlight_run(n)
            prev.addnext(n)
            prev = n
            made.append(n)
        r.getparent().remove(r)


def find_par(cell, startswith):
    for p in cell.paragraphs:
        if p.text.strip().startswith(startswith):
            return p
    raise KeyError(startswith)


def highlight_options(cell, options):
    for opt in options:
        highlight_sub(find_par(cell, opt), opt)


def uniq(row):
    out, seen = [], set()
    for c in row.cells:
        if id(c._tc) not in seen:
            seen.add(id(c._tc))
            out.append(c)
    return out


def fill_time(cell, text):
    # the Time column is one character wide in the template, so minutes are written as 8'
    p = cell.paragraphs[0]
    if p.runs:
        set_par_text(p, text)
    else:
        add_run(p, text)


def fill_scan(cell, lines):
    """'S. can…' placeholder -> 'S. can …' with the stage's lines."""
    p = find_par(cell, "S. can")
    like = p.runs[0]
    set_par_text(p, "S. can:", like)
    write_lines(clone_par_after(p), lines, like)


def fill_teacher(cell, lines, size_like=None):
    p = cell.paragraphs[0]
    if p.text.strip() == "":
        for r in list(p.runs):
            r._r.getparent().remove(r._r)
        like = size_like
        write_lines(p, lines, like)
    else:
        write_lines(clone_par_after(cell.paragraphs[-1]), lines, size_like or cell.paragraphs[-1].runs[-1])


def add_rationale(cell, text):
    last = cell.paragraphs[-1]
    if not last.text.strip():
        last = [p for p in cell.paragraphs if p.text.strip()][-1]
    like = last.runs[0]
    write_lines(clone_par_after(last), [text], like)


# ---------- the lesson ----------
CANDO = [l.strip() for l in (HERE / "cando.txt").read_text().splitlines() if l.strip()]


def main():
    shutil.copyfile(SRC, OUT)
    d = docx.Document(OUT)
    t0, t1, t2 = d.tables

    # Header
    r = t0.rows
    def after_label(cell, label, value):
        p = find_par(cell, label)
        add_run(p, value, like=p.runs[-1])
    after_label(r[2].cells[0], "Lesson plan for the date of", "Wednesday, 11 November 2026")
    after_label(r[2].cells[1], "Grade", " 5")
    after_label(r[3].cells[0], "Topic/ Content", "Nature words")
    after_label(r[3].cells[1], "Target words", "air, dry, forest, grass, mountain, nature, river, sea")
    p = find_par(r[4].cells[1], "Additional Materials")
    write_lines(p, ["projector; whiteboard;"], p.runs[-1])
    write_lines(clone_par_after(p), [
        "one laptop per student, with headphones;",
        f"the lesson site {SITE}, shared in one Google Classroom post.",
    ], p.runs[-1])

    # Communicative modes (row 6: two cells)
    m1, m2 = uniq(r[6])
    highlight_sub(find_par(m1, "Spoken RECEPTION"), "Spoken RECEPTION/ Written RECEPTION")
    highlight_sub(find_par(m1, "Spoken PRODUCTION"), "Spoken PRODUCTION/ Written PRODUCTION")
    highlight_sub(find_par(m2, "Spoken INTERACTION"), "Spoken INTERACTION")
    highlight_sub(find_par(m2, "Spoken MEDIATION"), "Spoken MEDIATION")

    # Can-do statements (quoted word for word from the curriculum's Pre-A1/A1 can-do lists)
    c = uniq(r[7])[0]
    p1, p2 = find_par(c, "1."), find_par(c, "2.")
    level = {0: "Pre-A1", 1: "Pre-A1", 2: "Pre-A1", 3: "Pre-A1", 4: "Pre-A1", 5: "A1"}
    for i, line in enumerate(CANDO):
        if i == 0:
            p = p1
        elif i == 1:
            p = p2
        else:
            p = clone_par_after(p)
            add_run(p, f"{i + 1}.", like=p1.runs[0], bold=True)
        add_run(p, f" “{line}” ({level.get(i, 'Pre-A1')})", like=p1.runs[0])

    # Learning outcomes
    c = uniq(r[8])[0]
    p = find_par(c, "Students can … aiming")
    set_par_text(p, "")
    add_run(p, "Students can say, read and spell the eight target words, aiming to reach the "
               "spoken/ written level at the word/ chunk/ sentence stage.", like=None)
    blank = [q for q in c.paragraphs if not q.text.strip()][0]
    write_lines(blank, [
        "1. Students can say each of the eight target words aloud when shown its picture or gesture.",
        "2. Students can match each of the eight words, heard or read, to its picture (at least 3 of 4 right in the laptop’s Last check).",
        "3. Students can build each of the eight words from its letters on the laptop.",
        "4. Students can say one full sentence, “Let’s go to the sea / mountain / river / forest!”, about a place they choose.",
    ], like=find_par(c, "Students can+").runs[-1])

    # Prerequisites
    c = uniq(r[9])[0]
    p = find_par(c, "Vocabulary Items")
    add_run(p, " a few nature words the students already know in English (elicited in the opening, e.g. tree, water, sky).")
    p = find_par(c, "Grammar patterns")
    add_run(p, " none new; “Let’s go to the ___!” is taught as a fixed chunk.")
    p = find_par(c, "Other")
    add_run(p, "opening a laptop, Google Classroom and a link; wearing headphones; knowing the English letters to build words.")

    # ---- Table 1: Routine + Opening ----
    rows = t1.rows
    _, time, teach, scan, comm, inter, rat = uniq(rows[2])
    fill_time(time, "1\'")
    write_lines(clone_par_after(find_par(teach, "The teacher writes")),
                ["“Today is Wednesday, the eleventh of November.”"], find_par(teach, "The teacher writes").runs[0])
    highlight_options(comm, ["Spoken/ written Reception", "Spoken/ written Production", "Spoken/ written Interaction"])
    highlight_options(inter, ["whole class"])

    _, time, teach, scan, comm, inter, rat = uniq(rows[3])
    fill_time(time, "3\'")
    highlight_sub(find_par(teach, "2.Spoonfeeding"), "2.Spoonfeeding")
    size = find_par(teach, "1.Creating").runs[0]
    write_lines(clone_par_after(find_par(teach, "3.Discovery")), [
        "**Welcome to nature (laptops closed). Projector: title, goal, agenda.",
        "“Good morning! Laptops closed. Eyes on me.”",
        "“Look. Trees, water, a big sky. This is nature.”",
        "“Who likes a trip to the sea? Hands up! To the mountains?”",
        "“You know nature words in English. Tell me!” T writes six on the board; the class repeats each.",
        "“Today: eight NEW nature words. Read with me.” The class reads the goal: “I can say, read and spell eight nature words.”",
        "“First we meet the words. Then a quiz and a game on the laptop. Then we talk.”",
        "“Who can say in Hebrew what we do today?”",
        "Transition: “Stand up!” Praise: “You know so many nature words already!”",
    ], size)
    fill_scan(scan, [
        "raise hands and call out nature words they already know;",
        "repeat each word after the teacher;",
        "read the lesson goal aloud together;",
        "say in Hebrew what the lesson will be about.",
    ])
    highlight_options(inter, ["whole class"])
    add_rationale(rat, "The trip to nature (sea, mountains) is the context for the whole lesson; "
                       "a student restating the agenda in Hebrew checks that everyone understood.")

    # Communicative mode cell of the Opening row is empty in the template: leave it.

    # ---- Body 1 (oral, word stage) ----
    # Stage 2: Meet the words -> Repetition
    stp, time, teach, scan, inter, rat = uniq(rows[7])
    fill_time(time, "8\'")
    fill_teacher(teach, [
        "**Stage 2: Meet the words (students standing).",
        "Projector: word cards in this order: nature, mountain, river, forest, sea, grass, air, dry; then all eight pictures without words.",
        "Per word (about 35 seconds): “Look. Listen.” T presses Space: the recording plays and the word appears. "
        "T says it twice with its gesture. “Everybody!” three times: normal, whisper, loud. “What is it in Hebrew?”",
        "Gestures: nature, arms wide; mountain, hands meet in a peak; river, hand swims like a snake; forest, arms up like trees; "
        "sea, big waves; grass, wiggle fingers low; air, blow on your hand; dry, wipe your forehead.",
        "Show me! “I say the word. You show me with your hands. No talking!”",
        "Say it! “I show, you say.” Trick: T points at the mountain: “Sea?” Class: “No! Mountain!”",
        "Transition: “Sit down. Don’t open the laptop yet. Look at the screen.”",
        "Praise: “Beautiful gestures, and I heard everyone’s voice!”",
    ], find_par(rat, "Students need").runs[0])
    fill_scan(scan, [
        "repeat each new word at least four times, with its gesture;",
        "say the Hebrew meaning of each word;",
        "show the right gesture when they hear a word, and say the word when they see its picture.",
    ])
    highlight_options(inter, ["whole class"])
    add_rationale(rat, "Choral repetition in three voices with a gesture links sound, meaning and movement before any reading.")

    # Stage 3: Quiz time -> Recognition
    stp, time, teach, scan, inter, rat = uniq(rows[8])
    fill_time(time, "5\'")
    fill_teacher(teach, [
        "**Stage 3: Quiz time (demo, then each student alone).",
        "Projector: the three places, then a demo of one quiz question and one “Listen. Say the word.” on the students’ page.",
        "“Look. Three places: River, Forest and Mountain. All three have the same eight words. First, a quick quiz. "
        "Then the laptop gives you a place to start. It is a good start for YOU today. Your place can change later, and that is OK. "
        "Points and cards are the same in every place. Eyes on your own screen.”",
        "Demo: “I listen. I press. No right or wrong in the quiz, just try. When you see the microphone, press Allow.”",
        "On four fingers: “One: open your laptop. Two: Classroom, ‘Nature Words’. Three: press the link. Four: headphones on, press Start.”",
        "Check: “Who can say the four steps in Hebrew?” Then “Go!”",
        "Quiz: eight questions, each word once: hear a word and press its picture; see a picture and press its word; complete a short sentence. "
        "The laptop places each student in River, Forest or Mountain.",
        "Praise: “Quick and quiet. Thank you!”",
    ], find_par(rat, "Students need").runs[0])
    fill_scan(scan, [
        "follow four spoken instructions and repeat them in Hebrew;",
        "recognise each word they hear or read by pressing its picture or word;",
        "complete a short sentence by choosing the right word.",
    ])
    highlight_options(inter, ["whole class", "individual work"])
    add_rationale(rat, "The quiz checks recognition of every word once and starts each student at a level where they can succeed.")

    # ---- Body 2 (written, word stage) ----
    r2 = t2.rows
    # Stage 4: Play and race -> Reproduction (games also cover repetition and recognition)
    stp, time, teach, scan, inter, rat = uniq(r2[4])
    fill_time(time, "18\'")
    fill_teacher(teach, [
        "**Stage 4: Play and race (laptops, headphones; T walks around).",
        "Projector: the eight pictures with words and an 18-minute bar.",
        "Each student plays four games in their place (River, Forest or Mountain), eight rounds each: "
        "1. Listen. Say the word (record and play back). 2. Listen. Press the picture/word. "
        "3. Look/Read. Press the word. 4. Build the word from letters. Then the Last check (a flag): four questions.",
        "At 11 minutes: “Look, a flag! Finish your word. Press the flag: four last questions.”",
        "At 13 minutes: “After the flag you can race: 30 seconds. Beat your best! Or go to the next place. You choose.”",
        "While they work: River screens first, “Play me your word.” One specific praise per student, for effort. "
        "At an offer to change place: “Good choice” either way.",
        "Transition at 18: “Stop. Headphones off. Screens half down. Eyes on me.”",
        "Praise: “You tried again and you got it. That is how we learn.”",
    ], find_par(rat, "Students need").runs[0])
    fill_scan(scan, [
        "say each word into the microphone and listen to themselves;",
        "find the picture or word they hear, then the word that matches a picture or sentence;",
        "build each word from its letters;",
        "answer the four Last-check questions, then race against their own best score.",
    ])
    highlight_options(inter, ["individual work"])
    add_rationale(rat, "Three levels with the same eight words let every student practise at the right challenge, "
                       "moving from recognition to building the word on their own.")

    # ---- Body 3 (oral, sentence stage) ----
    # Stage 5: Let's go! -> Reproduction (fixed sentence frame)
    stp, time, teach, scan, inter, rat = uniq(r2[10])
    fill_time(time, "4\'")
    fill_teacher(teach, [
        "**Stage 5: Let’s go! (the whole-class speaking moment).",
        "Projector: two pictures and “Let’s go to the ___!” Three slides: sea or mountain; river or forest; all four.",
        "Per slide: “The sea or the mountain? Show me with your hands!” “Say it, everybody: Let’s go to the…!” "
        "“Sea people, stand up! Tell them!” To one or two volunteers: “Why the sea?” T says the answer back in full; the class repeats.",
        "Last slide: “Four places, one class trip. On three, say your sentence, loud!”",
        "Praise: “Full sentences from everyone. That is real English!”",
    ], find_par(rat, "Students need").runs[0])
    fill_scan(scan, [
        "choose a place and say “Let’s go to the sea / mountain / river / forest!”;",
        "repeat a classmate’s answer in a full sentence.",
    ])
    highlight_options(inter, ["whole class"])
    add_rationale(rat, "A fixed, useful chunk lets beginners say a whole sentence about their own choice.")

    # Stage 6: What's missing? -> Closure
    stp, time, teach, scan, inter, rat = uniq(r2[17])
    fill_time(time, "4\'")
    fill_teacher(teach, [
        "**Stage 6: What’s missing?",
        "“Who beat their own best in the race? Stand up!” Applause.",
        "“Close your eyes! … Open! What’s missing?” Four rounds with the word pictures.",
        "“How many new words today? Eight! Say all eight with your hands, fast!”",
        "“Read the goal. Thumbs up, middle or down. How many stars in your last check? Show me with your fingers.”",
        "“Open your screen. Press ‘Mark as done’. Close your laptop.”",
        "Praise: “Eight new words in one lesson. You worked hard in every place. I am proud of you!”",
    ], find_par(rat, "To review").runs[0])
    fill_scan(scan, [
        "name the missing word from memory;",
        "say all eight words with their gestures;",
        "rate themselves against the goal and show their Last-check stars.",
    ])
    highlight_options(inter, ["whole class"])

    d.save(OUT)
    shutil.copyfile(OUT, OUT_HOME)
    print("wrote", OUT, "and", OUT_HOME)


if __name__ == "__main__":
    main()
