use std::fs::File;
use std::path::Path;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;
use rustfft::{FftPlanner, num_complex::Complex};

// Krumhansl-Schmuckler key profiles (root = index 0)
const KS_MAJOR: [f64; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const KS_MINOR: [f64; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];
const KEY_NAMES: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

pub struct AudioAnalysisResult {
    pub duration_seconds: u64,
    pub waveform_data: String,
    pub bpm: f64,
    pub key: String,
    pub scale: String,
    pub key_strength: f64,
    pub loudness_lufs: f64,
    pub loudness_range: f64,
}

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
    let sample_rate = codec_params.sample_rate.ok_or("No sample rate in codec params")?;

    // Derive duration from container metadata when available; count samples as fallback
    let container_duration: Option<u64> = codec_params.time_base.zip(codec_params.n_frames)
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
    let mut stereo_buf: Vec<f32> = Vec::new();

    while let Ok(packet) = probed.format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };

        stereo_buf.clear();
        let mut packet_sum_sq = 0.0f64;
        let mut packet_frames = 0usize;

        let handled = match decoded {
            AudioBufferRef::F32(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let c0 = buf.chan(0);
                if channels == 1 {
                    for &s in c0 {
                        mono_samples.push(s);
                        stereo_buf.push(s);
                        stereo_buf.push(s);
                        packet_sum_sq += (s * s) as f64;
                    }
                } else {
                    let c1 = buf.chan(1);
                    for i in 0..frames {
                        let mono = (c0[i] + c1[i]) * 0.5;
                        mono_samples.push(mono);
                        stereo_buf.push(c0[i]);
                        stereo_buf.push(c1[i]);
                        packet_sum_sq += (mono * mono) as f64;
                    }
                }
                packet_frames = frames;
                true
            }
            AudioBufferRef::F64(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let c0 = buf.chan(0);
                if channels == 1 {
                    for &s in c0 {
                        let v = s as f32;
                        mono_samples.push(v);
                        stereo_buf.push(v);
                        stereo_buf.push(v);
                        packet_sum_sq += (v * v) as f64;
                    }
                } else {
                    let c1 = buf.chan(1);
                    for i in 0..frames {
                        let mono = ((c0[i] + c1[i]) * 0.5) as f32;
                        mono_samples.push(mono);
                        stereo_buf.push(c0[i] as f32);
                        stereo_buf.push(c1[i] as f32);
                        packet_sum_sq += (mono * mono) as f64;
                    }
                }
                packet_frames = frames;
                true
            }
            AudioBufferRef::S16(buf) => {
                let norm = i16::MAX as f32;
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let c0 = buf.chan(0);
                if channels == 1 {
                    for &s in c0 {
                        let v = s as f32 / norm;
                        mono_samples.push(v);
                        stereo_buf.push(v);
                        stereo_buf.push(v);
                        packet_sum_sq += (v * v) as f64;
                    }
                } else {
                    let c1 = buf.chan(1);
                    for i in 0..frames {
                        let l = c0[i] as f32 / norm;
                        let r = c1[i] as f32 / norm;
                        let mono = (l + r) * 0.5;
                        mono_samples.push(mono);
                        stereo_buf.push(l);
                        stereo_buf.push(r);
                        packet_sum_sq += (mono * mono) as f64;
                    }
                }
                packet_frames = frames;
                true
            }
            AudioBufferRef::S32(buf) => {
                let norm = i32::MAX as f32;
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let c0 = buf.chan(0);
                if channels == 1 {
                    for &s in c0 {
                        let v = s as f32 / norm;
                        mono_samples.push(v);
                        stereo_buf.push(v);
                        stereo_buf.push(v);
                        packet_sum_sq += (v * v) as f64;
                    }
                } else {
                    let c1 = buf.chan(1);
                    for i in 0..frames {
                        let l = c0[i] as f32 / norm;
                        let r = c1[i] as f32 / norm;
                        let mono = (l + r) * 0.5;
                        mono_samples.push(mono);
                        stereo_buf.push(l);
                        stereo_buf.push(r);
                        packet_sum_sq += (mono * mono) as f64;
                    }
                }
                packet_frames = frames;
                true
            }
            AudioBufferRef::U8(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let c0 = buf.chan(0);
                if channels == 1 {
                    for &s in c0 {
                        let v = (s as f32 - 128.0) / 128.0;
                        mono_samples.push(v);
                        stereo_buf.push(v);
                        stereo_buf.push(v);
                        packet_sum_sq += (v * v) as f64;
                    }
                } else {
                    let c1 = buf.chan(1);
                    for i in 0..frames {
                        let l = (c0[i] as f32 - 128.0) / 128.0;
                        let r = (c1[i] as f32 - 128.0) / 128.0;
                        let mono = (l + r) * 0.5;
                        mono_samples.push(mono);
                        stereo_buf.push(l);
                        stereo_buf.push(r);
                        packet_sum_sq += (mono * mono) as f64;
                    }
                }
                packet_frames = frames;
                true
            }
            _ => false,
        };

        if !handled { continue; }

        if !stereo_buf.is_empty() {
            let _ = meter.add_frames_f32(&stereo_buf);
        }
        if packet_frames > 0 {
            rms_energies.push((packet_sum_sq / packet_frames as f64).sqrt() as f32);
        }
    }

    let duration_seconds = container_duration
        .unwrap_or_else(|| (mono_samples.len() as f64 / sample_rate as f64).round() as u64);

    let loudness_lufs = meter.loudness_global()
        .map(|v| (v * 100.0).round() / 100.0)
        .unwrap_or(f64::NEG_INFINITY);
    let loudness_range = meter.loudness_range()
        .map(|v| (v * 100.0).round() / 100.0)
        .unwrap_or(0.0);

    let waveform = downsample_profile(&rms_energies, 128);
    let waveform_data = serde_json::to_string(&waveform).map_err(|e| e.to_string())?;

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

    let (key, scale, key_strength) = compute_key_from_mono(cropped, sample_rate)?;
    let bpm = compute_bpm_from_mono(cropped, sample_rate)?;

    Ok(AudioAnalysisResult {
        duration_seconds,
        waveform_data,
        bpm,
        key,
        scale,
        key_strength,
        loudness_lufs,
        loudness_range,
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

fn compute_key_from_mono(samples: &[f32], sample_rate: u32) -> Result<(String, String, f64), String> {
    const FFT_SIZE: usize = 4096;
    const HOP_SIZE: usize = 2048;

    let hann: Vec<f32> = (0..FFT_SIZE)
        .map(|n| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * n as f32 / (FFT_SIZE - 1) as f32).cos()))
        .collect();

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let mut buf: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); FFT_SIZE];
    let mut chroma = [0.0f64; 12];

    let sr = sample_rate as f64;
    let mut frame_start = 0;

    while frame_start + FFT_SIZE <= samples.len() {
        for (i, c) in buf.iter_mut().enumerate() {
            c.re = samples[frame_start + i] * hann[i];
            c.im = 0.0;
        }
        fft.process(&mut buf);

        for k in 1..FFT_SIZE / 2 {
            let freq = k as f64 * sr / FFT_SIZE as f64;
            if freq < 65.0 || freq > 4000.0 {
                continue;
            }
            let mag = (buf[k].re as f64).hypot(buf[k].im as f64);
            let semitone = 12.0 * (freq / 440.0).log2() + 69.0;
            let pc = (semitone.round() as i64).rem_euclid(12) as usize;
            chroma[pc] += mag;
        }

        frame_start += HOP_SIZE;
    }

    let total: f64 = chroma.iter().sum();
    if total > 0.0 {
        chroma.iter_mut().for_each(|v| *v /= total);
    }

    // HPCP-style harmonic suppression
    let mut suppressed = chroma;
    for i in 0..12 {
        let fifth = (i + 7) % 12;
        suppressed[fifth] = (suppressed[fifth] - 0.55 * chroma[i]).max(0.0);
        let third = (i + 4) % 12;
        suppressed[third] = (suppressed[third] - 0.30 * chroma[i]).max(0.0);
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
        if r_major > best_corr { best_corr = r_major; best_key = root; best_scale = "major"; }
        if r_minor > best_corr { best_corr = r_minor; best_key = root; best_scale = "minor"; }
    }

    Ok((
        KEY_NAMES[best_key].to_string(),
        best_scale.to_string(),
        (best_corr * 10000.0).round() / 10000.0,
    ))
}

