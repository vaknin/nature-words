# /// script
# dependencies = ["python-docx"]
# ///
"""Build the Classroom guide for the class teacher: a designed A4 Word file and its PDF.

Run: uv run teacher-doc/build.py
Writes teacher-doc/Nature-Words-Classroom-Guide.docx and teacher-doc/Nature-Words-Classroom-Guide.pdf.

The note to the teacher, the post title, the Instructions block and the one class link are read from classroom.md
at build time, so the document and classroom.md never drift. One link only: no per-place links and no projector
link, since the guide goes to the class teacher and its post goes to the kids. The PDF comes from LibreOffice headless:
soffice from $SOFFICE, else the extracted AppImage left by the lesson-plan build, else a fresh AppImage download.
"""
import os
import re
import shutil
import subprocess
import tempfile
import urllib.request
from pathlib import Path

import docx
from docx.enum.table import WD_TABLE_ALIGNMENT
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.oxml import OxmlElement
from docx.oxml.ns import qn
from docx.shared import Cm, Pt, RGBColor

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
NAME = "Nature-Words-Classroom-Guide"
OUT = HERE / f"{NAME}.docx"
PDF = HERE / f"{NAME}.pdf"
SOFFICE_KNOWN = Path("/tmp/lp1616lo/squashfs-root/opt/libreoffice25.8/program/soffice")
APPIMAGE_URL = "https://appimages.libreitalia.org/LibreOffice-fresh.standard-x86_64.AppImage"

# Colours are the site's own (index.html :root), so the paper matches what the kids see.
INK = RGBColor(0x1F, 0x2A, 0x3D)      # --ink: body text
MUTED = RGBColor(0x5D, 0x6B, 0x82)    # --muted: small print
RIVER = RGBColor(0x2F, 0x80, 0xED)    # --river: headings and links
WHITE = RGBColor(0xFF, 0xFF, 0xFF)
FOREST_HEX = "1F9D63"                 # --forest: the title band (the post's 🌲)
BOX_HEX = "F4F8FB"                    # --bg: the copy-me boxes, light enough to print cheaply
LINE_HEX = "C9D6E6"                   # a shade darker than --line, so the box edge survives a grey printer

FONT = "Arial"  # his note: the teacher's Windows has Arial with Hebrew; LibreOffice here maps it to Liberation Sans.
# Sizes in points. 11 pt body fits the whole guide on two A4 pages and stays easy to read on paper;
# the title is three times body so the band reads from across a desk; the link is twice body ("large type").
BODY, SMALL, HEADING, TITLE, LINK = 11, 9.5, 15, 34, 20
# A4 with 2 cm margins: Word's narrowest "normal" margin that every school printer handles.
PAGE_W, PAGE_H, MARGIN = Cm(21), Cm(29.7), Cm(2)

HEB = re.compile(r"[֐-׿]")


# ---------- reading classroom.md ----------
def read_source():
    md = (ROOT / "classroom.md").read_text(encoding="utf-8")
    lines = md.splitlines()
    link = re.search(r"The link: \*\*(https://\S+?)\*\*", md).group(1)
    # The note: the "> " blockquote under its heading; a bare ">" splits paragraphs.
    i = lines.index("## Note to forward to the class teacher") + 1
    while not lines[i].startswith(">"):
        i += 1
    note, para = [], []
    while i < len(lines) and lines[i].startswith(">"):
        t = lines[i][1:].strip()
        if t:
            para.append(t)
        elif para:
            note.append(para); para = []
        i += 1
    if para:
        note.append(para)
    title = re.search(r"\*\*Title:\*\* `([^`]+)`", md).group(1)
    # The Instructions block: the fenced block after "**Instructions:**", dedented by the fence's own indent.
    j = next(k for k, l in enumerate(lines) if "**Instructions:**" in l) + 1
    fence = lines[j]
    indent = len(fence) - len(fence.lstrip())
    assert fence.strip() == "```", fence
    block = []
    j += 1
    while lines[j].strip() != "```":
        block.append(lines[j][indent:] if lines[j].strip() else "")
        j += 1
    return dict(link=link, note=note, title=title, block=block)


