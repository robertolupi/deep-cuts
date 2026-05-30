/// Essentia-compatible log-mel spectrogram and patch extraction for the
/// Discogs-Effnet classifier pipeline.
///
/// Parameters match the Essentia MelBands / FrameCutter / Windowing settings:
///   sample rate : 16 000 Hz
///   FFT size    : 512
///   hop size    : 256
///   mel bands   : 96  (0 – 8 000 Hz, Slaney, unit_tri normalisation)
///   log scaling : log10(1 + 10000 × energy)
///
/// The mel filterbank weights are pre-computed by
/// `tools/export_essentia_mel_weights.py` and embedded at compile time.
use ndarray::Array2;
use rustfft::{num_complex::Complex, FftPlanner};

const MEL_WEIGHTS_RAW: &[u8] = include_bytes!("essentia_mel_weights.bin");

fn get_mel_weights() -> Vec<f32> {
    MEL_WEIGHTS_RAW
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

/// Resamples a mono f32 buffer from `source_rate` to 16 000 Hz.
pub fn resample_to_16k(input: &[f32], source_rate: u32) -> Result<Vec<f32>, String> {
    if source_rate == 16_000 {
        return Ok(input.to_vec());
    }

    use rubato::{FftFixedInOut, Resampler};

    let chunk_size = 1024usize;
    let mut resampler =
        FftFixedInOut::<f32>::new(source_rate as usize, 16_000, chunk_size, 1)
            .map_err(|e| e.to_string())?;

    let mut output = Vec::new();
    let mut read_idx = 0;
    while read_idx < input.len() {
        let needed = resampler.input_frames_next();
        let mut chan = vec![0.0f32; needed];
        let copy = (input.len() - read_idx).min(needed);
        chan[..copy].copy_from_slice(&input[read_idx..read_idx + copy]);
        let processed = resampler.process(&[chan], None).map_err(|e| e.to_string())?;
        if let Some(ch) = processed.first() {
            output.extend_from_slice(ch);
        }
        read_idx += needed;
    }

    let expected = ((input.len() as f64) * 16_000.0 / source_rate as f64).round() as usize;
    output.truncate(expected.max(output.len().min(expected + 128)));
    Ok(output)
}

/// Computes the log-mel spectrogram for a 16 kHz mono buffer.
/// Returns an Array2 of shape `(n_frames, 96)`.
pub fn compute_log_mel_spectrogram(audio_16k: &[f32]) -> Result<Array2<f32>, String> {
    const FFT_SIZE: usize = 512;
    const HOP_SIZE: usize = 256;
    const N_BANDS: usize = 96;
    const N_BINS: usize = FFT_SIZE / 2 + 1; // 257

    if audio_16k.len() < FFT_SIZE {
        return Err("Audio buffer too short for spectrogram analysis".to_string());
    }

    let hann: Vec<f32> = (0..FFT_SIZE)
        .map(|n| {
            0.5 * (1.0
                - (2.0 * std::f32::consts::PI * n as f32 / (FFT_SIZE - 1) as f32).cos())
        })
        .collect();

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

    let mel_weights = get_mel_weights();
    if mel_weights.len() != N_BANDS * N_BINS {
        return Err(format!(
            "essentia_mel_weights.bin size mismatch: expected {} floats, got {}",
            N_BANDS * N_BINS,
            mel_weights.len()
        ));
    }

    let mut frames = Vec::new();
    let mut fft_buf = vec![Complex::new(0.0f32, 0.0f32); FFT_SIZE];

    let mut frame_start = 0;
    while frame_start + FFT_SIZE <= audio_16k.len() {
        // Hann window + zero-phase circular shift
        for i in 0..FFT_SIZE {
            let win = audio_16k[frame_start + i] * hann[i];
            let shift_idx = (i + FFT_SIZE / 2) % FFT_SIZE;
            fft_buf[shift_idx].re = win;
            fft_buf[shift_idx].im = 0.0;
        }

        fft.process(&mut fft_buf);

        // Power spectrum
        let mut power = vec![0.0f32; N_BINS];
        for k in 0..N_BINS {
            let re = fft_buf[k].re;
            let im = fft_buf[k].im;
            power[k] = re * re + im * im;
        }

        // Mel filterbank
        let mut mel = vec![0.0f32; N_BANDS];
        for m in 0..N_BANDS {
            let off = m * N_BINS;
            for k in 0..N_BINS {
                mel[m] += power[k] * mel_weights[off + k];
            }
        }

        // log10(1 + 10000 × energy)
        let log_mel: Vec<f32> = mel.iter().map(|&e| (1.0 + 10_000.0 * e).log10()).collect();
        frames.push(log_mel);
        frame_start += HOP_SIZE;
    }

    let n_frames = frames.len();
    if n_frames == 0 {
        return Err("No frames extracted from audio".to_string());
    }

    let mut spec = Array2::<f32>::zeros((n_frames, N_BANDS));
    for (fi, frame) in frames.iter().enumerate() {
        for m in 0..N_BANDS {
            spec[[fi, m]] = frame[m];
        }
    }
    Ok(spec)
}

/// Segments the spectrogram into overlapping 128-frame patches (hop 62).
/// Zero-pads if the spectrogram is shorter than one patch.
/// Returns flat `(128 × 96)` vectors suitable for ONNX input.
pub fn extract_patches(spec: &Array2<f32>) -> Result<Vec<Vec<f32>>, String> {
    const PATCH_SIZE: usize = 128;
    const PATCH_HOP: usize = 62;
    const N_BANDS: usize = 96;

    let n_frames = spec.nrows();
    let mut patches = Vec::new();

    if n_frames < PATCH_SIZE {
        let mut padded = vec![0.0f32; PATCH_SIZE * N_BANDS];
        for i in 0..n_frames {
            for m in 0..N_BANDS {
                padded[i * N_BANDS + m] = spec[[i, m]];
            }
        }
        patches.push(padded);
    } else {
        let mut start = 0;
        while start + PATCH_SIZE <= n_frames {
            let mut patch = Vec::with_capacity(PATCH_SIZE * N_BANDS);
            for i in start..start + PATCH_SIZE {
                for m in 0..N_BANDS {
                    patch.push(spec[[i, m]]);
                }
            }
            patches.push(patch);
            start += PATCH_HOP;
        }
    }

    Ok(patches)
}
