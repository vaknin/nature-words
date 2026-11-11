# Student sign-in: class code, name, secret picture (research, 2026-11-11)

Scope: 5th grade (10-11 years), Israel, shared Asus Windows laptops, browser, no password. Stored: display name, picture id, level, words known, best race scores. Not legal advice (see section 5).

Honesty note: I opened some sources and only saw others in search results. Each line says which. Several sites (Seesaw, ClassDojo, Sony) blocked direct fetch (HTTP 403), so those claims rest on search-result text.

## Recommendation (numbers we build on)

| Item | Number | Reason |
|---|---|---|
| Picture grid | 16 pictures, 4x4, tiles about 120 px with 12 px gaps (about 516 px square, fits a 1366x768 screen) | Lowest blind-guess rate that still fits big tiles. 120 px is far above the WCAG 24 px minimum and 44 px enhanced size, and children miss small targets more than adults (section 2). All 16 must be clearly different animals ("distinct objects" recalled best, [Assal et al.](https://arxiv.org/abs/1610.09743), 2016-10-31). 12 (4x3, tiles about 140 px) is an acceptable fallback if 16 animals are hard to tell apart. |
| Wrong tries before a pause | 3 | A kid who mis-taps or half-remembers gets a fair go; a blind guesser gets only a few shots (table below). |
| Pause length | 60 seconds for the first pause | Short enough that the owner is not stuck; long enough to stop rapid tapping. |
| Does the pause grow | Yes: 60 s, then 5 min, then locked for the rest of the lesson until the teacher taps "unlock" (6 wrong tries total) | A pause alone does not protect a 45-minute lesson (table below). Only a hard cap per record does. NIST allows up to 100 failures, so a cap of 6 is well inside it ([NIST SP 800-63B-4](https://pages.nist.gov/800-63-4/sp800-63b/authenticators), date not shown). |
| Session length | 45 minutes, then sign-in is forgotten on the laptop (new class code each lesson) | A lesson in Israeli schools is 45 minutes (anecdotal, section 2). Shared laptops pass between kids, so sign-in must not outlive the lesson. |
| Personal data | Display name (first name + family initial, or nickname) only; no email, birth date, full name, or IP log tied to a kid | Privacy by minimisation (section 5). |

Plain truth: with 16 pictures a friend who gets all 6 wrong tries has at most a 6 in 16 (37.5%) guess chance on one named kid, and 3 in 16 (19%) before the first pause. No grid a 10-year-old can use makes blind guessing hard. The picture password only stops casual snooping; it is not a lock. That is acceptable only because the stored data is low-stakes (word progress and race scores), the friend also needs the class code, and the teacher can reset.

## 1. Picture passwords for children: products and research

### Products (what they do)

- Clever Badges: a printed QR card a young student holds up to the laptop camera to sign in. Introduced for K-2 in [VentureBeat](https://venturebeat.com/business/clevers-new-badges-help-k-2-students-access-apps-on-its-education-platform) (2016-04-19, opened). Districts say lost badges must be voided and regenerated, and anyone holding the code can get in, so treat it like a password ([search result summary of district pages](https://www.sfusd.edu/school/lafayette-elementary-school/students/digital-backpack), date not shown). Clever's own Feb 2026 notice says Badges and the typed login "work the same as before" and nothing about picture login changed ([SFUSD notice](https://www.sfusd.edu/announcements/2026-02-03-clevers-login-screen-has-new-look), 2026-02-03, opened). So Clever Badges are QR cards, not a "pick your picture" grid.
- Seesaw: students sign in with a per-student QR code or text code; the teacher can reset codes any time if shared ([Seesaw help](https://help.seesaw.me/hc/en-us/articles/206447475-How-to-reset-student-passwords), date not shown, search text only, page blocked). Seesaw warns the class code must not be used at home because it exposes other students' journals (same search).
- ClassDojo: students use a class QR code and then pick their name from the class list, or an individual QR code; fallback is a 6-letter code ([ClassDojo help](https://help.classdojo.com/hc/en-us/articles/360023367251-How-Students-Can-Login-With-an-Individual-QR-Code), date not shown, search text only). This is the same "code, then tap your name" shape we plan, minus the picture.
- Epic: class code, then tap your profile; PIN is optional ([Epic support](https://support.getepic.com/hc/en-us/articles/115001263046-How-do-my-students-log-into-my-educator-classroom), date not shown, search text only). A school page names the picture-password product as Raz-Kids / Reading A-Z (a class picture), not Epic.
- Microsoft Windows picture password: Windows 8 era only; three gestures on a photo, falls back to a text password after five incorrect gesture guesses, admins can disable it ([itnews](https://www.itnews.com.au/news/windows-8-offers-picture-gesture-password-284842), date not shown; [Sony support](https://www.sony.co.nz/electronics/support/articles/S500024695), last modified 2018, page blocked). A USENIX 2013 paper ([PDF](https://cse.buffalo.edu/~hongxinh/papers/USENIX2013.pdf), seen in search, not opened) says such schemes have smaller password spaces than text. Not an education product and not child-specific.
- Google Workspace for Education: I found no native picture password or QR badge from Google. Young-student QR logins on Chromebooks come from Clever and ClassLink QuickCards ([district page](https://sites.google.com/tempeschools.org/td3-edtech/tech-support/sso-qr-code-badges), date not shown).

### Research on children

- Assal, Imran, Chiasson: [An Exploration of Graphical Password Authentication for Children](https://arxiv.org/abs/1610.09743) (2016-10-31, opened abstract). Children recalled passwords best with images of distinct objects; age range and numbers not on the page.
- Ratakonda, Mehrpouyan, Fails: ["Pictures are easier to remember than spellings!" KidsPic](https://experts.boisestate.edu/en/publications/pictures-are-easier-to-remember-than-spellings-designing-and-eval/) (Sep 2022, opened). 40 children aged 6-11 had significantly more memorability trouble with alphanumeric passwords than with the picture scheme. KidsPic is a picture sequence, not a single pick.
- Renaud, Volkamer, Mayer, Grimm: [Principles for Designing Authentication Mechanisms for Young Children (KidzPass)](https://aisel.aisnet.org/thci/vol13/iss4/2) (2021, opened abstract). Ages 4-7 preferred KidzPass to text; personalised images (faces, own doodles) are most memorable but slow to collect; generic images can work for older children if they have certain qualities (the page does not list them).

Where this leaves us: pictures beat text for 6-11 year olds, and distinct objects are recalled best. I found no study of a single-picture-from-a-grid password for 10-11 year olds returning after days or weeks, and no figure for how many pictures a 10-year-old can tell apart.

## 2. Grid size, guess rate, touch size, lesson length

Model: friend targets one named kid, taps blind guesses, about 3 seconds per tap, in a 45-minute lesson. Worst case assumes the friend never repeats a wrong picture (chance = guesses / N). With a shuffled grid and no memory it is slightly lower (1 - (1 - 1/N)^guesses).

Without any cap, a pause only slows guessing. Guesses possible in 45 minutes (computed): 3 wrong tries then 30 s pause = 210; 3 then 60 s = 120; 3 then 60 s doubling each time = 21. Even 21 guesses beats every grid up to 16. So pauses alone fail; a cap per record is needed.

Blind-guess chance by number of wrong tries allowed per lesson (G/N, rounded):

| Grid N | 1 guess | 3 wrong tries then lock | 6 wrong tries then lock | 21 (pause only, doubling) |
|---|---|---|---|---|
| 4 | 25% | 75% | 100% | 100% |
| 6 | 17% | 50% | 100% | 100% |
| 9 | 11% | 33% | 67% | 100% |
| 12 | 8% | 25% | 50% | 100% |
| 16 | 6% | 19% | 38% | 100% |

Touch size on 1366x768 (usable viewport is roughly 1366x650 after browser bars; my estimate):
- WCAG 2.2 Target Size (Minimum): at least 24x24 CSS px ([W3C Understanding 2.5.8](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html), updated 2026-11-02, opened). Enhanced (AAA) is 44x44 ([search summary](https://www.disabilityworld.org/toolkit/standards/wcag/2-5-5-target-size-enhanced/), date not shown).
- Children miss targets more: about 46% vs 32% for adults in Brown and Anthony ([CHI 2012 PDF](https://lisa-anthony.com/wp-content/uploads/2012/03/brown-and-anthony-chi2012eist.pdf), 2012; figures from a search summary, paper not opened; these were younger children on tablets). A 2020 study of 54 children aged 5-10 found standard Fitts' law with nominal widths fits well ([Woodward et al. AVI 2020](https://init.cise.ufl.edu/wp-content/uploads/sites/378/2021/05/woodward-et-al-AVI2020.pdf), search text only).
- Fit: 16 tiles at 120 px fit (516 px square). 25 tiles at 100 px fit but animals get small. Mouse or touchpad on a school laptop is easier than a finger, so 120 px is generous.

Lesson length: lessons are 45 minutes with a short break after, per a teacher forum post ([ESL Printables forum](https://eslprintables.com/forum/topic.asp?id=42519), about 2013, anecdotal). I found no official Ministry of Education page stating it. Confirm with the teacher.

## 3. Shoulder surfing

- A friend beside or behind a kid can see which picture is tapped, or see the hover/highlight. Research on graphical schemes: re-randomising picture locations at each attempt is a known defence, but one academic comparison found both fixed and re-randomised variants still observable by a watcher, who could recover the password from one login ([Ho et al., via search results](https://arxiv.org/pdf/0912.0951), date not shown, not opened). So shuffling helps but is not a cure. A scheme with a session-specific randomised grid reported 0% attacker success in its authors' own test ([LocPass, MDPI](https://mdpi.com/2073-8994/11/10/1252), 2019 per URL range, authors' claim, not child-focused).
- Cheap mitigations for us: shuffle positions every time; no hover or highlight; the tap shows a neutral press for every tile, then the same "checking" screen for right or wrong; chosen picture never shown large or named after sign-in; the first pick happens in a private step (screen tilt, teacher says "turn your head away"); teacher can reset any picture; show only the kid's own name in the grid step so a watcher learns nothing about others.
- No child-specific classroom shoulder-surfing study found.

## 4. Teacher reset path

- Seesaw: teacher resets student codes in class settings, all or one student ([Seesaw help](https://help.seesaw.me/hc/en-us/articles/206447475-How-to-reset-student-passwords), search text only).
- Clever Badges: void the lost badge, then generate a new one; voiding cannot be disabled in one district's setup ([district page](https://www.sfusd.edu/school/lafayette-elementary-school/students/digital-backpack), date not shown).
- Our design: teacher taps the kid's name, "clear picture"; the next sign-in goes straight to "pick your picture" after the class code and name. A lock from too many wrong tries is cleared the same way. Keep progress; only the picture id is erased.

## 5. Least personal data and Israeli privacy (not legal advice)

This is a research summary, not legal advice. "The school's OK" means the school agreed to the tool; it does not by itself prove each rule below is met. Ask the school's privacy or information-security officer to confirm.

- Law: Amendment 13 to the Privacy Protection Law, 1981, entered into force 14 August 2025; the Privacy Protection Authority gained wider enforcement and published guidance and a compliance hub ([Pearl Cohen](https://www.pearlcohen.com/israel-significant-amendment-to-the-privacy-law-takes-effect/), 2025-09-03, opened). That page does not cover minors, definitions, or fines. A search summary says no dedicated child-consent age exists and a 100,000-person threshold applies to some sensitive-data duties ([law.co.il summary](https://www.law.co.il/en/news/internet-law-update/?page=102), date not shown); I could not verify this.
- Security: Privacy Protection (Data Security) Regulations 5777-2017, in force 2018, as cited by the State Comptroller ([report PDF](https://library.mevaker.gov.il/sites/DigitalLibrary/Documents/69b/2019-69b-101-Hagana.pdf), 2019 per filename, not opened in full). Not read directly.
- Students: the school-side rules page lists the Director-General circular on privacy on school websites (תשס"ג/7(א), 2003-03-02), the ethics-online circular (תשפ"ב/4 as written on the page), and the Privacy Protection Authority guide "Student Privacy Guide in Educational Institutions in the Digital Age"; publishing student details needs written parent and student consent and may not include home address, phone, or grades ([Kol Zchut](https://www.kolzchut.org.il/he/%D7%96%D7%9B%D7%95%D7%AA_%D7%94%D7%AA%D7%9C%D7%9E%D7%99%D7%93_%D7%9C%D7%9B%D7%91%D7%95%D7%93_%D7%95%D7%9C%D7%A4%D7%A8%D7%98%D7%99%D7%95%D7%AA), updated 2025-09-09, opened). I could not open the guide itself.
- Ministry guidance: a Ministry page says to check that a tool is Ministry-approved and that users' data is protected, and (Hebrew) "do not use a service that requires registering students" ([Ministry data-security page](https://pop.education.gov.il/sherutey-tiksuv-bachinuch/data-security-e-learning/), date not shown, opened). Our no-account, name-only design fits that line. A Ministry cloud-policy PDF exists ([policy-cloud-edu-ins-1.4](https://meyda.education.gov.il/files/PortalShaar/policy-cloud-edu-ins-1.4.pdf), file dated 2024-08-13) but I could not read its text.
- Our minimum: first name + family initial or nickname, picture id, level, words, scores. No email, birth date, full name, phone, photo, IP-address log, or device id tied to a kid. Server logs should drop or truncate IPs and not join them to a kid record. Do not use the name to look anyone up elsewhere. Say plainly to kids and parents which fields exist and that the teacher can delete a record.

## Where research is silent

- No study found of a single-picture-from-a-grid password for 10-11 year olds, nor memory over weeks.
- No source for how many animal pictures a 10-year-old can tell apart; 16 is my judgment, to be tested with a few kids.
- No Kahoot, Prodigy, Boddle, or Microsoft Learning Tools sign-in source found; no native Google picture password found.
- No child-specific shoulder-surfing study; no current (2026) source on picture-password products beyond Clever's Feb 2026 notice.
- No official source for 45-minute lessons; no reliable grade-5 touch-target size standard found (only WCAG and a 2012 study).
- Israeli sources: could not open the Privacy Protection Authority student guide, the 2017 regulations text, or the Ministry cloud policy; I found no circular specifically on external sites in classrooms. Whether our site needs registration as a database or a named privacy lead is unanswered.
- Product-help pages (Seesaw, ClassDojo, Epic) were read only through search text; dates not shown.