# ---------- low-level Word helpers ----------
def set_run(run, size=BODY, bold=False, color=INK, rtl=None):
    """One font for Latin and Hebrew (Arial), with the complex-script size, bold and language set too,
    because Word draws Hebrew with the cs properties, not the Latin ones."""
    text = run.text
    if rtl is None:
        rtl = bool(HEB.search(text))
    run.font.name = FONT
    run.font.size = Pt(size)
    run.font.bold = bold
    run.font.color.rgb = color
    rpr = run._r.get_or_add_rPr()
    fonts = rpr.find(qn("w:rFonts"))
    for a in ("w:ascii", "w:hAnsi", "w:cs", "w:eastAsia"):
        fonts.set(qn(a), FONT)
    if bold:
        rpr.append(OxmlElement("w:bCs"))
    szcs = OxmlElement("w:szCs"); szcs.set(qn("w:val"), str(int(size * 2))); rpr.append(szcs)
    if rtl:
        rpr.append(OxmlElement("w:rtl"))
    lang = OxmlElement("w:lang"); lang.set(qn("w:val"), "en-US"); lang.set(qn("w:bidi"), "he-IL"); rpr.append(lang)
    return run


def para(container, text="", *, rtl=None, size=BODY, color=INK, align=None, before=0, after=4, bold_all=False):
    """A paragraph from text with **bold** spans. Hebrew text makes it a right-to-left (bidi) paragraph."""
    p = container.add_paragraph()
    if rtl is None:
        rtl = bool(HEB.search(text))
    if rtl:
        ppr = p._p.get_or_add_pPr()
        ppr.insert(0, OxmlElement("w:bidi"))
    # No w:jc on a bidi paragraph: its default start edge is the right one in both Word and LibreOffice, while
    # an explicit "right" means "end" (the left edge) to some readers of transitional OOXML.
    if align is not None:
        p.alignment = align
    elif not rtl:
        p.alignment = WD_ALIGN_PARAGRAPH.LEFT
    pf = p.paragraph_format
    pf.space_before, pf.space_after = Pt(before), Pt(after)
    for k, seg in enumerate(re.split(r"\*\*", text)):
        if seg:
            # Inside a Hebrew paragraph, each run is marked rtl only if it holds Hebrew: English button names stay LTR.
            set_run(p.add_run(seg), size=size, bold=bold_all or k % 2 == 1, color=color)
    return p


def shade(cell, hex_fill, border_hex=None):
    tcpr = cell._tc.get_or_add_tcPr()
    shd = OxmlElement("w:shd"); shd.set(qn("w:val"), "clear"); shd.set(qn("w:color"), "auto"); shd.set(qn("w:fill"), hex_fill)
    tcpr.append(shd)
    borders = OxmlElement("w:tcBorders")
    for side in ("top", "left", "bottom", "right"):
        b = OxmlElement(f"w:{side}")
        if border_hex:
            # 8 eighths of a point = 1 pt: a visible edge that is not a heavy frame.
            b.set(qn("w:val"), "single"); b.set(qn("w:sz"), "8"); b.set(qn("w:color"), border_hex)
        else:
            b.set(qn("w:val"), "nil")
        borders.append(b)
    tcpr.append(borders)
    mar = OxmlElement("w:tcMar")
    # 180 twentieths = 9 pt of padding: text never touches the box edge.
    for side in ("top", "left", "bottom", "right"):
        m = OxmlElement(f"w:{side}"); m.set(qn("w:w"), "180"); m.set(qn("w:type"), "dxa"); mar.append(m)
    tcpr.append(mar)


