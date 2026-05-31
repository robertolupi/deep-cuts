-- Record which algorithm produced the stored projection coordinates.
-- Values: 'pca' | 'umap'
ALTER TABLE track_coords ADD COLUMN algorithm TEXT NOT NULL DEFAULT 'pca';
