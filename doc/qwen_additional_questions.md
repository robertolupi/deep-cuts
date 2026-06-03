# Qwen Additional Questions — Design & Prompt Drafts

Each candidate field is rated for **likelihood of success** (1–5) based on how directly
audible the signal is for a 7B audio-language model, and whether the answer space is
constrained enough to avoid hallucination.

Test with: `tools/feedback.sh <audio-file> "<prompt>"`

## Context from the Qwen2-Audio technical report

The model scores **6.79/10** on the AIR-Bench Chat-Benchmark-Music (MusicCaps subset),
outperforming Gemini 1.5 Pro (5.06). MusicCaps captions describe genre, mood,
instrumentation, vocals, tempo feel, production style, and cultural origin — so the
model was trained to produce and evaluate exactly these kinds of descriptions.

Key training signals that inform our choices:
- **Multilingual ASR** across many languages → language detection is very reliable
- **Speech Emotion Recognition (SER)** → likely transfers to emotional quality of singing
- **Vocal Sound Classification** → distinguishes vocal types/styles
- **MusicCaps-style descriptions** → cultural/geographic origin, rhythm feel, vocal style
  are all part of the training distribution

---

## 1. Lyrics Language

**Likelihood: 5/5**

Directly audible. Categorical answer. No inference required — the model either hears
words or it doesn't, and spoken/sung language is one of the strongest audio signals.

Proposed DB column: `ai_language TEXT`

```
What language are the lyrics or vocals sung in?
If there are no vocals, answer "instrumental".
Respond in this format:
LANGUAGE: language name or "instrumental"
```

---

## 2. Energy Level

**Likelihood: 4/5**

Tempo, dynamics, and density are all audible. The three-point scale keeps the answer
space small. Risk: "medium" becomes a catch-all for ambiguous tracks.

Proposed DB column: `ai_energy TEXT`  — values: `low` / `medium` / `high`

```
How would you rate the energy level of this track?
Consider tempo, loudness, and overall intensity.
Respond in this format:
ENERGY: low, medium, or high
```

---

## 3. Listening Context

**Likelihood: 3/5**

Useful for playlist building. The model has to infer use-case from sonic texture, which
is a reasonable leap for a 7B model but introduces more subjectivity than the fields
above. Results will be plausible but not authoritative.

Proposed DB column: `ai_context TEXT`

```
What is the best listening context for this track?
Choose one or two from: focus, workout, party, background, sleep, commute, dinner.
Respond in this format:
CONTEXT: context1, context2
```

---

## 4. Decade / Era

**Likelihood: 3/5**

Production style, synthesis type, and recording texture are audible era cues. Works
well for recordings that have a clear sonic signature (80s synth pop, 60s soul, etc.).
Unreliable for AI-generated music that intentionally mimics a retro aesthetic, or for
tracks with anachronistic production.

Proposed DB column: `ai_era TEXT`

```
What decade or era does this track most sound like?
Examples: 1960s, 1970s, 1980s, 1990s, 2000s, 2010s, contemporary.
Respond in this format:
ERA: decade
```

---

## 5. Tempo Feel

**Likelihood: 4/5**

Distinct from numeric BPM: a 90 BPM hip-hop track feels slow, a 90 BPM punk track
feels fast. This captures the perceived pace rather than the measured one. Complements
the existing `bpm` column well.

Proposed DB column: `ai_tempo_feel TEXT` — values: `slow` / `mid-tempo` / `fast`

```
How does the tempo of this track feel to a listener?
Ignore the exact BPM — consider groove and pace perception.
Respond in this format:
TEMPO FEEL: slow, mid-tempo, or fast
```

---

## 6. Danceability

**Likelihood: 4/5**

Binary (or three-point) answer with clear sonic correlates: beat regularity, groove,
rhythmic drive. The model should handle this well. Overlaps somewhat with energy level
and genre, but "danceable sad ballad" and "non-danceable high-energy noise" are real
edge cases worth capturing.

Proposed DB column: `ai_danceability TEXT` — values: `not danceable` / `somewhat danceable` / `danceable`

```
Is this track danceable?
Consider the beat, groove, and rhythmic drive.
Respond in this format:
DANCEABILITY: not danceable, somewhat danceable, or danceable
```

---

## 7. Acoustic vs Electronic

**Likelihood: 5/5**

One of the clearest sonic distinctions. Directly audible. Useful as a filter and
consistent with essentia's existing `mood_acoustic` / `mood_electronic` scores — could
serve as a human-readable label for those.

Proposed DB column: `ai_production TEXT` — values: `acoustic` / `mixed` / `electronic`

```
Is this track primarily acoustic, electronic, or a mix of both?
Respond in this format:
PRODUCTION: acoustic, electronic, or mixed
```

---

## 8. Live vs Studio

**Likelihood: 3/5**

Audible cues exist (crowd noise, room reverb, performance imperfections) but a polished
live recording can fool the model, as can a studio recording with artificial crowd noise.
Worth testing but expect ~20% error rate.

Proposed DB column: `ai_recording TEXT` — values: `studio` / `live` / `unknown`

