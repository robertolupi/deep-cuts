// viterbi_decoder.rs
// Rust implementation for aligning structural search sequences against ONNX prediction probabilities
// Adapted to match our actual sample_predictions.json schema.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Deserialize)]
struct SegmentPrediction {
    segment_index: usize,
    sax_char: String,
    waveform_value: f32,
    predicted_label: String,
    probabilities: HashMap<String, f32>,
}

#[derive(Debug, Deserialize)]
struct TrackPrediction {
    title: String,
    artist: String,
    waveform_sax: String,
    predictions: Vec<SegmentPrediction>,
}

#[derive(Debug, Serialize)]
struct Decoded {
    title: String,
    artist: String,
    path: Vec<String>,
    log_prob: f32,
}

fn viterbi(probs: &[Vec<f32>], transition: &[Vec<f32>], init: &[f32]) -> (Vec<usize>, f32) {
    let t_len = probs.len();
    let n_states = probs[0].len();
    
    // log probabilities for numerical stability
    let mut dp = vec![vec![f32::NEG_INFINITY; n_states]; t_len];
    let mut back = vec![vec![0usize; n_states]; t_len];
    
    // initialization
    for s in 0..n_states {
        dp[0][s] = init[s].ln() + probs[0][s].ln().max(-1e9);
    }
    
    // recursion
    for t in 1..t_len {
        for s in 0..n_states {
            let mut best_val = f32::NEG_INFINITY;
            let mut best_prev = 0;
            for sp in 0..n_states {
                let val = dp[t-1][sp] + transition[sp][s].ln() + probs[t][s].ln().max(-1e9);
                if val > best_val {
                    best_val = val;
                    best_prev = sp;
                }
            }
            dp[t][s] = best_val;
            back[t][s] = best_prev;
        }
    }
    
    // termination
    let (mut best_last, mut best_score) = (0, f32::NEG_INFINITY);
    for s in 0..n_states {
        if dp[t_len-1][s] > best_score {
            best_score = dp[t_len-1][s];
            best_last = s;
        }
    }
    
    // backtrack
    let mut path = vec![0usize; t_len];
    path[t_len-1] = best_last;
    for t in (1..t_len).rev() {
        path[t-1] = back[t][path[t]];
    }
    
    (path, best_score)
}

fn build_transition_matrix(labels: &[String]) -> Vec<Vec<f32>> {
    let n = labels.len();
    let mut trans = vec![vec![0.01; n]; n]; // small smoothing
    
    // Example structural priors for music:
    let mut priors = HashMap::new();
    priors.insert(("intro", "verse"), 0.7);
    priors.insert(("intro", "chorus"), 0.2);
    priors.insert(("verse", "chorus"), 0.6);
    priors.insert(("verse", "verse"), 0.2);
    priors.insert(("chorus", "verse"), 0.5);
    priors.insert(("chorus", "bridge"), 0.2);
    priors.insert(("bridge", "chorus"), 0.6);
    priors.insert(("chorus", "outro"), 0.3);
    
    for (i, from) in labels.iter().enumerate() {
        for (j, to) in labels.iter().enumerate() {
            if let Some(&p) = priors.get(&(from.as_str(), to.as_str())) {
                trans[i][j] = p;
            }
        }
        // normalize
        let sum: f32 = trans[i].iter().sum();
        for v in &mut trans[i] {
            *v /= sum;
        }
    }
    trans
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = fs::read_to_string("sample_predictions.json")?;
    let predictions: Vec<TrackPrediction> = serde_json::from_str(&data)?;
    
    let labels = vec![
        "unknown".to_string(),
        "intro".to_string(),
        "verse".to_string(),
        "pre-chorus".to_string(),
        "chorus".to_string(),
        "bridge".to_string(),
        "outro".to_string(),
        "end".to_string(),
    ];
    let n_states = labels.len();
    let init = vec![1.0 / n_states as f32; n_states];
    let trans = build_transition_matrix(&labels);
    
    let mut results = Vec::new();
    
    for pred in predictions {
        // Build probs matrix from the predictions mapping [time][state]
        let mut probs = Vec::new();
        for step in &pred.predictions {
            let mut step_probs = Vec::new();
            for label in &labels {
                let p = step.probabilities.get(label).cloned().unwrap_or(0.0);
                step_probs.push(p);
            }
            probs.push(step_probs);
        }
        
        let (path_idx, score) = viterbi(&probs, &trans, &init);
        let path_labels = path_idx.iter().map(|&i| labels[i].clone()).collect();
        
        results.push(Decoded {
            title: pred.title,
            artist: pred.artist,
            path: path_labels,
            log_prob: score,
        });
    }
    
    println!("{}", serde_json::to_string_pretty(&results)?);
    Ok(())
}
