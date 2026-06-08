use rustfft::{num_complex::Complex, FftPlanner};
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;

// Krumhansl-Schmuckler key profiles (root = index 0)
const KS_MAJOR: [f64; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const KS_MINOR: [f64; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];
const KEY_NAMES: [&str; 12] = [
    "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];

pub struct AudioAnalysisResult {
    pub duration_seconds: u64,
    pub waveform_data: String,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub scale: Option<String>,
    pub key_strength: Option<f64>,
    pub loudness_lufs: f64,
    pub loudness_range: f64,
    pub silence_regions: String,
    pub has_long_silence: bool,
    /// Picked spectral-flux onset peaks (NOT beats — tempo comes from
    /// autocorrelation, which has no phase). Times are seconds from the start
    /// of the analysis window (the centre crop used for key/BPM), paired with
    /// the normalised flux strength at each peak. Empty when analysis fails.
    pub onsets: Vec<(f32, f32)>,
    /// Short-hop chroma time-series. One 12-d pitch-class vector per
    /// `chroma_time_step` seconds, each paired with its window start time
    /// (seconds from the start of the analysis window). Large — persisted to
    /// the `.dc.json` sidecar, never a DB column.
    pub chroma_series: Vec<(f32, [f32; 12])>,
    /// Time step (seconds) between consecutive `chroma_series` frames.
    pub chroma_time_step: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SilenceAnalysisResult {
    pub silence_regions: String,
    pub has_long_silence: bool,
}

/// Detect contiguous low-energy regions in mono audio.
///
/// A region is considered silence when 10 ms RMS blocks stay below -60 dBFS
/// for at least 2 seconds. `has_long_silence` is true for any detected region
/// longer than 10 seconds.
pub fn detect_silence_regions(
    samples: &[f32],
    sample_rate: u32,
) -> Result<SilenceAnalysisResult, String> {
    if sample_rate == 0 {
        return Err("Sample rate must be greater than zero".to_string());
    }
    if samples.is_empty() {
        return Ok(SilenceAnalysisResult {
            silence_regions: "[]".to_string(),
            has_long_silence: false,
        });
    }

    let block_size = ((sample_rate as f64 * 0.010).round() as usize).max(1);
    let threshold = 10f32.powf(-60.0 / 20.0);
    let min_region_seconds = 2.0;
    let long_region_seconds = 10.0;
    let mut regions: Vec<[f64; 2]> = Vec::new();
    let mut active_start: Option<usize> = None;

    for (block_index, block) in samples.chunks(block_size).enumerate() {
        let rms = (block.iter().map(|s| s * s).sum::<f32>() / block.len() as f32).sqrt();
        if rms < threshold {
            active_start.get_or_insert(block_index);
        } else if let Some(start_block) = active_start.take() {
            let start = start_block as f64 * block_size as f64 / sample_rate as f64;
            let end = block_index as f64 * block_size as f64 / sample_rate as f64;
            if end - start >= min_region_seconds {
                regions.push([start, end]);
            }
        }
    }

    if let Some(start_block) = active_start {
        let start = start_block as f64 * block_size as f64 / sample_rate as f64;
        let end = samples.len() as f64 / sample_rate as f64;
        if end - start >= min_region_seconds {
            regions.push([start, end]);
        }
    }

    let has_long_silence = regions
        .iter()
        .any(|[start, end]| end - start > long_region_seconds);
    let silence_regions = serde_json::to_string(&regions).map_err(|e| e.to_string())?;

    Ok(SilenceAnalysisResult {
        silence_regions,
        has_long_silence,
    })
}

/// Helper to extract audio samples from an AudioBufferRef as normalized f32.
/// Returns a tuple containing:
/// - A flat vector of stereo samples (if mono, duplicated to stereo)
/// - A flat vector of mono averaged samples
/// - The number of frames in the buffer
fn extract_samples_as_f32(decoded: &AudioBufferRef) -> Option<(Vec<f32>, Vec<f32>, usize)> {
    let mut stereo = Vec::new();
    let mut mono = Vec::new();
    let frames;

    match decoded {
        AudioBufferRef::F32(buf) => {
            frames = buf.frames();
            let channels = buf.spec().channels.count();
            let c0 = buf.chan(0);
            if channels == 1 {
                for &s in c0 {
                    mono.push(s);
                    stereo.push(s);
                    stereo.push(s);
                }
            } else {
                let c1 = buf.chan(1);
                for i in 0..frames {
                    mono.push((c0[i] + c1[i]) * 0.5);
                    stereo.push(c0[i]);
                    stereo.push(c1[i]);
                }
            }
        }
        AudioBufferRef::F64(buf) => {
            frames = buf.frames();
            let channels = buf.spec().channels.count();
            let c0 = buf.chan(0);
            if channels == 1 {
                for &s in c0 {
                    let v = s as f32;
                    mono.push(v);
                    stereo.push(v);
                    stereo.push(v);
                }
            } else {
                let c1 = buf.chan(1);
                for i in 0..frames {
                    let l = c0[i] as f32;
                    let r = c1[i] as f32;
                    mono.push((l + r) * 0.5);
                    stereo.push(l);
                    stereo.push(r);
                }
            }
        }
        AudioBufferRef::S16(buf) => {
            frames = buf.frames();
            let norm = i16::MAX as f32;
            let channels = buf.spec().channels.count();
            let c0 = buf.chan(0);
            if channels == 1 {
                for &s in c0 {
                    let v = s as f32 / norm;
                    mono.push(v);
                    stereo.push(v);
                    stereo.push(v);
                }
            } else {
                let c1 = buf.chan(1);
                for i in 0..frames {
                    let l = c0[i] as f32 / norm;
                    let r = c1[i] as f32 / norm;
                    mono.push((l + r) * 0.5);
                    stereo.push(l);
                    stereo.push(r);
                }
            }
        }
        AudioBufferRef::S32(buf) => {
            frames = buf.frames();
            let norm = i32::MAX as f32;
            let channels = buf.spec().channels.count();
            let c0 = buf.chan(0);
            if channels == 1 {
                for &s in c0 {
                    let v = s as f32 / norm;
                    mono.push(v);
                    stereo.push(v);
                    stereo.push(v);
                }
            } else {
                let c1 = buf.chan(1);
                for i in 0..frames {
                    let l = c0[i] as f32 / norm;
                    let r = c1[i] as f32 / norm;
                    mono.push((l + r) * 0.5);
                    stereo.push(l);
                    stereo.push(r);
                }
            }
        }
        AudioBufferRef::U8(buf) => {
            frames = buf.frames();
            let channels = buf.spec().channels.count();
            let c0 = buf.chan(0);
            if channels == 1 {
                for &s in c0 {
                    let v = (s as f32 - 128.0) / 128.0;
                    mono.push(v);
                    stereo.push(v);
                    stereo.push(v);
                }
            } else {
                let c1 = buf.chan(1);
                for i in 0..frames {
                    let l = (c0[i] as f32 - 128.0) / 128.0;
                    let r = (c1[i] as f32 - 128.0) / 128.0;
                    mono.push((l + r) * 0.5);
                    stereo.push(l);
                    stereo.push(r);
                }
            }
        }
        _ => return None,
    }

    Some((stereo, mono, frames))
}

/// Decodes an audio file to a mono f32 sample vector, returning (samples, sample_rate).
pub fn decode_audio_to_mono(path: &str) -> Result<(Vec<f32>, u32), String> {
    let file = File::open(Path::new(path)).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = Path::new(path).extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let mut probed = symphonia::default::get_probe()
        .format(&hint, mss, &Default::default(), &Default::default())
        .map_err(|e| e.to_string())?;

    let track = probed.format.default_track().ok_or("No default track")?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let sample_rate = codec_params
        .sample_rate
        .ok_or("No sample rate in codec params")?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &Default::default())
        .map_err(|e| e.to_string())?;

    let mut mono_samples: Vec<f32> = Vec::new();

    while let Ok(packet) = probed.format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if let Some((_, mono, _)) = extract_samples_as_f32(&decoded) {
            mono_samples.extend(mono);
        }
    }

    Ok((mono_samples, sample_rate))
}

/// @concept AudioAnalysis
/// @skill add-analysis-pass
/// Single-pass decode: computes duration, waveform, BPM, key, loudness from one file read.
pub fn run_audio_analysis(path: &str) -> Result<AudioAnalysisResult, String> {
    let file = File::open(Path::new(path)).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = Path::new(path).extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let mut probed = symphonia::default::get_probe()
        .format(&hint, mss, &Default::default(), &Default::default())
        .map_err(|e| e.to_string())?;

    let track = probed.format.default_track().ok_or("No default track")?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let sample_rate = codec_params
        .sample_rate
        .ok_or("No sample rate in codec params")?;

    // Derive duration from container metadata when available; count samples as fallback
    let container_duration: Option<u64> =
        codec_params
            .time_base
            .zip(codec_params.n_frames)
            .map(|(tb, n)| {
                let t = tb.calc_time(n);
                (t.seconds as f64 + t.frac).round() as u64
            });

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &Default::default())
        .map_err(|e| e.to_string())?;

    let mut meter = ebur128::EbuR128::new(2, sample_rate, ebur128::Mode::I | ebur128::Mode::LRA)
        .map_err(|e| e.to_string())?;

    let mut mono_samples: Vec<f32> = Vec::new();
    let mut rms_energies: Vec<f32> = Vec::new();

    while let Ok(packet) = probed.format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };

        if let Some((stereo, mono, frames)) = extract_samples_as_f32(&decoded) {
            let _ = meter.add_frames_f32(&stereo);

            let mut packet_sum_sq = 0.0f64;
            for &s in &mono {
                packet_sum_sq += (s * s) as f64;
            }

            mono_samples.extend(mono);
            if frames > 0 {
                rms_energies.push((packet_sum_sq / frames as f64).sqrt() as f32);
            }
        }
    }

    let duration_seconds = container_duration
        .unwrap_or_else(|| (mono_samples.len() as f64 / sample_rate as f64).round() as u64);

    let loudness_lufs = meter
        .loudness_global()
        .map(|v| (v * 100.0).round() / 100.0)
        .unwrap_or(f64::NEG_INFINITY);
    let loudness_range = meter
        .loudness_range()
        .map(|v| (v * 100.0).round() / 100.0)
        .unwrap_or(0.0);

    let waveform = downsample_profile(&rms_energies, 128);
    let waveform_data = serde_json::to_string(&waveform).map_err(|e| e.to_string())?;
    let silence = detect_silence_regions(&mono_samples, sample_rate)?;

    // Crop to centre 90 s window for key and BPM
    let cap = (90u64 * sample_rate as u64) as usize;
    let cropped: &[f32] = if mono_samples.len() > cap {
        let mid = mono_samples.len() / 2;
        let half = cap / 2;
        let start = mid.saturating_sub(half);
        let end = (start + cap).min(mono_samples.len());
        &mono_samples[start..end]
    } else {
        &mono_samples
    };

    if cropped.is_empty() {
        return Err("Audio too short for analysis".to_string());
    }

    let (key, scale, key_strength, bpm, onsets, chroma_series, chroma_time_step) =
        match analyze_key_and_bpm_joint(cropped, sample_rate) {
            Ok(j) => (
                Some(j.key),
                Some(j.scale),
                Some(j.key_strength),
                Some(j.bpm),
                j.onsets,
                j.chroma_series,
                j.chroma_time_step,
            ),
            Err(e) => {
                log::warn!("[run_audio_analysis] Joint key/BPM analysis failed: {}. Setting fields to NULL.", e);
                (None, None, None, None, Vec::new(), Vec::new(), 0.2)
            }
        };

    Ok(AudioAnalysisResult {
        duration_seconds,
        waveform_data,
        bpm,
        key,
        scale,
        key_strength,
        loudness_lufs,
        loudness_range,
        silence_regions: silence.silence_regions,
        has_long_silence: silence.has_long_silence,
        onsets,
        chroma_series,
        chroma_time_step,
    })
}

