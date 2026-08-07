# demo.tape — VHS script for clackr's README / release demo GIF.
#
# Regenerated manually via `.github/workflows/demo.yml` (workflow_dispatch
# only), never on every merge — see that workflow for why. Run locally with:
#   vhs demo.tape
#
# The `clackr` binary doesn't exist yet as of this commit (M8 lands ahead of
# the TUI being wired up), but this tape is written now for the intended v1
# UX: launch straight into a short words-mode test, type it at a human pace
# with one realistic typo + correction (a typo-free demo wouldn't show off
# the correct/incorrect/extra coloring that's the whole point of the
# engine), then hold on the results screen long enough to actually read it.

Output demo.gif

Set Shell "bash"
Set FontFamily "JetBrains Mono"
Set FontSize 20
Set Width 1200
Set Height 650
Set Padding 20
Set Theme "Dracula"
Set TypingSpeed 60ms

# Launch straight into a short, demo-friendly test.
Type "clackr --mode words --words 10"
Enter
Sleep 1s

# Ten words, typed at a human pace with one realistic typo + correction
# partway through, instead of one long instantly-inserted string.
Type "the quick brown "
Sleep 200ms
Type "foxx"
Sleep 300ms
Backspace 2
Sleep 150ms
Type "x "
Sleep 150ms
Type "jumps over the lazy "
Sleep 250ms
Type "dog "
Sleep 150ms
Type "while "
Sleep 200ms
Type "typing"
Sleep 200ms
Space

# Hold on the results screen (hero wpm/acc block, WPM + raw + errors
# chart, bottom stat row) long enough for a viewer to read it.
Sleep 4s
