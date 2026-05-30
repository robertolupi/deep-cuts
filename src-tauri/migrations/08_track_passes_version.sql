-- Add pass_version to track_passes so the pipeline knows which algorithm/model
-- version produced each result. Existing DONE rows are backfilled to 1 (the
-- first version of every pass), so work already completed is not re-run.
-- Rows in any other status get 0, which is below every current constant.
ALTER TABLE track_passes ADD COLUMN pass_version INTEGER NOT NULL DEFAULT 0;
UPDATE track_passes SET pass_version = 1 WHERE status = 2;
