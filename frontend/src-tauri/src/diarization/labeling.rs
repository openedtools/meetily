//! Turn per-line voice embeddings into a speaker number for every transcript line.
//!
//! Only lines long enough to carry a reliable voice are clustered. Shorter lines join the nearest
//! voice; lines too short to measure at all take the speaker of the line next to them.

use super::cluster::{average_linkage, cosine_distances, cosine_similarity, estimate_num_speakers, normalized};

#[derive(Debug, Clone)]
pub struct Options {
    /// Exact number of speakers, when the user knows it. `None` estimates it.
    pub num_speakers: Option<usize>,
    /// Lines at least this long (seconds) are used to find the voices.
    pub min_cluster_secs: f64,
    /// Upper bound for the automatic estimate.
    pub max_speakers: usize,
    /// Cluster at most this many lines (spread evenly); the rest join the nearest voice.
    pub max_cluster_items: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { num_speakers: None, min_cluster_secs: 1.0, max_speakers: 8, max_cluster_items: 800 }
    }
}

/// One transcript line: how long it is, and its voice embedding if it was long enough to measure.
#[derive(Debug, Clone)]
pub struct LineInput {
    pub duration: f64,
    /// L2-normalised voice embedding, or `None` for lines too short to measure.
    pub embedding: Option<Vec<f32>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Labeling {
    /// Speaker index for each line (0-based, numbered by first appearance in the meeting).
    pub labels: Vec<usize>,
    pub num_speakers: usize,
}

pub fn assign_speakers(lines: &[LineInput], opts: &Options) -> Labeling {
    if lines.is_empty() {
        return Labeling { labels: Vec::new(), num_speakers: 0 };
    }

    // Lines that can anchor a voice. If none are long enough, use whatever we could measure.
    let mut anchors: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].embedding.is_some() && lines[i].duration >= opts.min_cluster_secs)
        .collect();
    if anchors.is_empty() {
        anchors = (0..lines.len()).filter(|&i| lines[i].embedding.is_some()).collect();
    }
    if anchors.is_empty() {
        // Nothing measurable: one speaker.
        return Labeling { labels: vec![0; lines.len()], num_speakers: 1 };
    }
    if anchors.len() > opts.max_cluster_items.max(2) {
        let step = anchors.len().div_ceil(opts.max_cluster_items.max(2));
        anchors = anchors.into_iter().step_by(step).collect();
    }

    let embeddings: Vec<Vec<f32>> = anchors.iter().map(|&i| lines[i].embedding.clone().unwrap()).collect();
    let tree = average_linkage(&cosine_distances(&embeddings), embeddings.len());
    let k = match opts.num_speakers {
        Some(k) => k.clamp(1, embeddings.len()),
        None => estimate_num_speakers(&tree, opts.max_speakers),
    };
    let anchor_labels = tree.labels_for_k(k);
    let k = anchor_labels.iter().max().map_or(1, |m| m + 1);

    // One centroid per voice
    let dim = embeddings[0].len();
    let mut centroids = vec![vec![0.0f32; dim]; k];
    for (emb, &label) in embeddings.iter().zip(&anchor_labels) {
        centroids[label].iter_mut().zip(emb).for_each(|(c, e)| *c += e);
    }
    let centroids: Vec<Vec<f32>> = centroids.into_iter().map(normalized).collect();

    let mut labels: Vec<Option<usize>> = vec![None; lines.len()];
    for (&line, &label) in anchors.iter().zip(&anchor_labels) {
        labels[line] = Some(label);
    }
    // Shorter lines: nearest voice
    for (i, line) in lines.iter().enumerate() {
        if labels[i].is_none() {
            if let Some(embedding) = &line.embedding {
                labels[i] = (0..k).max_by(|&a, &b| {
                    cosine_similarity(embedding, &centroids[a]).total_cmp(&cosine_similarity(embedding, &centroids[b]))
                });
            }
        }
    }
    // Unmeasurable lines: whoever was speaking just before, else just after
    for i in 0..lines.len() {
        if labels[i].is_none() {
            labels[i] = labels[..i].iter().rev().flatten().next().copied().or_else(|| labels[i + 1..].iter().flatten().next().copied());
        }
    }

    // Number speakers in order of first appearance so "Speaker 1" is whoever talks first
    let mut order: Vec<usize> = Vec::new();
    let labels: Vec<usize> = labels
        .into_iter()
        .map(|l| {
            let l = l.unwrap_or(0);
            order.iter().position(|&o| o == l).unwrap_or_else(|| {
                order.push(l);
                order.len() - 1
            })
        })
        .collect();
    Labeling { num_speakers: order.len().max(1), labels }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next_f32(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            ((self.0 >> 11) as f64 / (1u64 << 53) as f64) as f32 * 2.0 - 1.0
        }
    }

    /// One line per entry of `pattern` (the voice speaking it), each a noisy unit vector around
    /// that voice. Voices point along different axes, so they are clearly distinct by construction.
    fn conversation(pattern: &[usize], spread: f32, seed: u64, durations: impl Fn(usize) -> f64) -> (Vec<LineInput>, Vec<usize>) {
        let k = pattern.iter().max().unwrap() + 1;
        let mut rng = Rng(seed);
        let protos: Vec<Vec<f32>> = (0..k).map(|v| (0..16).map(|d| if d == v { 1.0 } else { 0.0 }).collect()).collect();
        let lines = pattern
            .iter()
            .enumerate()
            .map(|(i, &v)| LineInput {
                duration: durations(i),
                embedding: Some(normalized(protos[v].iter().map(|x| x + spread * rng.next_f32()).collect())),
            })
            .collect();
        (lines, pattern.to_vec())
    }

    fn same_partition(a: &[usize], b: &[usize]) -> bool {
        (0..a.len()).all(|i| (0..a.len()).all(|j| (a[i] == a[j]) == (b[i] == b[j])))
    }

    const TURNS: [usize; 24] = [0, 1, 2, 0, 1, 1, 2, 0, 2, 1, 0, 0, 1, 2, 2, 0, 1, 0, 2, 1, 1, 0, 2, 0];

    #[test]
    fn labels_are_numbered_by_first_appearance() {
        // The first line is spoken by voice 2, so it must come out as speaker 0
        let pattern: Vec<usize> = TURNS.iter().map(|v| 2 - v).collect();
        let (lines, truth) = conversation(&pattern, 0.15, 3, |_| 3.0);
        let result = assign_speakers(&lines, &Options::default());
        assert_eq!(result.num_speakers, 3);
        assert_eq!(result.labels[0], 0);
        assert!(same_partition(&result.labels, &truth));
        let mut seen = vec![];
        for &l in &result.labels {
            if !seen.contains(&l) {
                seen.push(l);
            }
        }
        assert_eq!(seen, vec![0, 1, 2], "new speakers appear as 0, 1, 2 in order");
    }

    #[test]
    fn a_known_speaker_count_overrides_the_estimate() {
        let (lines, _) = conversation(&TURNS, 0.15, 4, |_| 3.0);
        assert_eq!(assign_speakers(&lines, &Options::default()).num_speakers, 3);
        assert_eq!(assign_speakers(&lines, &Options { num_speakers: Some(2), ..Options::default() }).num_speakers, 2);
        assert_eq!(assign_speakers(&lines, &Options { num_speakers: Some(1), ..Options::default() }).num_speakers, 1);
        // Asking for more voices than there are lines is capped, never a panic
        assert!(assign_speakers(&lines[..3], &Options { num_speakers: Some(9), ..Options::default() }).num_speakers <= 3);
    }

    #[test]
    fn short_lines_join_the_nearest_voice_and_unmeasurable_ones_follow_their_neighbour() {
        // Every third line is short; two of those could not be measured at all
        let (mut lines, truth) = conversation(&TURNS, 0.12, 5, |i| if i % 3 == 2 { 0.6 } else { 3.0 });
        let unmeasured = [5usize, 11];
        for &i in &unmeasured {
            lines[i].embedding = None;
        }
        let result = assign_speakers(&lines, &Options::default());
        assert_eq!(result.num_speakers, 3);
        for (i, (&got, &want)) in result.labels.iter().zip(&truth).enumerate() {
            if unmeasured.contains(&i) {
                assert_eq!(got, result.labels[i - 1], "unmeasurable line {i} takes the previous speaker");
            } else {
                // compare via the partition of measurable lines
                let j = (0..lines.len()).find(|&j| !unmeasured.contains(&j) && truth[j] == want).unwrap();
                assert_eq!(got, result.labels[j], "line {i}");
            }
        }
    }

    #[test]
    fn one_voice_throughout_is_one_speaker() {
        let (lines, _) = conversation(&[0; 20], 0.25, 6, |_| 2.5);
        let result = assign_speakers(&lines, &Options::default());
        assert_eq!(result.num_speakers, 1);
        assert!(result.labels.iter().all(|&l| l == 0));
    }

    #[test]
    fn nothing_measurable_means_one_speaker_and_empty_input_is_empty() {
        let lines = vec![LineInput { duration: 0.2, embedding: None }; 4];
        assert_eq!(assign_speakers(&lines, &Options::default()), Labeling { labels: vec![0; 4], num_speakers: 1 });
        assert_eq!(assign_speakers(&[], &Options::default()).labels.len(), 0);
    }

    #[test]
    fn only_short_lines_still_get_clustered() {
        let (lines, truth) = conversation(&TURNS, 0.12, 7, |_| 0.5);
        let result = assign_speakers(&lines, &Options::default());
        assert!(same_partition(&result.labels, &truth));
    }

    #[test]
    fn very_long_meetings_are_subsampled_for_clustering_but_every_line_is_labelled() {
        let pattern: Vec<usize> = (0..300).map(|i| (i / 3) % 3).collect();
        let (lines, truth) = conversation(&pattern, 0.12, 8, |_| 2.0);
        let result = assign_speakers(&lines, &Options { max_cluster_items: 40, ..Options::default() });
        assert_eq!(result.labels.len(), 300);
        assert!(same_partition(&result.labels, &truth));
    }
}