```
Does this track sound like a studio recording or a live performance?
Listen for crowd noise, room acoustics, or performance imperfections.
Respond in this format:
RECORDING: studio, live, or unknown
```

---

## 9. Lyrical Themes / Subject

**Likelihood: 2/5**

High value for discovery ("show me tracks about loss / nature / cities") but requires
understanding lyrics, which depends on the language and the model's multilingual
comprehension. Will be unreliable for non-English tracks and for instrumentals. Worth
exploring but treat output as approximate.

Proposed DB column: `ai_themes TEXT`

```
In a few words, what are the main themes or subjects of the lyrics?
If this is an instrumental, answer "instrumental".
Respond in this format:
THEMES: theme1, theme2
```

---

## 10. Complexity / Arrangement Density

**Likelihood: 3/5**

Captures how many layers are happening at once: a solo piano piece vs a full orchestral
arrangement vs a minimal techno loop. Useful for "something simple" vs "something rich"
moods. The model should be able to hear this, but the three-point scale may not be
granular enough to be interesting.

Proposed DB column: `ai_complexity TEXT` — values: `minimal` / `moderate` / `complex`

```
How would you describe the arrangement complexity of this track?
Consider the number of instruments, layers, and production density.
Respond in this format:
COMPLEXITY: minimal, moderate, or complex
```

---

## 11. Vocal Style

**Likelihood: 4/5**

Essentia detects vocal *presence* well, but says nothing about style. Qwen2-Audio's
Vocal Sound Classification training makes this a good fit. Useful for discovery
("find me tracks with raspy male vocals" or "operatic soprano").

Proposed DB column: `ai_vocal_style TEXT`

```
How would you describe the vocal style in this track?
Examples: smooth, raspy, falsetto, operatic, choir, spoken word, rap, auto-tuned.
If there are no vocals, answer "instrumental".
Respond in this format:
VOCAL STYLE: description or "instrumental"
```

---

## 12. Geographic / Cultural Origin

**Likelihood: 3/5**

MusicCaps captions frequently include cultural markers (Brazilian bossa nova, Irish
folk, West African highlife). This is part of the model's training distribution.
Works well for music with strong idiomatic markers; weaker for generic Western pop.

Proposed DB column: `ai_origin TEXT`

```
What is the geographic or cultural origin of this music?
Examples: Brazilian, Irish, West African, Jamaican, Indian classical, Japanese.
If the origin is unclear or generic Western pop/rock, answer "Western" or "unclear".
Respond in this format:
ORIGIN: origin
```

---

## 13. Rhythm Feel

**Likelihood: 4/5**

Distinct from tempo and danceability. Captures the groove character: syncopated,
straight, shuffle, swing, Latin, polyrhythmic. MusicCaps descriptions include this.
Useful alongside numeric BPM and tempo feel for playlist matching.

Proposed DB column: `ai_rhythm TEXT`

```
How would you describe the rhythmic feel or groove of this track?
Examples: straight, syncopated, shuffle, swing, Latin, polyrhythmic, rubato.
Respond in this format:
RHYTHM: description
```

---

## 14. Emotional Tone of Vocals

**Likelihood: 4/5**

Qwen2-Audio was trained on Speech Emotion Recognition (SER), which likely transfers
to sung emotion. This is richer than the overall mood field — a track can have a
"melancholic" mood but "tender" vocals, or an "aggressive" mood with "desperate"
vocals. Only meaningful when vocals are present.

Proposed DB column: `ai_vocal_emotion TEXT`

```
What is the emotional tone or feeling conveyed by the vocals in this track?
Examples: tender, joyful, melancholic, desperate, angry, serene, playful.
If there are no vocals, answer "instrumental".
Respond in this format:
VOCAL EMOTION: emotion or "instrumental"
```

---

## Priority order for implementation

| #  | Field                  | Likelihood | Value  | Notes                                             |
|----|------------------------|------------|--------|---------------------------------------------------|
|  1 | Lyrics language        | 5/5        | High   | Strong multilingual ASR training; no essentia overlap |
|  2 | Acoustic vs electronic | 5/5        | High   | Complements essentia mood_acoustic score          |
|  3 | Energy level           | 4/5        | High   | Complements mood, distinct from BPM               |
|  4 | Vocal style            | 4/5        | High   | No essentia equivalent; in training distribution  |
|  5 | Rhythm feel            | 4/5        | High   | In MusicCaps distribution; complements BPM        |
|  6 | Emotional tone (vocals)| 4/5        | High   | SER training transfers to sung emotion            |
|  7 | Tempo feel             | 4/5        | Medium | Complements numeric BPM                           |
|  8 | Danceability           | 4/5        | Medium | Useful filter; also planned as essentia model     |
|  9 | Listening context      | 3/5        | High   | High value if quality holds up                    |
| 10 | Geographic origin      | 3/5        | Medium | Works for idiomatic music; weak on generic pop    |
| 11 | Decade / era           | 3/5        | Medium | Unreliable for AI-generated retro music           |
| 12 | Complexity             | 3/5        | Medium | May need better scale                             |
| 13 | Live vs studio         | 3/5        | Low    | Modest value, moderate error rate                 |
| 14 | Lyrical themes         | 2/5        | High   | High value but unreliable cross-language          |
