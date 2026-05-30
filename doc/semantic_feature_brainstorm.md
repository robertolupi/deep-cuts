# Deep Cuts — Advanced Semantic & Producer Feature Brainstorm

This document outlines high-level concepts for advanced user features leveraging our local multi-modal analysis dataset (acoustic CLAP embeddings, semantic Qwen descriptions, MiniLM text embeddings, and structured metadata). 

These features can be built incrementally on top of our existing database schema without disrupting the background scanner or import pipeline.

---

## 1. Advanced Semantic & Discovery Features

### 🔍 Natural Language Semantic Search (Local NLP)
Since prose descriptions are mapped to a 384-dimensional vector space using MiniLM and indexed in the SQLite `description_embeddings` (`vec0`) virtual table, we can support **instant, 100% offline semantic search**.
* **How it works**: The user types descriptive queries into the search bar:
  * *"A gritty, industrial electronic track with heavy distorted synthesizers"*
  * *"Warm folk acoustic guitar with emotional strings"*
  * *"Laid-back, cool jazz lounge piano and saxophone"*
* **Under the Hood**: The app converts the search query into a 384-d vector on the fly using our existing sentence-embedding session (`all-MiniLM-L6-v2.onnx`), performs a cosine-similarity query against the `description_embeddings` virtual table, and ranks the entire library by conceptual relevance.

### 🎚️ Dual-Engine Similarity Sidebar ("Sounds Like" vs. "Feels Like")
When a track is playing, the app can offer a sidebar of recommended next tracks using a blended similarity engine:
* **"Sounds Like" (Acoustic Engine)**: Queries nearest neighbors using the **CLAP** (512-d) space to match tracks with similar tempo, timbre, spectral balance, and vocal presence.
* **"Feels Like" (Semantic Engine)**: Queries nearest neighbors using the **MiniLM** (384-d) space to match tracks with similar instrumentation narratives, prose descriptions, and emotional moods.
* **The Vibe Slider**: A visual slider lets the user blend these weights (e.g., `80% Acoustic / 20% Semantic` to find tracks that sound identical but cover different genres, or `10% Acoustic / 90% Semantic` to find tracks that tell similar stories with completely different instruments).

### 🗺️ "Vibe Continents" Map Layering
Transform the current UMAP 2D projection scatter plot into an interactive acoustic dashboard:
* **Genre & Mood Heatmaps**: Group and color the UMAP dots dynamically to render visual boundaries or "continents of sound" (e.g., the *Ambient Archipelago*, the *Intense Synth Desert*, or the *Heartfelt Acoustic Coast*).
* **Instrument Highlighter**: Let producers toggle filters (e.g., click a "Saxophone" button) to highlight only the dots representing tracks containing `saxophone` in their `ai_instruments` field, allowing visual navigation of instrument densities in their collection.

### 🔀 Pathfinding Playlists (Acoustic DJ Transitions)
Using coordinates on the 2D UMAP projection, implement a playlist generator that maps smooth sonic journeys:
* **The Concept**: The user selects a **Start Track** (e.g., a quiet acoustic ballad) and an **End Track** (e.g., an intense electronic techno track).
* **The Execution**: The app calculates a geometric path through the nearest neighboring UMAP points to build a playlist that **gradually morphs in vibe** from the start to the end, ensuring natural transitions in BPM, key, acoustic texture, and mood.

---

## 2. Music Producer & Audio Engineer Features

### 🎛️ Reference Track Mix Matcher
Producers frequently use commercial "reference tracks" to check if their own mixes have appropriate frequency balance, loudness (LUFS), and width.
* **Reference Drag-and-Drop**: A producer drags in a commercial WAV/MP3 track as a temporary reference.
* **Instant Feature Extraction**: The app instantly calculates its average spectral profile (1/3-octave frequency distribution), average RMS/loudness, tempo, and Key/Scale.
* **Library Matcher**:
  * **Mix Profile Matching**: Suggests tracks in their own catalog that share the closest spectral distribution and dynamic range, helping them identify if their low-end (bass) or high-end (air) is over-emphasized.
  * **Acoustic TIMBRE Matching**: Finds tracks in their library with the closest CLAP distance, allowing them to instantly locate songs that share the same production aesthetic (e.g., finding other tracks with "dry 70s drum mixes").

### 📊 Acoustic Spectral Profile Overlays
* Compute a simplified 24-band frequency spectrum curve for every track during the analysis stage and store it in the database.
* Provide a mini-visualizer in the audio player that lets the producer **visually overlay** the frequency spectrum of a work-in-progress track against a reference track. This makes mixing deficiencies (e.g. "muddy low-mids at 250Hz") immediately apparent.

---

## 3. Hip Hop & Electronic Sampling Features

### 🥁 Breakbeat & Drum Groove Similarity
Sampling producers often look for specific drum textures (e.g., a "dusty, warm funk break" vs. a "crisp, modern acoustic snare").
* **Rhythmic Similarity**: If a producer finds a drum break they like (such as a classic 70s soul break), they can click *"Find similar drum grooves"*.
* **timbral Clustering**: Because CLAP embeddings capture acoustic signatures, tracks containing drums with similar mic setups, room acoustics, tape saturation, and syncopation will naturally group together, turning their library into an automated drum-digging crate.

