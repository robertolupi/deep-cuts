-- bpm_raw stores the original detector output from audio_analysis, never updated.
-- Used for debugging and allows BPM correction passes to recompute from the raw value.
ALTER TABLE tracks ADD COLUMN bpm_raw REAL;