fn downsample_profile(raw: &[f32], target: usize) -> Vec<f32> {
    if raw.is_empty() {
        return vec![0.0; target];
    }
    let chunk = raw.len() as f64 / target as f64;
    (0..target)
        .map(|i| {
            let start = (i as f64 * chunk) as usize;
            let end = (((i + 1) as f64 * chunk) as usize).min(raw.len());
            if start >= end {
                *raw.get(start).unwrap_or(&0.0)
            } else {
                raw[start..end].iter().sum::<f32>() / (end - start) as f32
            }
        })
        .collect()
}

/// Result of joint key/BPM analysis plus the cached intermediate DSP features.
pub struct JointAnalysis {
    pub key: String,
    pub scale: String,
    pub key_strength: f64,
    pub bpm: f64,
    /// Peak-picked onsets: (time_seconds, normalised_strength).
    pub onsets: Vec<(f32, f32)>,
    /// Chroma time-series: (time_seconds, 12-d L1-normalised pitch-class vector).
    pub chroma_series: Vec<(f32, [f32; 12])>,
    /// Time step (seconds) between consecutive chroma frames.
    pub chroma_time_step: f32,
}

fn analyze_key_and_bpm_joint(
    samples: &[f32],
    sample_rate: u32,
) -> Result<JointAnalysis, String> {
    const FFT_SIZE: usize = 4096;
    const HOP_SIZE: usize = 1024;

    if samples.is_empty() {
        return Err("Audio too short for analysis".to_string());
    }

    let hann: Vec<f32> = (0..FFT_SIZE)
        .map(|n| {
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * n as f32 / (FFT_SIZE - 1) as f32).cos())
        })
        .collect();

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let mut fft_buf = vec![Complex::new(0.0, 0.0); FFT_SIZE];

    let block_len = 10 * sample_rate as usize;
    let frame_dt = HOP_SIZE as f32 / sample_rate as f32;

    // Per-frame timelines collected across all retained blocks. Times are
    // absolute seconds from the start of `samples` (the analysis window).
    // `(time, flux)` for the onset envelope and `(time, chroma12)` for the
    // chroma time-series. Only filled on the retained (non-skipped) pass.
    type FrameTimelines = (Vec<(f32, f32)>, Vec<(f32, [f32; 12])>);

    let mut run_analysis_loop = |apply_filter: bool| -> (usize, [f64; 12], Vec<f64>, FrameTimelines) {
        let mut active_count = 0;
        let mut global_chroma = [0.0f64; 12];
        let mut global_ac = Vec::new();
        let mut flux_timeline: Vec<(f32, f32)> = Vec::new();
        let mut chroma_timeline: Vec<(f32, [f32; 12])> = Vec::new();

        for (block_idx, block) in samples.chunks(block_len).enumerate() {
            if block.len() < FFT_SIZE {
                continue;
            }
            let block_time0 = (block_idx * block_len) as f32 / sample_rate as f32;

            // Cheap RMS check
            let sum_sq: f32 = block.iter().map(|&x| x * x).sum();
            let rms = (sum_sq / block.len() as f32).sqrt();
            let rms_min = if apply_filter { 0.005 } else { 0.001 };
            if rms < rms_min {
                continue;
            }

            let mut onset = Vec::new();
            let mut prev_mag = vec![0.0f32; FFT_SIZE / 2];
            let mut block_chroma = [0.0f64; 12];
            // Per-frame chroma (raw magnitudes, unnormalised) collected for the
            // chroma time-series. Committed to `chroma_timeline` only if the
            // block survives all downstream filters.
            let mut frame_chromas: Vec<[f32; 12]> = Vec::new();
            let mut spectral_flatness_sum = 0.0f64;
            let mut frame_count = 0;

            let mut frame_start = 0;
            while frame_start + FFT_SIZE <= block.len() {
                for (i, c) in fft_buf.iter_mut().enumerate() {
                    c.re = block[frame_start + i] * hann[i];
                    c.im = 0.0;
                }
                fft.process(&mut fft_buf);

                let mut flux = 0.0f32;
                let sr = sample_rate as f64;

                let mut log_sum = 0.0f64;
                let mut mag_sum = 0.0f64;
                let num_bins = FFT_SIZE / 2;
                let mut frame_chroma = [0.0f32; 12];

                for k in 1..num_bins {
                    let mag = (fft_buf[k].re as f64).hypot(fft_buf[k].im as f64);
                    let mag_f = mag as f32;

                    // Onset (Flux)
                    let diff = mag_f - prev_mag[k];
                    if diff > 0.0 {
                        flux += diff;
                    }
                    prev_mag[k] = mag_f;

                    // Flatness metrics
                    log_sum += (mag + 1e-7).ln();
                    mag_sum += mag;

                    // Key (Chroma)
                    let freq = k as f64 * sr / FFT_SIZE as f64;
                    if freq >= 65.0 && freq <= 4000.0 {
                        let semitone = 12.0 * (freq / 440.0).log2() + 69.0;
                        let pc = (semitone.round() as i64).rem_euclid(12) as usize;
                        block_chroma[pc] += mag;
                        frame_chroma[pc] += mag_f;
                    }
                }

                onset.push(flux);
                frame_chromas.push(frame_chroma);

                let arithmetic_mean = mag_sum / (num_bins - 1) as f64;
                let geometric_mean = (log_sum / (num_bins - 1) as f64).exp();
                let flatness = if arithmetic_mean > 0.0 {
                    geometric_mean / arithmetic_mean
                } else {
                    1.0
                };
                spectral_flatness_sum += flatness;
                frame_count += 1;

                frame_start += HOP_SIZE;
            }

            if frame_count < 10 {
                continue;
            }

            let mean_onset = onset.iter().sum::<f32>() / onset.len() as f32;
            let var_onset = onset.iter().map(|&x| (x - mean_onset).powi(2)).sum::<f32>() / onset.len() as f32;
            let avg_flatness = spectral_flatness_sum / frame_count as f64;

            if apply_filter {
                if var_onset < 0.01 || avg_flatness > 0.75 {
                    continue;
                }
            }

            // Commit per-frame onset envelope and chroma to the timelines now
            // that the block has survived all filters. Times are absolute
            // seconds from the start of the analysis window.
            for (frame_idx, &flux) in onset.iter().enumerate() {
                let t = block_time0 + frame_idx as f32 * frame_dt;
                flux_timeline.push((t, flux));
            }
            for (frame_idx, chroma) in frame_chromas.iter().enumerate() {
                let t = block_time0 + frame_idx as f32 * frame_dt;
                chroma_timeline.push((t, *chroma));
            }

            // Autocorrelation accumulation
            let fps = sample_rate as f64 / HOP_SIZE as f64;
            let lag_min = ((fps * 60.0 / 210.0).ceil() as usize).max(1);
            let lag_max = ((fps * 60.0 / 40.0).floor() as usize).min(onset.len() / 2);

            if global_ac.is_empty() {
                global_ac = vec![0.0; lag_max + 1];
            }

            let mean = onset.iter().sum::<f32>() / onset.len() as f32;
            for lag in lag_min..=lag_max {
                let ac: f64 = (0..onset.len() - lag)
                    .map(|i| (onset[i] - mean) as f64 * (onset[i + lag] - mean) as f64)
                    .sum();
                global_ac[lag] += ac;
            }

            // Chroma accumulation
            let total_chroma: f64 = block_chroma.iter().sum();
            if total_chroma > 0.0 {
                for i in 0..12 {
                    global_chroma[i] += block_chroma[i] / total_chroma;
                }
            }

            active_count += 1;
        }

        (active_count, global_chroma, global_ac, (flux_timeline, chroma_timeline))
    };

    let (mut active_block_count, mut global_chroma, mut global_ac, mut timelines) =
        run_analysis_loop(true);

    if active_block_count == 0 {
        // Fallback: Run without block filtering
        let res = run_analysis_loop(false);
        active_block_count = res.0;
        global_chroma = res.1;
        global_ac = res.2;
        timelines = res.3;
    }

    if active_block_count == 0 || global_ac.is_empty() {
        return Err("Audio too short or silent for joint analysis".to_string());
    }

    // Resolve BPM from global autocorrelation
    let fps = sample_rate as f64 / HOP_SIZE as f64;
    let lag_min = ((fps * 60.0 / 210.0).ceil() as usize).max(1);
    let lag_max = ((fps * 60.0 / 40.0).floor() as usize).min(global_ac.len() - 1);

    let mut best_score = f64::NEG_INFINITY;
    let mut best_lag = lag_min;

    for lag in lag_min..=lag_max {
        let ac = global_ac[lag];
        let bpm_at_lag = fps * 60.0 / lag as f64;
        let pref = if (80.0..=160.0).contains(&bpm_at_lag) {
            1.2
        } else {
            1.0
        };
        if ac * pref > best_score {
            best_score = ac * pref;
            best_lag = lag;
        }
    }

    let ac_fn = |l: usize| -> f64 {
        if l < lag_min || l > lag_max {
            return 0.0;
        }
        let ac = global_ac[l];
        let bpm_at_lag = fps * 60.0 / l as f64;
        let pref = if (80.0..=160.0).contains(&bpm_at_lag) {
            1.2
        } else {
            1.0
        };
        ac * pref
    };

    let half_lag = best_lag / 2;
    if half_lag >= lag_min && best_score > 0.0 && ac_fn(half_lag) > 0.70 * best_score {
        best_lag = half_lag;
    }

    let refined_lag = if best_lag > lag_min && best_lag < lag_max {
        let y0 = global_ac[best_lag - 1];
        let y1 = global_ac[best_lag];
        let y2 = global_ac[best_lag + 1];
        let denom = y0 - 2.0 * y1 + y2;
        if denom.abs() > 1e-10 {
            best_lag as f64 - 0.5 * (y2 - y0) / denom
        } else {
            best_lag as f64
        }
    } else {
        best_lag as f64
    };

    let bpm = fps * 60.0 / refined_lag;
    let bpm = if bpm < 70.0 { bpm * 2.0 } else { bpm };
    let bpm = (bpm * 100.0).round() / 100.0;

    // Resolve Key from global chroma
    let total_c: f64 = global_chroma.iter().sum();
    let mut normalized_chroma = global_chroma;
    if total_c > 0.0 {
        normalized_chroma.iter_mut().for_each(|v| *v /= total_c);
    }

    let mut suppressed = normalized_chroma;
    for i in 0..12 {
        let fifth = (i + 7) % 12;
        suppressed[fifth] = (suppressed[fifth] - 0.55 * normalized_chroma[i]).max(0.0);
        let third = (i + 4) % 12;
        suppressed[third] = (suppressed[third] - 0.30 * normalized_chroma[i]).max(0.0);
    }
    let total_s: f64 = suppressed.iter().sum();
    if total_s > 0.0 {
        suppressed.iter_mut().for_each(|v| *v /= total_s);
    }

    let mut best_corr = f64::NEG_INFINITY;
    let mut best_key = 0usize;
    let mut best_scale = "major";

    for root in 0..12usize {
        let r_major = pearson_with_rotation(&suppressed, &KS_MAJOR, root);
        let r_minor = pearson_with_rotation(&suppressed, &KS_MINOR, root);
        if r_major > best_corr {
            best_corr = r_major;
            best_key = root;
            best_scale = "major";
        }
        if r_minor > best_corr {
            best_corr = r_minor;
            best_key = root;
            best_scale = "minor";
        }
    }

    // Post-process the cached intermediate features.
    let (flux_timeline, chroma_timeline) = timelines;
    let onsets = pick_onset_peaks(&flux_timeline);
    let chroma_time_step = 0.2f32;
    let chroma_series = bin_chroma_series(&chroma_timeline, chroma_time_step);

    Ok(JointAnalysis {
        key: KEY_NAMES[best_key].to_string(),
        scale: best_scale.to_string(),
        key_strength: (best_corr * 10000.0).round() / 10000.0,
        bpm,
        onsets,
        chroma_series,
        chroma_time_step,
    })
}