### 🕵️ Obscurity Indexing ("Crate Digger Mode")
Hip-hop and electronic producers thrive on finding obscure, rare, or isolated sounds that don't sound like anything else.
* **UMAP Isolation Score**: We can calculate an "Obscurity Score" for each track by measuring its geometric distance to its nearest neighbors in UMAP space. Dots far out in the periphery represent acoustically unique or highly isolated recordings in their library.
* **Digger Sort**: Let users search or browse their library sorted by *"Most Obscure / Acoustically Isolated"*, immediately surfacing weird interludes, rare field recordings, or unique acoustic arrangements perfect for sampling.

### 🗣️ Vocal & Instrumental Scraper
* **Accurate Instrumental Extraction**: Diggers can search for clean instrumental hooks by filtering for tracks where Qwen classified `vocal = "instrumental"` (or `vocal_confidence` is very low) and `ai_instruments` contains a target instrument (e.g., `"piano"`, `"strings"`, or `"vibraphone"`).
* This allows them to instantly scrape thousands of hours of music to find clean, vocal-free sections containing solo instruments ready for chopped samples.

### 📐 Pitch & Tempo Harmonizer (Sampler Transpose Calculator)
When a producer finds a track they want to sample:
* The app cross-references the sample track's detected BPM and Key against their current active DAW project (or a selected "target" reference track).
* It dynamically calculates the precise transposition and time-stretch parameters:
  > *"To fit your project (**95 BPM / G-Minor**), pitch this sample (**C-Minor / 112 BPM**) by **+5 semitones** and stretch it by **117.9%**."*

---

## 4. DJ & Live Performance Features

### 🌡️ Energy Contour & Crowd Moods
Standard tempo (BPM) and Key are great for technical mixing, but they don't capture the energy progression or overall "vibe" of a track (e.g. warm-up vs. peak-time).
* **Vibe Classifications**: We can map Qwen's emotional and timbral descriptors to dynamic **Energy Levels** (e.g., *1: Ambient/Warm-up, 2: Warm-up Groove, 3: Steady Build, 4: Peak-Time Energy, 5: High-Intensity Banger*).
* **Crowd Mood Filtering**: Let DJs search and filter their set crates by active floor responses, such as *"Euphoric & Hypnotic"* or *"Gritty & Aggressive"*, ensuring they maintain or shift the room's energy systematically.

### 🎼 Harmonic UMAP Map Matcher (Smarter Harmonic Mixing)
Harmonic mixing (using the Camelot Wheel) dictates mixing in relative keys (e.g., E-Minor to A-Minor or B-Minor).
* **Timbral Compatibility Overlay**: When a DJ selects an active track, the app instantly overlays harmonic filters on the 2D UMAP Map, highlighting only the dots in compatible keys.
* Because the map organizes tracks by acoustic similarity, compatible keys that are **closest geometrically** are guaranteed to blend both harmonically and texture-wise, preventing jarring timbral clashes during long blends.

### 🌉 Vibe Drift Warning & Transit Bridges
When a DJ is manually compiling a set list:
* **Drift Detection**: The app monitors the path between successive tracks in UMAP space.
* **Alert Trigger**: If two adjacent tracks represent a massive jump in acoustic or semantic distance (e.g. transitioning directly from organic deep house to aggressive industrial techno), the app flags a *"Vibe Drift Warning"*.
* **Transit Suggestions**: The app automatically suggests 2 or 3 "bridge tracks" from the DJ's library that lie geometrically between the two endpoints, facilitating a smooth, logical transition across genres.

### 💥 "Double Drop" Compatibility Meter
In high-energy genres (Drum & Bass, Techno, Dubstep), DJs love "double dropping"—playing the explosive drop sections of two tracks simultaneously.
* **Spectral Analysis Comparison**: The app compares the stored 24-band spectral profiles and CLAP embeddings of the two tracks.
* **Clash Score**: It calculates a dynamic compatibility score based on frequency overlapping (e.g., checking if the low-end basslines or high-frequency hats will fight each other, causing mud). It guides the DJ on EQ adjustments or highlights pairs that double-drop seamlessly.

---

## 5. Corpus Analytics & Statistical Visualizations

### 📊 Comparative "Timbral Footprints" (Radar/Line Chart Overlay)
For any two subsets of tracks (e.g., a specific folder, a genre filter, or custom tags like *My Rock Tracks* vs. *My Reference Rock Tracks*), we can compute and plot their average acoustic signature:
* **Timbral Comparison**: Compare frequency energy distributions (lows, mids, highs), dynamic ranges, and average CLAP centroids.
* **Visualizing the Gap**: Plotting these signatures as an overlaid radar chart immediately reveals structural mixing differences:
  > *"Your rock songs have an average of +4dB more energy in the mud region (250Hz) and -3dB less high-end presence (above 8kHz) compared to your commercial Rock Reference collection."*

