# Asset provenance

This file is the record of where every embedded asset came from, under what
license, and what we did to it before it went into `assets/`. It exists so
that a "malformed asset" bug and a "licensing" bug are never the same
investigation: if it's in `assets/`, it's accounted for here.

**MonkeyType (AGPL-3.0) was not used as a source for anything below.**
Bundling its `frontend/static/languages/*.json` or theme collection into
this binary would argue the binary is a derivative work, which would force
AGPL onto a project that ships MIT OR Apache-2.0. Nothing in this repo was
copied from MonkeyType's source tree.

## Word list: `languages/english1k.txt`

**We did not use `first20hours/google-10000-english`**, despite it being
named in the original asset-sourcing plan for this project. On inspection
(2026-08-07) its `LICENSE.md` reads:

> Educational and personal/research use of this data is permitted under the
> LDC license, Norvig's MIT license for his contributions, and US fair use
> doctrine. I do not recommend using this data for commercial purposes
> without licensing it from the Linguistic Data Consortium.

GitHub's own license detector reports `NOASSERTION` for that repository.
That is not a clean permissive license for a binary we intend to distribute
publicly, so per "if you cannot verify a source's license, do not ship that
asset," we did not ship it. We also evaluated and rejected
`hermitdave/FrequencyWords` (content is CC-BY-SA-4.0 -- share-alike, same
category of problem as Wikiquote) and `wordlist.aspell.net/12dicts`
(public domain, but its frequency data is banded into ~20 coarse buckets
rather than individually ranked, and pulling a clean top-1000 out of it was
not tractable in the time available).

**What we did instead:** we derived a first-pass `english1k.txt` from
scratch, by frequency-counting the ten Project Gutenberg texts listed
below (all confirmed pre-1929 / US public domain), and then hand-curated
that frequency list into the version actually shipped. This sidesteps the
licensing question entirely -- there is no third-party word list being
redistributed at any point. Step 1 is an original frequency analysis we
ran over public-domain raw text, the same way `first20hours/google-10000-english`
did over its own (differently-licensed) corpus. Step 2 is our own
editorial selection on top of that analysis -- individual English words
are not copyrightable, and a curated list of which 1000 to include, in
what order, is original expression authored for this project, not a
redistribution of anyone else's compilation.

**Step 1 -- frequency analysis (corpus-derived, mechanical).** Downloaded
each `pg<id>.txt` from gutenberg.org, stripped the Gutenberg header/footer
between the `*** START OF ... ***` / `*** END OF ... ***` markers (the
boilerplate itself carries a trademark license from Project Gutenberg and
is not meant to be redistributed as part of the "work"; the underlying
book text is public domain), tokenized on `[A-Za-z]+`, discarded any token
touching an apostrophe (so contractions like "don't" contribute nothing,
rather than leaking fragments like "don" or "isn"), lowercased, dropped
single letters other than "a"/"i", counted frequency across all ten books
combined, then manually removed ~60 proper nouns (character names,
ship/place names -- e.g. "darcy", "ahab", "holmes", "nantucket") and title
abbreviations ("mr", "mrs", "dr", "st") that would otherwise have ranked
in the top 1000 purely because they're characters in a small, 10-book
corpus rather than genuinely common English words. This produced a
mechanically-ranked top-1000 list.