fn compute_bpm_from_mono(samples: &[f32], sample_rate: u32) -> Result<f64, String> {
    const FFT_SIZE: usize = 1024;
    const HOP_SIZE: usize = 512;

    let hann: Vec<f32> = (0..FFT_SIZE)
        .map(|n| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * n as f32 / (FFT_SIZE - 1) as f32).cos()))
        .collect();

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let mut buf: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); FFT_SIZE];
    let mut prev_mag: Vec<f32> = vec![0.0; FFT_SIZE / 2];
    let mut onset: Vec<f32> = Vec::new();

    let mut frame_start = 0;
    while frame_start + FFT_SIZE <= samples.len() {
        for (i, c) in buf.iter_mut().enumerate() {
            c.re = samples[frame_start + i] * hann[i];
            c.im = 0.0;
        }
        fft.process(&mut buf);

        let mut flux = 0.0f32;
        for k in 0..FFT_SIZE / 2 {
            let mag = (buf[k].re as f64).hypot(buf[k].im as f64) as f32;
            let diff = mag - prev_mag[k];
            if diff > 0.0 { flux += diff; }
            prev_mag[k] = mag;
        }
        onset.push(flux);
        frame_start += HOP_SIZE;
    }

    let n = onset.len();
    if n < 30 {
        return Err("Audio too short for BPM detection".to_string());
    }

    let fps = sample_rate as f64 / HOP_SIZE as f64;
    let lag_min = ((fps * 60.0 / 210.0).ceil() as usize).max(1);
    let lag_max = ((fps * 60.0 / 40.0).floor() as usize).min(n / 2);
    let mean = onset.iter().sum::<f32>() / n as f32;

    let mut best_score = f64::NEG_INFINITY;
    let mut best_lag = lag_min;

    for lag in lag_min..=lag_max {
        let ac: f64 = (0..n - lag)
            .map(|i| (onset[i] - mean) as f64 * (onset[i + lag] - mean) as f64)
            .sum();
        let bpm_at_lag = fps * 60.0 / lag as f64;
        let pref = if (80.0..=160.0).contains(&bpm_at_lag) { 1.2 } else { 1.0 };
        if ac * pref > best_score {
            best_score = ac * pref;
            best_lag = lag;
        }
    }

    // Check if double tempo (half lag) has a strong enough score
    let ac_fn = |l: usize| -> f64 {
        if l < lag_min || l > lag_max { return 0.0; }
        let ac: f64 = (0..n - l)
            .map(|i| (onset[i] - mean) as f64 * (onset[i + l] - mean) as f64)
            .sum();
        let bpm_at_lag = fps * 60.0 / l as f64;
        let pref = if (80.0..=160.0).contains(&bpm_at_lag) { 1.2 } else { 1.0 };
        ac * pref
    };

    let half_lag = best_lag / 2;
    if half_lag >= lag_min && best_score > 0.0 && ac_fn(half_lag) > 0.70 * best_score {
        best_lag = half_lag;
    }

    // Sub-sample refinement via parabolic interpolation
    let refined_lag = if best_lag > lag_min && best_lag < lag_max {
        let ac = |l: usize| -> f64 {
            (0..n - l).map(|i| (onset[i] - mean) as f64 * (onset[i + l] - mean) as f64).sum()
        };
        let y0 = ac(best_lag - 1);
        let y1 = ac(best_lag);
        let y2 = ac(best_lag + 1);
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
    Ok((bpm * 100.0).round() / 100.0)
}

