//! Opt-in accuracy check: runs the real pipeline (the app's own audio decoder, the speaker model,
//! clustering) on recordings that come with the true speaker for every line.
//!
//! Ignored by default because it needs the downloaded model and some audio. Each recording is
//! `<name>.<m4a|mp4|wav>` plus `<name>.json`: `[{"start": 1.2, "end": 4.0, "speaker": "spk1"}, ...]`.
//!
//! ```text
//! SPEAKER_TEST_MODEL=/path/to/wespeaker_en_voxceleb_resnet34.onnx \
//! SPEAKER_TEST_DIR=/path/to/recordings \
//!   cargo test --lib speaker_accuracy -- --ignored --nocapture
//! ```

use super::embedder::SpeakerEmbedder;
use super::fbank::SAMPLE_RATE;
use super::labeling::{assign_speakers, LineInput, Options};
use crate::audio::decoder::decode_audio_file;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(serde::Deserialize)]
struct Turn {
    start: f64,
    end: f64,
    speaker: String,
}

/// Share of lines that agree under the best one-to-one matching of detected to true speakers.
fn accuracy(truth: &[usize], predicted: &[usize]) -> f64 {
    let kt = truth.iter().max().map_or(0, |m| m + 1);
    let kp = predicted.iter().max().map_or(0, |m| m + 1);
    let mut confusion = vec![vec![0usize; kt]; kp];
    for (t, p) in truth.iter().zip(predicted) {
        confusion[*p][*t] += 1;
    }
    fn best(confusion: &[Vec<usize>], p: usize, used: &mut [bool]) -> usize {
        if p == confusion.len() {
            return 0;
        }
        let mut top = best(confusion, p + 1, used); // leave this detected speaker unmatched
        for t in 0..used.len() {
            if !used[t] {
                used[t] = true;
                top = top.max(confusion[p][t] + best(confusion, p + 1, used));
                used[t] = false;
            }
        }
        top
    }
    best(&confusion, 0, &mut vec![false; kt]) as f64 / truth.len().max(1) as f64
}

fn audio_for(dir: &Path, name: &str) -> Option<PathBuf> {
    ["m4a", "mp4", "wav"].iter().map(|ext| dir.join(format!("{name}.{ext}"))).find(|p| p.exists())
}

#[test]
fn accuracy_helper_matches_labels_up_to_renaming() {
    assert_eq!(accuracy(&[0, 0, 1, 1], &[1, 1, 0, 0]), 1.0);
    assert_eq!(accuracy(&[0, 0, 1, 1], &[0, 0, 0, 0]), 0.5);
    assert_eq!(accuracy(&[0, 1, 2, 0], &[0, 1, 1, 0]), 0.75);
}

#[test]
#[ignore = "needs SPEAKER_TEST_MODEL and SPEAKER_TEST_DIR (see module docs)"]
fn speaker_accuracy_on_real_recordings() {
    let model = std::env::var("SPEAKER_TEST_MODEL").expect("set SPEAKER_TEST_MODEL");
    let dir = PathBuf::from(std::env::var("SPEAKER_TEST_DIR").expect("set SPEAKER_TEST_DIR"));
    let min_accuracy: f64 = std::env::var("SPEAKER_TEST_MIN_ACCURACY").ok().and_then(|v| v.parse().ok()).unwrap_or(0.85);
    let mut embedder = SpeakerEmbedder::load(Path::new(&model)).expect("load model");

    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().and_then(|f| f.strip_suffix(".json")).map(str::to_string))
        .collect();
    names.sort();

    println!("{:<20} {:>5} {:>5} | {:>11} | {:>16}", "recording", "lines", "true", "known-count", "estimated: k/acc");
    let mut scored = Vec::new();
    for name in names {
        let Some(audio) = audio_for(&dir, &name) else { continue };
        let turns: Vec<Turn> = serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap()).unwrap();
        // Exactly what api_detect_speakers does: decode with the app's decoder, then 16 kHz mono
        let samples = decode_audio_file(&audio).expect("decode").to_whisper_format();

        let lines: Vec<LineInput> = turns
            .iter()
            .map(|t| {
                let from = ((t.start * SAMPLE_RATE as f64) as usize).min(samples.len());
                let to = ((t.end * SAMPLE_RATE as f64) as usize).min(samples.len());
                LineInput { duration: t.end - t.start, embedding: embedder.embed(&samples[from..to]).unwrap() }
            })
            .collect();
        if turns.iter().all(|t| t.speaker == "?") {
            // No answer key (speaker "?"): nothing to score
            println!("{:<20} {:>5}  (no ground truth, skipped)", name, turns.len());
            continue;
        }
        let mut ids: BTreeMap<&str, usize> = BTreeMap::new();
        let truth: Vec<usize> = turns.iter().map(|t| { let n = ids.len(); *ids.entry(&t.speaker).or_insert(n) }).collect();

        let known = assign_speakers(&lines, &Options { num_speakers: Some(ids.len()), ..Options::default() });
        let auto = assign_speakers(&lines, &Options::default());
        let known_acc = accuracy(&truth, &known.labels);
        println!("{:<20} {:>5} {:>5} | {:>11.2} | {:>7} / {:.2}", name, turns.len(), ids.len(), known_acc, auto.num_speakers, accuracy(&truth, &auto.labels));
        // Recordings built from near-identical synthetic voices are expected to fail; report only
        if !name.contains("hard") {
            scored.push((name, known_acc));
        }
    }

    assert!(!scored.is_empty(), "no recordings found in {}", dir.display());
    let mean = scored.iter().map(|(_, a)| a).sum::<f64>() / scored.len() as f64;
    println!("mean accuracy with the speaker count given: {mean:.3} over {} recordings", scored.len());
    assert!(mean >= min_accuracy, "mean accuracy {mean:.3} below {min_accuracy}");
}
