# First class feedback: the nature lesson, 5th grade (2026-11-11)

Aviv taught this lesson to 5th grade on 11 Nov. His feedback, word for word (task 1708):

> to the lesson plan skill, please add The following as feedback
> add context to the vocabulary so it's easier to learn them in context
> repetition recognition reproductions
> first on the words and then on the written form
> the QR idea was not good. some of them did manage to scan it but it was clunky and it caused a lot of issues. so I think next time we will go for Google classroom, but we need to think of a proper way to share it. and I think I should work with them on the vocabulary first on the screen and only then move towards the games
> the differential instructions were not very good and it wasn't implemented correctly in my opinion
> it wasn't very clear what is happening and for some reason the race was only at the end of mountain instead of for all three levels
> and they couldn't start with the raise right? or view it. somehow they had to finish everything and only then be able to race. so some of them took something like 30 minutes and it was very confusing for them
> another thing is about 90% of students did not use full screen and therefore they did not see the top bar and lost a lot of context and they didn't see the score. so we need some kind of a way to force them into full screen perhaps even enforce it by not letting them continue without full screen
> also, a lot of them had automatic translation from Hebrew to English on their browser so we need to somehow disable that feature as well. I'm not really sure how perhaps you shouldn't be rendered as text and as an image instead or a JavaScript canvas. I'm not really sure
> they did like the race. they really enjoyed it
> I wish we could have a nice way to view the leader board something like top 10 scores even without finishing the entire game, so perhaps be able to view it from the main screen
> or start the race Midway as well
> also the levels
> they really like the levels, at least some of them
> but it was very underwhelming
> the levels didn't give them anything and it didn't really feel satisfying to level up
> so perhaps we want to tap into even more. not addicting behavior but just things that kids love you know like high scores or sound more visual feedback animations just to make it almost addicting to learn English
> some of them really couldn't take their hands off the screen and really enjoyed it, but I think that we can make it even better
> and I want the differentiated learning and instructions to be even better
> perhaps to research that even more perhaps to use a database so we can learn from class to class? remember our students and perhaps give them perhaps even individual differential instructions. so instead of three tiers for 30 students we will have 30 tiers for 30 students
> I'm not really sure how feasible that is but we can at least try
> create the thread with all of this information so we can make the skill even better
> if you remember, we made final pass on the design of the web page, so I want that instead of doing that every time we will just save all of the lessons we've learned about this lesson plan and save it to the skill so we reuse this knowledge instead of reinventing the wheel every time
> perhaps even take what we've done today and somehow make a function from it. so as an example, I give you eight words and you can create a lesson planned for me easily without spending usage or at least not a lot of usage and without working a few hours. but perhaps you can do that in minutes
> we basically need to find a way to automate the process so it will be cheap and fast and individual per student
> but again it's only planned right now. I'm not really sure what do I want but I think we can make it work
> for now, let's focus only on 5th grade
> since this is the only class I will be teaching
> if you've got any questions feel free to ask me

The general lessons from this went into the lesson-site skill (~/.claude/skills/lesson-site/SKILL.md),
with the research in differentiation-research.md and the generator plan in generator-plan.md beside it.
This file keeps what was specific to this lesson. Nothing in the live page is fixed here.

## Why it went wrong in this build (index.html, read from the code)

Line numbers are index.html at commit 1124e59 (the build he taught from).

1. **The race only at the end, reached in practice only at Mountain's end.**
   The race chip in the header is locked until the Last check is done: `raceOpen = s.checked !== undefined`
   in `hud()` (line 729-733), with the lock text "Race opens after the last check" (`locked`/`heLocked`,
   lines 477-478). `checked` is only set when `lastCheck()` finishes (line 891). The race card itself is only
   drawn by `trailEnd()` (lines 976-990). On River's and Forest's end screen the big primary button is
   "next place" (`data-next`, line 983) and the race card sits beside it; Mountain's end has no next place
   (`nx` is undefined), so the race card is the only thing left. The code does open the race at the end of
   every place; that kids met it only at Mountain is inferred from this layout, not reproduced with kids.
   The race is per place (`race(t)`, line 993, board per `t.id`), so there is no start-screen way in.
