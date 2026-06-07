"""Phase 0 Evaluation Contract: Reproducible SALAMI Evaluation Script.

Implements the contract and statistical safeguards from roadmap.md:
- Dual-mode execution (Legacy full-track vs Corrected windowed eval)
- Track-dependent crop offset alignment
- Window filtering to central crop
- mir_eval P/R/F1 boundary metrics and pairwise clustering
- Bootstrap 95% confidence intervals over tracks
- Wilcoxon signed-rank paired significance testing
- Golden-number regression test asserts
"""

from pathlib import Path
from typing import Literal, TypedDict
import numpy as np

class BoundaryScores(TypedDict):
    precision: float
    recall: float
    f1: float


def calculate_crop_offset(duration: float, window: float = 90.0) -> float:
    """Return max(0, duration/2 - window/2). Implements the centre-crop logic from dsp.rs.
    
    Add a unit test that a synthetic boundary at t=10s in the crop round-trips 
    to 70s absolute on a 300s track.
    """
    raise NotImplementedError()


def to_absolute_time(times_crop: list[float], offset: float) -> list[float]:
    """Add offset to every timestamp from sidecar (crop-relative → track-absolute)."""
    raise NotImplementedError()


def filter_to_window(times_abs: list[float], start: float, end: float) -> list[float]:
    """Keep only boundaries inside [start, end]. Used for Option A windowed mode."""
    raise NotImplementedError()


def load_track(track_id: str, db_path: Path) -> dict:
    """Load duration, JAMS GT, and model predictions.
    
    Returns {'duration': float, 'gt_abs': [...], 'pred_crop': [...]}
    """
    raise NotImplementedError()


def score_mireval(
    pred_abs: list[float], 
    gt_abs: list[float], 
    tolerances: tuple[float, float] = (0.5, 3.0)
) -> dict[str, BoundaryScores]:
    """Run mir_eval.segment.detection for ±0.5s and ±3.0s. 
    
    Returns {'0.5': {...}, '3.0': {...}}
    """
    raise NotImplementedError()


def bootstrap_ci(
    scores: np.ndarray, 
    n_resamples: int = 2000, 
    alpha: float = 0.05
) -> tuple[float, float, float]:
    """Return (mean, lower, upper) 95% CI via resampling tracks with replacement."""
    raise NotImplementedError()


def paired_wilcoxon(a: np.ndarray, b: np.ndarray) -> dict:
    """Wilcoxon signed-rank on per-track paired differences. 
    
    Returns {'stat': ..., 'p': ..., 'mean_diff': ...}
    """
    raise NotImplementedError()


def evaluate_split(
    track_ids: list[str],
    db_path: Path,
    mode: Literal['legacy', 'windowed'] = 'windowed',
    window: float = 90.0
) -> dict:
    """Dual-mode execution:
    - legacy: score full-track pred vs full-track GT (for reproducing archived 21.8%/33.3%)
    - windowed: apply offset, then filter both pred and GT to [offset, offset+window]
    Returns per-track scores and aggregates.
    """
    raise NotImplementedError()


def run_phase0(
    split_json: Path,
    db_path: Path,
    mode: Literal['legacy', 'windowed'] = 'windowed',
    n_bootstrap: int = 2000
) -> dict:
    """Phase 0 entry point. Implements roadmap Phase 0 contract:
    - fixed splits, no leakage
    - P/R/F1 triple at both tolerances
    - bootstrap CIs
    - Wilcoxon vs baseline and vs oracle
    - golden-number regression check
    """
    raise NotImplementedError()
