-- Cache picked spectral-flux onset peaks from the audio_analysis pass.
-- Compact JSON: {"times":[s,...],"strengths":[0..1,...]}. Times are seconds
-- from the start of the analysis window (centre crop). NOT beats — tempo is
-- from autocorrelation and carries no phase.
ALTER TABLE tracks ADD COLUMN onsets TEXT;