2. **A kid had to finish everything (~30 min) before racing; no board to look at before then.**
   Each place is four games of `rounds: 8` words each (line 488) plus a comeback round for every missed word
   (`back`, lines 842-859), then the 4-question Last check. The flag that jumps to the Last check early only
   shows `flagMinutes: 11` after the quiz reveal (lines 398-399, used in `shell()` line 822). The top-5 board
   is only drawn by `boardHtml()` inside the race's end card (lines 1036-1039); `start()` (lines 779-791) shows no
   board and no race. So the board could not be viewed and the race could not be started from the start
   screen or mid-place.
3. **~90% never pressed full screen, so the top bar and score were lost.**
   Full screen was only asked for: a button and a hint on the start page (`data-fs-start`, `data-fs-hint`,
   lines 783-788), hidden once full screen (line 83). Nothing stops a kid from pressing Start without it, and
   leaving full screen (Esc) brings nothing back: the `fullscreenchange` listener (line 1179) only toggles a
   class. The header with the place, points, level and race chip (`.thead`, line 102; `shell()` lines 819-827)
   is not sticky, and the pages are laid out to fill exactly 100vh (`.trail{min-height:100vh}`, line 101).
   Measured headless (this task): at 1366x768 a game page does not scroll; in a browser window with its
   tabs and address bar, or with Windows display scaling, it does: at 1366x625 the say game is 689 px tall,
   at 1093x500 (1366x768 at 125% minus the browser bars) the say and build games are 618-652 px tall, so a
   kid scrolls down to the answers and the header with the score scrolls off. The tests only ever ran at
   1366x768 and 1280x720 with no browser bars (test/smoke.mjs `SIZES`), so this never showed.
4. **Browser auto-translate changed the page's language.**
   The page says `<html lang="en">` (line 2), with no `translate="no"`, no
   `<meta name="google" content="notranslate">` and no `notranslate` class anywhere. On a laptop whose
   browser language is Hebrew, Chrome and Edge offer (or, when set to always translate, auto-run) a page
   translation, which rewrites the English words, the questions and the answer buttons. He wrote "from
   Hebrew to English"; the page's English is what a Hebrew-language browser translates, so the likely
   direction is English into Hebrew. Which way it went in class is not checked.
5. **The QR link was clunky.**
   The teacher view's QR (`showQR()`, lines 1167-1175, the Q key on line 1151) shows the class link full
   screen for kids to scan. School laptops' cameras and scanning were never tested, and the Classroom guide
   (teacher-doc/build.py, classroom.md) already gave the one class link; the QR was a second way in that
   added steps.
6. **Level-ups gave nothing.**
   `levelUp()` (line 769) is a 1.6 s toast "Level up! Level N" (`toastMs: 1600`, line 430) with a 4-note
   sound (`SFX.level`, line 589) and confetti. Nothing unlocks, nothing changes in the page or the kid's
   look, and levels come from points alone (`firstLevel: 50, perLevel: 100`, line 427; `levelOf()`, line
   725). The header shows the level only as a small pill from level 2 (line 731).
7. **The differentiation was unclear to kids.**
   The quiz places a kid silently (`result()`, lines 810-817: "Start at River! Well done!") without saying
   what River means or how it differs from Forest or Mountain; the start page shows the three places as
   plain tags (line 787). The three places run the same four games with different option counts and
   letters (lines 373-391), so a kid sees little difference. The move offers (`offer()`, lines 872-881:
   "Want to try Forest?") come after streaks (`streak: {up: 5, down: 3}`, line 420) without saying why.
   That is three tiers for 30 kids, decided by one 8-question quiz, with nothing remembered between
   lessons (`localStorage` keyed by date, line 715).
