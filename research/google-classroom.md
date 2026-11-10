# Google Classroom for a 5th-grade, one-laptop-each vocabulary lesson

Short and practical. What each choice means for this lesson is in **bold**.

## 1. Keep the three levels private: assign each to selected students
- An assignment can go to chosen students: on the form, open the "All students" menu, untick it, tick the names. Up to 100 students at a time. Source: Google Help, *Create an assignment* — https://support.google.com/edu/classroom/answer/6020265?hl=en
- Students who are not assigned an item do not see it in Classroom (teacher guides; Google's page does not say it explicitly). https://ditchthattextbook.com/5-ideas-for-using-google-classrooms-new-differentiation-and-grouping-feature/
- **Student Groups** (2024, extended April 2025): assign to a saved group from the "Assign to" button, Groups tab. Students can't see which group an item went to, the group's name, or the other members. Creating/editing groups needs a **premium (Education Plus / Teaching & Learning) licence**; any teacher can assign to groups that already exist. Source: Google Help, *Manage student groups* — https://support.google.com/edu/classroom/answer/15263790?hl=en ; launch post — https://workspaceupdates.googleblog.com/2024/08/student-groups-in-google-classroom.html ; 2025 update — https://workspaceupdates.googleblog.com/2025/04/new-student-group-capabilities-google-classroom.html
- **For us:** three assignments ("Level 1/2/3" are names only the teacher sees in his view; the kids see a friendly title such as "Nature Quest 🌲"), each with its own link `…/#level-1`, `#level-2`, `#level-3`, each assigned to its kids by ticking names (or a group if his school has the licence). A kid sees only his own card, so no kid sees he is "level 1". Unchecked: whether a school without the premium licence shows the Groups tab at all; the tick-names route works on every licence.

## 2. Organise with a Topic; Material vs Assignment
- Topics act as folders on the Classwork page; topics with nothing posted stay hidden from students; topics can be dragged into order. https://sites.google.com/bcsd.com/gobcsdcombis/google-classroom-guidebook/classwork/topic ; https://shakeuplearning.com/blog/how-to-organize-assignments-in-google-classroom/
- A **Material** is a post with attachments and no due date or hand-in; an **Assignment** has "Turn in", optional due date, points and rubric. Each item has one topic. https://support.google.com/edu/classroom/answer/6020265?hl=en ; https://sites.google.com/tamingthetech.org/classroom/the-classwork-page
- **For us:** one Topic "🌍 Nature Words" at the top, holding the three level Assignments (so each kid presses "Mark as done / Turn in" at the end, which gives the teacher a live checklist of who finished) and one Exit-ticket quiz. No Material needed: the projector page is the teacher's.

## 3. Link the site, don't embed it
- A link attached to Classwork opens the URL in a new browser tab. Google Classroom API doc: "Students … click the link Material to open the provided URL in a new tab" — https://developers.google.com/classroom/guides/manage-coursework
- Classroom has no general "embed a web page" for teachers; embedding (iframe) inside Classroom is only for paid **add-ons** built by developers. https://developers.google.com/workspace/classroom/add-ons/walkthroughs/external-attachments
- **For us:** attach the level link with *Attach → Link*. On a laptop the site opens in its own full tab, which is what we want (big buttons, sound, full width). Letter-b was already a GitHub Pages link; same idea. Add the link as the **only** attachment so the kid has exactly one thing to press.

## 4. Exit ticket: Forms quiz assignment
- *Create → Quiz assignment* makes a Google Forms quiz attached to an assignment; grades can be imported to Classroom; on **managed Chromebooks** "Locked mode" stops other tabs. Unmanaged devices can't use locked mode. https://support.google.com/edu/classroom/answer/9095575?hl=en ; https://support.google.com/docs/answer/7634943?hl=en
- **For us:** a 4-question picture quiz (one per word group, self-marking) as the closing exit ticket, assigned to everyone. Locked mode only if the school laptops are managed Chromebooks (unknown: ask). Not required: the site can also end with its own check.

## 5. Rubrics
- Rubrics are attached to an assignment (*Rubric → Create rubric*), up to 50 criteria × 10 levels, reusable, and since mid-2026 Gemini can draft one. You can't edit a rubric once grading started. https://support.google.com/edu/classroom/answer/9335069?hl=en ; https://workspaceupdates.googleblog.com/2026/08/streamlining-rubric-generation-in-Google-Classroom-with-Gemini.html
- **For us:** optional, one tiny 3-row rubric for the final "make it" task (word used right / picture matches / finished). Not needed for a 45-min observed lesson; the observation form asks for feedback, not grades.

## 6. What a kid sees on a laptop
- Teachers can't preview Classroom as a student unless they join with a student account; the "Preview" share link is for other teachers and hides student data. https://alicekeeler.com/2021/09/11/google-classroom-student-view/ ; https://support.google.com/edu/classroom/answer/13768073?hl=en
- Students see the Stream and the Classwork tab; an assignment card shows title, the instruction preview, and the attachment; a "Your work" box with **Turn in / Mark as done**. https://gatlabs.com/education/blog/google-classroom-student-view ; https://dummies.com/article/what-can-students-do-in-google-classroom-144821
- **For us:** the Classroom card is just the door. Title with an emoji, one line of instructions in English + Hebrew, one big link tile. Everything fun happens on our page, which must say from the first second exactly what to press. A test student account (or a colleague's) is the only way to see the real card before class.

## Best practice for 5th graders (summary)
1. One Topic, few items, emoji titles, the link as the only attachment: fewer clicks, fewer lost kids.
2. Post (or schedule) the three level assignments **before** class; the kids press only "Open". Scheduled posts can lag up to 5 min, so post 10 minutes early. https://support.google.com/edu/classroom/answer/6020265?hl=en
3. Use "Turn in" at the end as the finished-signal the teacher watches.
4. Headphones: each kid hears his own clips; otherwise 30 laptops talk at once.