### 🗺️ UMAP Density Contours (Sonic Topography)
Visualizing a flat swarm of dots can become overwhelming in large libraries. Overlaying density contour maps (similar to topographic elevation maps) highlights the focal points of subsets:
* **Footprint Overlap**: Render a green density contour for *Folder A (Acoustic/Folk)* and a purple density contour for *Folder B (Electronic/House)*.
* **Analysis**: DJs and producers can immediately see where their folders overlap (hybrid tracks) and where they are completely separated, helping them understand the visual and acoustic partitioning of their directory organization.

### 🔠 Comparative Semantic Tag Clouds
Compare the Qwen narrative vocabularies of two subsets (e.g., *Folder X* vs. *Folder Y*):
* **Contrast Clouds**: Instead of standard clouds, show a "contrast tag cloud" where word sizes reflect relative frequency differences rather than absolute counts.
* **Analysis**: Clicking *My Rock Tracks* vs. *Reference Rock Tracks* might show that your tracks are dominated by words like `"guitar"`, `"raw"`, and `"gritty"`, while your references feature `"polished"`, `"wide"`, and `"epic"`, indicating stylistic deviations.

### 🎼 Key & BPM Distribution Histograms
Provide comparative statistical dashboards of library coverage:
* **Camelot Wheel Overlays**: Render side-by-side or overlaid Camelot Key Wheels with heatmaps indicating track density.
* **BPM Histograms**: Chart BPM distribution in 5-BPM buckets.
* **Gap Analysis**: Surfacing gaps in the library (e.g., *"You have a high density of G-Minor tracks at 120 BPM, but a complete transition gap in G-Minor at 128 BPM"*), helping DJs systematically target what types of tracks they need to purchase or export next.

### ⏳ Sonic Style Timeline (Evolution Tracker)
If tracks have local file creation dates or year metadata:
* **UMAP Migration Path**: Plot the average UMAP centroids of a producer's tracks grouped by year.
* **Visual Trend**: Draw a path connecting 2024 -> 2025 -> 2026. This renders a fascinating, local "style migration path" showing how your personal music production aesthetic has evolved over time (e.g., moving from organic/acoustic clusters to electronic/heavy techno zones).

---

## 6. Advanced Ad-Hoc Extraction & Interactive Audio LLM QA

### 🧬 Continuous "Sonic DNA" Tracking (Sliding Window CLAP)
The standard analysis pass only analyzes 3 cropped midpoint blocks of a track to save database space. For advanced ad-hoc comparisons, we can extract CLAP embeddings across the **entire duration** of a song:
* **Timbral Trajectory**: Run a sliding 10-second window (e.g., with a 5-second step size) over the whole track. This yields a series of 512-d vectors representing the **acoustic evolution** of the song over time.
* **Sonic DNA Visualizer**: Plot this sequence as a continuous, colored waveform or line chart representing the song's timbral signature (e.g. showing exactly when a vocal enters, when the beat drops, or when the outro fades).
* **Dynamic Time Warping (DTW) Comparison**: When comparing two songs, the app doesn't just calculate a static similarity score; it runs DTW over their timbral trajectories. This lets the app highlight structurally similar sections:
  > *"Track A's bridge at 2:15 shares an 94% acoustic similarity with Track B's intro at 0:10."*

### 🎚️ Acoustic EQ Prefiltering (Groove & Beat Isolation)
DJs and producers often want to compare tracks by specific musical layers—such as matching the exact swing/rhythm of a drum beat, regardless of what vocals or synths are on top. We can introduce a **DSP filtering stage** before feeding audio to the CLAP encoder:
* **Bass/Kick Isolation (Low-Pass < 150Hz)**: Filters out all high-end synths and vocals, running CLAP *strictly* on the sub-bass and kick drums. This lets the producer match tracks based purely on **low-end weight and rhythmic groove**.
* **Percussion/Swing Isolation (High-Pass > 2kHz)**: Filters out low-end mud, running CLAP purely on transient hi-hats, shakers, and snares to match tracks by their high-frequency rhythm and swing.
* **Vocal Texture Isolation (Band-Pass 300Hz - 3kHz)**: Isolates the midrange to compare tracks strictly by the acoustic timbre and delivery of the vocalists.

### 💬 Multimodal Interactive QA ("Chat with Your Music")
Since Qwen2-Audio is a native multimodal conversation model, we can leverage the local `llama-server` to let users **have conversational chats with their songs or samples**:
* **Ad-Hoc Chatbot Sidebar**: A chat interface in the app where the user selects one or more tracks and types interactive questions:
  * *"Describe the vocal delivery of the singer in this track."*
  * *"Where is the emotional climax of this recording?"*
  * *"Which of these three sample loops has the cleanest, most punchy bassline?"*
  * *"Compare the mood transitions of these two tracks."*
* **On-Demand Slicing**: The backend dynamically crops the relevant audio segments, compiles them into a multimodal completions payload, and streams Qwen's natural language responses locally in the chat window. This gives producers and diggers an interactive, conversational assistant that literally "hears" their music.