/// Peak-pick a per-frame spectral-flux envelope into a compact list of onsets.
///
/// Uses an adaptive-threshold local-maximum picker: a frame is an onset peak
/// when it is the maximum within a small neighbourhood AND exceeds a local mean
/// (over a wider window) plus a fraction of the global flux scale. Strengths are
/// normalised to the peak flux (0..1) and rounded for compact storage.
fn pick_onset_peaks(flux_timeline: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let n = flux_timeline.len();
    if n == 0 {
        return Vec::new();
    }

    let flux: Vec<f32> = flux_timeline.iter().map(|&(_, f)| f).collect();
    let max_flux = flux.iter().cloned().fold(0.0f32, f32::max);
    if max_flux <= 0.0 {
        return Vec::new();
    }
    let mean_flux = flux.iter().sum::<f32>() / n as f32;

    // ~0.05 s @ 23 ms/frame => +/-2 frames local-max window;
    // ~0.5 s mean window => +/-11 frames for the adaptive threshold.
    let local_win = 2usize;
    let mean_win = 11usize;
    let delta = 0.10 * max_flux; // minimum prominence above local mean

    let mut peaks: Vec<(f32, f32)> = Vec::new();
    let mut last_peak_idx: Option<usize> = None;
    for i in 0..n {
        let v = flux[i];
        if v < mean_flux {
            continue;
        }
        // Local maximum check.
        let lo = i.saturating_sub(local_win);
        let hi = (i + local_win + 1).min(n);
        if flux[lo..hi].iter().cloned().fold(0.0f32, f32::max) > v {
            continue;
        }
        // Adaptive threshold: exceed local mean + delta.
        let mlo = i.saturating_sub(mean_win);
        let mhi = (i + mean_win + 1).min(n);
        let local_mean = flux[mlo..mhi].iter().sum::<f32>() / (mhi - mlo) as f32;
        if v < local_mean + delta {
            continue;
        }
        // Minimum spacing of ~0.05 s to avoid double-counting plateaus.
        if let Some(p) = last_peak_idx {
            if i - p < local_win {
                continue;
            }
        }
        let t = (flux_timeline[i].0 * 1000.0).round() / 1000.0;
        let strength = (v / max_flux * 1000.0).round() / 1000.0;
        peaks.push((t, strength));
        last_peak_idx = Some(i);
    }
    peaks
}

