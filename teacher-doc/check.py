# /// script
# dependencies = ["python-docx"]
# ///
"""Check the Classroom guide against classroom.md. Run: uv run teacher-doc/check.py

Fails unless the Word file holds the link (and no #teacher or #level- link, no microphone or recording line), every line of the Instructions block exactly as in classroom.md, the title
and the Hebrew note; every Hebrew paragraph is marked right-to-left (bidi); and the PDF's text (pdftotext) holds the
link and Hebrew letters.
"""
import re
import subprocess
import sys
from pathlib import Path

import docx
from docx.oxml.ns import qn

HERE = Path(__file__).resolve().parent
sys.dont_write_bytecode = True  # no __pycache__ left in the repo by importing build
sys.path.insert(0, str(HERE))
from build import read_source, OUT, PDF  # noqa: E402  (one parser for both, so the check reads what the build read)

HEB = re.compile(r"[֐-׿]")
fails = []
ok = lambda c, m: c or fails.append(m)

src = read_source()
ok(OUT.stat().st_size > 0 if OUT.exists() else False, f"missing {OUT.name}")
ok(PDF.stat().st_size > 0 if PDF.exists() else False, f"missing {PDF.name}")
d = docx.Document(OUT)
paras = list(d.paragraphs) + [p for t in d.tables for row in t.rows for c in row.cells for p in c.paragraphs]
texts = [p.text for p in paras]
xml = d.element.xml + "".join(r.target_ref for r in d.part.rels.values() if r.is_external)

ok(src["link"] in "\n".join(texts) and src["link"] in xml, "the link is not in the Word file")
ok(src["title"] in texts, f"the post title {src['title']!r} is not a paragraph of its own")
for line in src["block"]:
    if line:
        ok(line in texts, f"Instructions line missing or changed: {line!r}")
for lines in src["note"]:
    for line in lines:
        ok(line in texts, f"note line missing or changed: {line[:40]!r}")
ok(any("מסך מלא" in t for t in texts), "the Hebrew Instructions line does not mention מסך מלא")
heb = [p for p in paras if HEB.search(p.text)]
ok(len(heb) > 0, "no Hebrew paragraphs")
for p in heb:
    ppr = p._p.pPr
    ok(ppr is not None and ppr.find(qn("w:bidi")) is not None, f"Hebrew paragraph not bidi: {p.text[:40]!r}")

pdf = subprocess.run(["pdftotext", "-layout", str(PDF), "-"], capture_output=True, text=True).stdout
# His words: one link for the class; no projector link and no per-place links where a kid could see them; never record.
for name, body in (("Word file", xml + "\n".join(texts)), ("PDF", pdf)):
    for bad in ("#teacher", "#level-", "מיקרופון", "microphone", "Microphone", "הקלטה"):
        ok(bad not in body, f"{name} still holds {bad!r}")
ok(src["link"] in pdf, "pdftotext: the link is not in the PDF")
ok(len(HEB.findall(pdf)) > 0, "pdftotext: no Hebrew letters in the PDF")

for f in fails:
    print("FAIL", f)
print(f"{len(fails)} failure(s)" if fails else f"all passed: {len(texts)} paragraphs, {len(heb)} Hebrew, PDF text {len(pdf)} chars")
sys.exit(1 if fails else 0)