**Step 2 -- hand curation (original, editorial).** The step-1 list read
like Victorian fiction rather than a modern typing-test corpus, for the
obvious reason that it *was* Victorian fiction: it carried archaic
pronouns and verb forms that have fallen out of use ("thou", "thee",
"thy", "whilst", "ere", "ye", "upon", "shall", "whom", plus the
dialect/register words "aye" and "morrow"), vocabulary that ranked high
purely because a handful of the ten source novels are saturated with it
rather than because the words are common in English generally (Moby-Dick
alone contributed "whale", "whales", "whaling", "sperm", "mast", and
"voyage"; other single-book artifacts included "cabin", "monster",
"flask", "lord", "lady", "sir", "madam", "carriage", "monsieur", "madame",
and "coffin"; "illustration" turned out to be a Gutenberg plate-caption
artifact rather than organic prose), a stray proper noun ("england") that
a mechanical proper-noun pass missed, and two words ("conversation",
"circumstances") long enough to work against a typing test that should be
comfortable to type. We removed those 32 words by hand, judging each on
whether a present-day typist would plausibly type it, not on whether it's
a real word -- genuinely timeless entries the source novels also happen to
use heavily (e.g. "ship", "horse", "law", "city") were kept. We then
hand-picked 32 replacement words -- ordinary modern vocabulary the
19th-century source corpus could never have produced because the things
didn't exist yet or weren't discussed in those terms: technology
("computer", "phone", "email", "internet", "app", "website", "software",
"laptop", "network", "online", "data", "technology"), work/institutions
("job", "office", "team", "market", "service", "system", "problem",
"group", "government", "president", "college", "student", "hospital"),
and everyday life ("month", "car", "music", "video", "game", "exercise",
"weather"). Each replacement was inserted next to an existing,
topically-related word already in the list (e.g. "computer" beside
"technology", "job" beside "work") rather than appended in a block at the
end, so the list still reads as roughly frequency-ordered rather than
"old words, then new words." The result is not, and does not claim to be,
a scientifically-measured modern frequency list -- it's our own editorial
judgment of what a common-word typing test should contain, seeded from
the step-1 analysis rather than pulled from any third-party source.

| Title | Author | Gutenberg ID | Source URL | License | Retrieved |
| --- | --- | --- | --- | --- | --- |
| Pride and Prejudice (1813) | Jane Austen | 1342 | https://www.gutenberg.org/ebooks/1342 | Public Domain (US) | 2026-08-07 |
| Alice's Adventures in Wonderland (1865) | Lewis Carroll | 11 | https://www.gutenberg.org/ebooks/11 | Public Domain (US) | 2026-08-07 |
| Frankenstein (1818) | Mary Shelley | 84 | https://www.gutenberg.org/ebooks/84 | Public Domain (US) | 2026-08-07 |
| Dracula (1897) | Bram Stoker | 345 | https://www.gutenberg.org/ebooks/345 | Public Domain (US) | 2026-08-07 |
| The Adventures of Sherlock Holmes (1892) | Arthur Conan Doyle | 1661 | https://www.gutenberg.org/ebooks/1661 | Public Domain (US) | 2026-08-07 |
| A Tale of Two Cities (1859) | Charles Dickens | 98 | https://www.gutenberg.org/ebooks/98 | Public Domain (US) | 2026-08-07 |
| Moby-Dick (1851) | Herman Melville | 2701 | https://www.gutenberg.org/ebooks/2701 | Public Domain (US) | 2026-08-07 |
| The Picture of Dorian Gray (1890) | Oscar Wilde | 174 | https://www.gutenberg.org/ebooks/174 | Public Domain (US) | 2026-08-07 |
| The Adventures of Tom Sawyer (1876) | Mark Twain | 74 | https://www.gutenberg.org/ebooks/74 | Public Domain (US) | 2026-08-07 |
| Anne of Green Gables (1908) | L. M. Montgomery | 45 | https://www.gutenberg.org/ebooks/45 | Public Domain (US) | 2026-08-07 |

**Transformation applied to all ten:** Gutenberg header/footer stripped;
tokenized to `[a-z]+` only (apostrophes, digits, and mixed-case artifacts
dropped); frequency-counted across the combined corpus; top ~1050 by
frequency manually reviewed and ~60 proper nouns / abbreviations removed;
truncated to the top 1000 remaining words -- that was the step-1 output.
Step 2 (hand curation, described above) then removed 32 archaic/period/
corpus-artifact words from that step-1 list and replaced them with 32
hand-picked modern words, each inserted next to a topically-related
neighbor rather than appended at the end. The shipped file is one word
per line, no blank lines, no trailing whitespace, deduplicated, and still
ordered by our best estimate of descending everyday frequency (not
alphabetical), but it is a hand-edited descendant of the step-1 output
rather than that output itself. Neither the step-1 frequency-counting
script nor a step-2 diff script is part of this repo (both were one-off
scratch tools), but both transformations are fully described above and
are reproducible from the ten URLs plus the word-level decisions listed.