/// Bin a per-frame chroma timeline into a fixed-hop time-series.
///
/// Frames falling within each `step` window are averaged and L1-normalised so
/// each emitted vector sums to 1 (or all-zero for silent windows). The emitted
/// time is the window start (seconds from the analysis-window start).
fn bin_chroma_series(chroma_timeline: &[(f32, [f32; 12])], step: f32) -> Vec<(f32, [f32; 12])> {
    if chroma_timeline.is_empty() || step <= 0.0 {
        return Vec::new();
    }
    let t_end = chroma_timeline.last().unwrap().0;
    let num_bins = (t_end / step).floor() as usize + 1;

    let mut sums = vec![[0.0f64; 12]; num_bins];
    let mut counts = vec![0u32; num_bins];
    for &(t, chroma) in chroma_timeline {
        let bin = (t / step).floor() as usize;
        if bin >= num_bins {
            continue;
        }
        for i in 0..12 {
            sums[bin][i] += chroma[i] as f64;
        }
        counts[bin] += 1;
    }

    let mut series = Vec::with_capacity(num_bins);
    for bin in 0..num_bins {
        if counts[bin] == 0 {
            continue;
        }
        let total: f64 = sums[bin].iter().sum();
        let mut vec12 = [0.0f32; 12];
        if total > 0.0 {
            for i in 0..12 {
                vec12[i] = ((sums[bin][i] / total) as f32 * 10000.0).round() / 10000.0;
            }
        }
        let t = bin as f32 * step;
        series.push((t, vec12));
    }
    series
}