fn pearson_with_rotation(chroma: &[f64; 12], profile: &[f64; 12], root: usize) -> f64 {
    let mean_c = chroma.iter().sum::<f64>() / 12.0;
    let mean_p = profile.iter().sum::<f64>() / 12.0;
    let num: f64 = (0..12)
        .map(|i| (chroma[(i + root) % 12] - mean_c) * (profile[i] - mean_p))
        .sum();
    let den_c = chroma.iter().map(|&x| (x - mean_c).powi(2)).sum::<f64>().sqrt();
    let den_p = profile.iter().map(|&x| (x - mean_p).powi(2)).sum::<f64>().sqrt();
    if den_c * den_p == 0.0 { 0.0 } else { num / (den_c * den_p) }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!((40.0..=220.0).contains(&result.bpm), "bpm {} out of range", result.bpm);
        assert!(KEY_NAMES.contains(&result.key.as_str()), "unexpected key: {}", result.key);
        assert!(result.scale == "major" || result.scale == "minor");
        assert!(result.loudness_lufs < 0.0, "LUFS should be negative");
        let waveform: Vec<f32> = serde_json::from_str(&result.waveform_data).unwrap();
        assert_eq!(waveform.len(), 128);
    }

    #[test]
    fn test_analysis_wav() {
        let path = fixture("(Tuesday) Men In The Machine.wav");
        let result = run_audio_analysis(&path).expect("wav analysis failed");
        assert!(result.duration_seconds > 0);
        assert!((40.0..=220.0).contains(&result.bpm), "bpm {} out of range", result.bpm);
        assert!(KEY_NAMES.contains(&result.key.as_str()));
        assert!(result.loudness_lufs < 0.0);
        let waveform: Vec<f32> = serde_json::from_str(&result.waveform_data).unwrap();
        assert_eq!(waveform.len(), 128);
    }

    #[test]
    fn test_analysis_flac() {
        let path = fixture("AI Steering Committee.flac");
        let result = run_audio_analysis(&path).expect("flac analysis failed");
        assert!(result.duration_seconds > 0);
        assert!((40.0..=220.0).contains(&result.bpm), "bpm {} out of range", result.bpm);
        assert!(KEY_NAMES.contains(&result.key.as_str()));
        assert!(result.loudness_lufs < 0.0);
        let waveform: Vec<f32> = serde_json::from_str(&result.waveform_data).unwrap();
        assert_eq!(waveform.len(), 128);
    }

    #[test]
    fn test_analysis_m4a() {
        let path = fixture("Digital Echoes.m4a");
        let result = run_audio_analysis(&path).expect("m4a analysis failed");
        assert!(result.duration_seconds > 0);
        assert!((40.0..=220.0).contains(&result.bpm), "bpm {} out of range", result.bpm);
        assert!(KEY_NAMES.contains(&result.key.as_str()));
        assert!(result.loudness_lufs < 0.0);
        let waveform: Vec<f32> = serde_json::from_str(&result.waveform_data).unwrap();
        assert_eq!(waveform.len(), 128);
    }
}