def box(doc, hex_fill, border_hex=None):
    """A one-cell, full-width table: the title band and the copy-me boxes."""
    t = doc.add_table(rows=1, cols=1)
    t.alignment = WD_TABLE_ALIGNMENT.CENTER
    cell = t.cell(0, 0)
    cell.width = PAGE_W - 2 * MARGIN
    shade(cell, hex_fill, border_hex)
    cell.paragraphs[0]._p.getparent().remove(cell.paragraphs[0]._p)
    return cell


def hyperlink(p, url, size, bold=True):
    """A real clickable link (python-docx has no API for it): a w:hyperlink holding one styled run."""
    rid = p.part.relate_to(url, "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink", is_external=True)
    h = OxmlElement("w:hyperlink"); h.set(qn("r:id"), rid)
    r = p.add_run(url)
    set_run(r, size=size, bold=bold, color=RIVER, rtl=False)
    u = OxmlElement("w:u"); u.set(qn("w:val"), "single"); r._r.get_or_add_rPr().append(u)
    h.append(r._r)
    p._p.append(h)


def heading(doc, text):
    # Space above separates sections without rules or boxes; the colour ties headings to the links.
    # Kept with the next paragraph, so a heading never ends a page alone.
    p = para(doc, text, size=HEADING, color=RIVER, before=12, after=4, bold_all=True)
    p.paragraph_format.keep_with_next = True
    return p


# ---------- the document ----------
def build(src):
    doc = docx.Document()
    sec = doc.sections[0]
    sec.page_width, sec.page_height = PAGE_W, PAGE_H
    sec.left_margin = sec.right_margin = sec.top_margin = sec.bottom_margin = MARGIN
    normal = doc.styles["Normal"]
    normal.font.name, normal.font.size = FONT, Pt(BODY)
    normal.element.get_or_add_rPr().get_or_add_rFonts().set(qn("w:cs"), FONT)

    # Title band: the post's own name and the eight words, white on the forest green.
    band = box(doc, FOREST_HEX)
    para(band, "Nature Words", size=TITLE, color=WHITE, align=WD_ALIGN_PARAGRAPH.CENTER, after=2, bold_all=True)
    para(band, "air · dry · forest · grass · mountain · nature · river · sea", size=HEADING, color=WHITE,
         align=WD_ALIGN_PARAGRAPH.CENTER, after=4)
    para(band, "מדריך קצר למחנכת: פרסום השיעור ב-Google Classroom", size=BODY + 1, color=WHITE,
         align=WD_ALIGN_PARAGRAPH.CENTER, after=0)

    heading(doc, "הודעה למחנכת")
    for lines in src["note"]:
        for k, line in enumerate(lines):
            para(doc, line, after=2 if k < len(lines) - 1 else 8)

    heading(doc, "הקישור (אחד לכל הכיתה)")
    p = para(doc, "", rtl=False, align=WD_ALIGN_PARAGRAPH.CENTER, before=2, after=6)
    hyperlink(p, src["link"], LINK)

    heading(doc, "הטקסט לפוסט (להעתקה כמו שהוא)")
    para(doc, "כותרת (**Title**):", after=2).paragraph_format.keep_with_next = True
    tb = box(doc, BOX_HEX, LINE_HEX)
    para(tb, src["title"], rtl=False, size=BODY + 3, after=0, bold_all=True)
    para(doc, "הוראות (**Instructions**):", before=8, after=2).paragraph_format.keep_with_next = True
    ib = box(doc, BOX_HEX, LINE_HEX)
    for line in src["block"]:
        para(ib, line, after=2)
    para(doc, "שם הכפתור \"סימון כבוצע\" (**Mark as done**) נכתב מהזיכרון: אם אצלך הוא שונה, כדאי להעתיק אותו מהמסך.",
         size=SMALL, color=MUTED, before=4)

    heading(doc, "שלב אחר שלב ב-Classroom")
    steps = [
        "1. פותחים את הכיתה, ואז את הלשונית **Classwork**.",
        "2. לוחצים **+ Create** ואז **Assignment** (מטלה נותנת לכל תלמיד כפתור **Mark as done**, וכך רואים מי סיים).",
        "3. בשדה **Title** מדביקים את הכותרת מהמסגרת הראשונה.",
        "4. בשדה **Instructions** מדביקים את ההוראות מהמסגרת השנייה.",
        "5. תחת **Attach** לוחצים **Link**, מדביקים את הקישור ולוחצים **Add link**. זה הקובץ המצורף היחיד, כך שלתלמיד יש דבר אחד ללחוץ עליו.",
        "6. ב-**Points** בוחרים **Ungraded**, ב-**Due** לא בוחרים תאריך, וב-**For** מסמנים את כל תלמידי הכיתה.",
        "7. לוחצים **Assign** (או על החץ שלידו ואז **Schedule**, לעשר דקות לפני השיעור: פוסט מתוזמן עלול להגיע באיחור של כמה דקות).",
    ]
    for s in steps:
        para(doc, s, after=3)

    heading(doc, "מה התלמידים עושים")
    para(doc, "כל התלמידים פותחים את אותו קישור. חידון קצר של 8 שאלות שולח כל תלמיד למקום שמתאים לו, "
              "מתוך שלושה: **River**, **Forest** או **Mountain**. שם הוא משחק במילים: מקשיב, אומר בקול, לוחץ ובונה מילים. "
              "בסוף יש מרוץ של 30 שניות עם לוח 5 המובילים של הכיתה. אין צורך בשום קישור נוסף.", after=3)

    heading(doc, "אם משהו לא עובד")
    tips = [
        "**אין קול במחשב:** הקולות מוקלטים בתוך הדף, אז הבעיה בעוצמת השמע או באוזניות, לא באינטרנט.",
        "**האינטרנט נפל:** דף שכבר נפתח ממשיך לעבוד, הוא לא צריך חיבור אחרי שנטען.",
        "**תלמיד סגר את הלשונית:** פותח שוב את הקישור ולוחץ **Continue**. המחשב זוכר איפה הוא עצר היום.",
        "**יצא ממסך מלא:** לוחצים F11 (בחלק מהמחשבים Fn+F11).",
    ]
    for t in tips:
        para(doc, "• " + t, after=3)
    para(doc, "האתר לא דורש הרשמה. שם פרטי נכתב רק ללוח 5 המובילים במרוץ, ונמחק לבד אחרי 12 שעות. תודה רבה על העזרה!", size=SMALL, color=MUTED, before=8)

    doc.core_properties.title = "Nature Words: Classroom guide"
    doc.save(OUT)