fn pearson_with_rotation(chroma: &[f64; 12], profile: &[f64; 12], root: usize) -> f64 {
    let mean_c = chroma.iter().sum::<f64>() / 12.0;
    let mean_p = profile.iter().sum::<f64>() / 12.0;
    let num: f64 = (0..12)
        .map(|i| (chroma[(i + root) % 12] - mean_c) * (profile[i] - mean_p))
        .sum();
    let den_c = chroma
        .iter()
        .map(|&x| (x - mean_c).powi(2))
        .sum::<f64>()
        .sqrt();
    let den_p = profile
        .iter()
        .map(|&x| (x - mean_p).powi(2))
        .sum::<f64>()
        .sqrt();
    if den_c * den_p == 0.0 {
        0.0
    } else {
        num / (den_c * den_p)
    }
}

fn create_wav_header(
    num_samples: usize,
    sample_rate: u32,
    num_channels: u16,
    bits_per_sample: u16,
) -> Vec<u8> {
    let mut header = Vec::with_capacity(44);
    let byte_rate = sample_rate * num_channels as u32 * (bits_per_sample / 8) as u32;
    let block_align = num_channels * (bits_per_sample / 8);
    let data_size = num_samples * num_channels as usize * (bits_per_sample as usize / 8);
    let file_size = 36 + data_size;

    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(file_size as u32).to_le_bytes());
    header.extend_from_slice(b"WAVE");

    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&num_channels.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&byte_rate.to_le_bytes());
    header.extend_from_slice(&block_align.to_le_bytes());
    header.extend_from_slice(&bits_per_sample.to_le_bytes());

    header.extend_from_slice(b"data");
    header.extend_from_slice(&(data_size as u32).to_le_bytes());

    header
}