## Quotes: `quotes/english.toml`

Same ten Project Gutenberg texts as above (see the table for per-title
URL/license/retrieval date); a subset of ten to five passages was hand-picked
per book, spread across the short (<100 chars) / medium (100-300) / long
(>300) buckets defined by `mode::QuoteLength`. 40 quotes total.

**Transformation applied:** exact verbatim excerpts (no paraphrasing) from
the stripped body text (same header/footer stripping as above); curly
quotes (`'` `'` `"` `"`) normalized to straight ASCII (`'` `"`); em/en
dashes (`—` `–`) normalized to `--`/`-`; Gutenberg's plain-text italics
markup (`_word_`) unwrapped to plain `word`; hard line-wraps collapsed to
single spaces. Every quote was checked to be pure ASCII and to fall in the
length bucket its `length` field declares (`words::quotes` has a test for
both). Quotes that were deliberately truncated mid-utterance were left
without enclosing quotation marks rather than with an unmatched one.

We deliberately avoided Wikiquote (CC BY-SA is share-alike and would force
that license onto this file) and did not use any post-1928 text, so nothing
here depends on the Google Books/HathiTrust-era US public-domain cutoff
being litigated differently than expected.

## Themes: `themes/*.toml`

| File | Palette | Source | License | Retrieved |
| --- | --- | --- | --- | --- |
| `amber-dark.toml` | Original to this project | Matches the reference results-screen screenshot commissioned for this project (`#e2b714` on `#323437`) | N/A -- hex triples are facts, not copyrightable expression; not derived from MonkeyType's theme files | 2026-08-07 |
| `nord.toml` | Nord | https://www.nordtheme.com/docs/colors-and-palettes / https://github.com/nordtheme/nord | MIT | 2026-08-07 |
| `dracula.toml` | Dracula | https://draculatheme.com/contribute (spec) / https://github.com/dracula/dracula-theme | MIT | 2026-08-07 |
| `gruvbox-dark.toml` | Gruvbox (dark, hard contrast) | https://github.com/morhetz/gruvbox (`gruvbox_256palette.sh`; README states "License: MIT/X11") | MIT | 2026-08-07 |
| `catppuccin-mocha.toml` | Catppuccin Mocha | https://github.com/catppuccin/palette (`palette.json`) | MIT | 2026-08-07 |
| `mono.toml` | Original to this project | A hand-picked neutral greyscale ramp; not derived from any third-party theme | N/A | 2026-08-07 |

Field mapping (`Theme` in `crates/clackr-core/src/theme.rs`): `main` and
`error` were chosen from each palette's own accent/red; `sub` from its
comment/subtext color; `sub_alt` from its lightly-raised background/surface
color; `text` from its foreground. None of these four upstream palettes
publish a *second* error tone for "extra" (overtyped) characters the way
`amber-dark`'s `#7e2a33` complements `#ca4754`, with one exception:
Catppuccin Mocha's official palette includes "Maroon" (`#eba0ac`), which we
used directly for `error_extra`. For Nord, Dracula, and Gruvbox we
manually darkened the palette's own red toward its own background (a
similar dark/muted relationship to `amber-dark`'s reference pair, not a
mechanical formula) rather than inventing an unrelated color; those three
`error_extra` values are therefore not upstream-sourced and are called out
here as our own derivation.

We deliberately did not reuse MonkeyType's theme *names* -- these are
independently-chosen names for independently-sourced palettes.