# ---------- the PDF ----------
def soffice():
    env = os.environ.get("SOFFICE")
    if env and Path(env).exists():
        return Path(env), None
    if SOFFICE_KNOWN.exists():
        return SOFFICE_KNOWN, None
    tmp = Path(tempfile.mkdtemp(prefix="nw-lo-"))
    img = tmp / "lo.AppImage"
    print("downloading LibreOffice AppImage ...")
    urllib.request.urlretrieve(APPIMAGE_URL, img)
    img.chmod(0o755)
    subprocess.run([str(img), "--appimage-extract"], cwd=tmp, check=True, stdout=subprocess.DEVNULL)
    exe = next((tmp / "squashfs-root").glob("opt/libreoffice*/program/soffice"))
    return exe, tmp


def to_pdf():
    exe, cleanup = soffice()
    with tempfile.TemporaryDirectory(prefix="nw-pdf-") as out, tempfile.TemporaryDirectory(prefix="nw-prof-") as prof:
        subprocess.run([str(exe), f"-env:UserInstallation=file://{prof}", "--headless", "--convert-to", "pdf",
                        "--outdir", out, str(OUT)], check=True, stdout=subprocess.DEVNULL, timeout=180)
        shutil.move(Path(out) / f"{NAME}.pdf", PDF)
    if cleanup:
        shutil.rmtree(cleanup, ignore_errors=True)


if __name__ == "__main__":
    build(read_source())
    to_pdf()
    print("wrote", OUT, "and", PDF)