pub fn encode_audio_to_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let header = create_wav_header(samples.len(), sample_rate, 1, 16);
    let mut wav_bytes = header;
    wav_bytes.reserve(samples.len() * 2);
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let s = (clamped * 32767.0) as i16;
        wav_bytes.extend_from_slice(&s.to_le_bytes());
    }
    wav_bytes
}

pub fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;
    while i < data.len() {
        let chunk = &data[i..(i + 3).min(data.len())];
        i += 3;
        let mut b = 0u32;
        for (idx, &byte) in chunk.iter().enumerate() {
            b |= (byte as u32) << (16 - idx * 8);
        }
        let char_count = chunk.len() + 1;
        for idx in 0..4 {
            if idx < char_count {
                let char_idx = ((b >> (18 - idx * 6)) & 0x3F) as usize;
                result.push(CHARSET[char_idx] as char);
            } else {
                result.push('=');
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
        assert_eq!(base64_encode(b"world!"), "d29ybGQh");
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn test_pick_onset_peaks_empty_and_silent() {
        assert!(pick_onset_peaks(&[]).is_empty());
        let silent: Vec<(f32, f32)> = (0..50).map(|i| (i as f32 * 0.023, 0.0)).collect();
        assert!(pick_onset_peaks(&silent).is_empty());
    }

    #[test]
    fn test_pick_onset_peaks_finds_spikes() {
        // Flat low envelope with two clear spikes; expect both to be picked.
        let mut env: Vec<(f32, f32)> = (0..60).map(|i| (i as f32 * 0.023, 0.1)).collect();
        env[20].1 = 5.0;
        env[45].1 = 4.0;
        let peaks = pick_onset_peaks(&env);
        assert_eq!(peaks.len(), 2, "expected two onset peaks, got {:?}", peaks);
        // Strength is normalised to the max flux (5.0) -> peak at idx20 == 1.0.
        assert!((peaks[0].1 - 1.0).abs() < 1e-3);
        // Times are the frame times of the spikes.
        assert!((peaks[0].0 - 20.0 * 0.023).abs() < 1e-3);
        assert!((peaks[1].0 - 45.0 * 0.023).abs() < 1e-3);
    }

    #[test]
    fn test_bin_chroma_series_bins_and_normalises() {
        let step = 0.2f32;
        // Two frames inside bin 0, one in bin 1. C (pc 0) dominant.
        let mut a = [0.0f32; 12];
        a[0] = 3.0;
        a[7] = 1.0;
        let mut b = [0.0f32; 12];
        b[0] = 1.0;
        let series = bin_chroma_series(&[(0.0, a), (0.05, a), (0.25, b)], step);
        assert_eq!(series.len(), 2);
        // Each emitted vector is L1-normalised.
        for (_, v) in &series {
            let sum: f32 = v.iter().sum();
            assert!((sum - 1.0).abs() < 1e-3, "vector not normalised: {:?}", v);
        }
        // Bin 0 starts at t=0, bin 1 at t=0.2.
        assert!((series[0].0 - 0.0).abs() < 1e-6);
        assert!((series[1].0 - 0.2).abs() < 1e-6);
        // Bin 1 is pure C.
        assert!((series[1].1[0] - 1.0).abs() < 1e-3);
    }

    #[test]
    fn test_encode_audio_to_wav_has_header() {
        let samples = vec![0.0f32; 100];
        let wav = encode_audio_to_wav(&samples, 16000);
        assert_eq!(wav.len(), 44 + 200);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
    }

    #[test]
    fn test_key_names_enharmonics() {
        assert_eq!(KEY_NAMES[3], "Eb");
        assert_eq!(KEY_NAMES[8], "Ab");
        assert_eq!(KEY_NAMES[10], "Bb");
    }

    #[test]
    fn test_pearson_perfect_correlation() {
        let chroma = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let profile = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert!(pearson_with_rotation(&chroma, &profile, 0) > 0.99);

        let shifted = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert!(pearson_with_rotation(&shifted, &profile, 1) > 0.99);
    }

    #[test]
    fn test_downsample_profile_empty() {
        let result = downsample_profile(&[], 128);
        assert_eq!(result.len(), 128);
        assert!(result.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn test_downsample_profile_exact() {
        let raw: Vec<f32> = (0..128).map(|i| i as f32).collect();
        let result = downsample_profile(&raw, 128);
        assert_eq!(result.len(), 128);
    }

    fn fixture(name: &str) -> String {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        format!("{}/tests/fixtures/{}", manifest, name)
    }

    #[test]
    fn test_analysis_mp3() {
        let path = fixture("(From Zombie) Re_ Brain Supply Issue.mp3");
        let result = run_audio_analysis(&path).expect("mp3 analysis failed");
        assert!(result.duration_seconds > 0, "duration should be non-zero");
        assert!(
            (40.0..=220.0).contains(&result.bpm.unwrap()),
            "bpm {:?} out of range",
            result.bpm
        );
        assert!(
            KEY_NAMES.contains(&result.key.as_deref().unwrap()),
            "unexpected key: {:?}",
            result.key
        );
        assert!(result.scale.as_deref().unwrap() == "major" || result.scale.as_deref().unwrap() == "minor");
        assert!(result.loudness_lufs < 0.0, "LUFS should be negative");
        let waveform: Vec<f32> = serde_json::from_str(&result.waveform_data).unwrap();
        assert_eq!(waveform.len(), 128);
        let silence_regions: Vec<[f64; 2]> = serde_json::from_str(&result.silence_regions).unwrap();
        assert!(!result.has_long_silence || !silence_regions.is_empty());
    }

    #[test]
    fn test_analysis_wav() {
        let path = fixture("(Tuesday) Men In The Machine.wav");
        let result = run_audio_analysis(&path).expect("wav analysis failed");
        assert!(result.duration_seconds > 0);
        assert!(
            (40.0..=220.0).contains(&result.bpm.unwrap()),
            "bpm {:?} out of range",
            result.bpm
        );
        assert!(KEY_NAMES.contains(&result.key.as_deref().unwrap()));
        assert!(result.loudness_lufs < 0.0);
        let waveform: Vec<f32> = serde_json::from_str(&result.waveform_data).unwrap();
        assert_eq!(waveform.len(), 128);
    }

    #[test]
    fn test_analysis_flac() {
        let path = fixture("AI Steering Committee.flac");
        let result = run_audio_analysis(&path).expect("flac analysis failed");
        assert!(result.duration_seconds > 0);
        assert!(
            (40.0..=220.0).contains(&result.bpm.unwrap()),
            "bpm {:?} out of range",
            result.bpm
        );
        assert!(KEY_NAMES.contains(&result.key.as_deref().unwrap()));
        assert!(result.loudness_lufs < 0.0);
        let waveform: Vec<f32> = serde_json::from_str(&result.waveform_data).unwrap();
        assert_eq!(waveform.len(), 128);
    }

    #[test]
    fn test_analysis_m4a() {
        let path = fixture("Digital Echoes.m4a");
        let result = run_audio_analysis(&path).expect("m4a analysis failed");
        assert!(result.duration_seconds > 0);
        assert!(
            (40.0..=220.0).contains(&result.bpm.unwrap()),
            "bpm {:?} out of range",
            result.bpm
        );
        assert!(KEY_NAMES.contains(&result.key.as_deref().unwrap()));
        assert!(result.loudness_lufs < 0.0);
        let waveform: Vec<f32> = serde_json::from_str(&result.waveform_data).unwrap();
        assert_eq!(waveform.len(), 128);
    }

    #[test]
    fn test_compute_bpm_too_short() {
        let samples = vec![0.0f32; 1000];
        let result = analyze_key_and_bpm_joint(&samples, 44100);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "Audio too short or silent for joint analysis");
    }

    #[test]
    fn test_detect_silence_regions_ignores_short_gaps() {
        let sample_rate = 100;
        let mut samples = vec![0.25; 100];
        samples.extend(vec![0.0; 150]);
        samples.extend(vec![0.25; 100]);

        let result = detect_silence_regions(&samples, sample_rate).unwrap();
        assert_eq!(result.silence_regions, "[]");
        assert!(!result.has_long_silence);
    }

    #[test]
    fn test_detect_silence_regions_flags_long_silence() {
        let sample_rate = 100;
        let mut samples = vec![0.25; 100];
        samples.extend(vec![0.0; 1_100]);
        samples.extend(vec![0.25; 100]);

        let result = detect_silence_regions(&samples, sample_rate).unwrap();
        let regions: Vec<[f64; 2]> = serde_json::from_str(&result.silence_regions).unwrap();
        assert_eq!(regions.len(), 1);
        assert!((regions[0][0] - 1.0).abs() < 0.01);
        assert!((regions[0][1] - 12.0).abs() < 0.01);
        assert!(result.has_long_silence);
    }

    #[test]
    fn test_run_analysis_empty_samples() {
        let result = run_audio_analysis("non_existent_file.mp3");
        assert!(result.is_err());
    }
}
