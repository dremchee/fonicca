//! Instrument categories. File and folder names come first (sample packs are
//! usually well named); the CLAP audio encoder classifies the rest.

use std::sync::LazyLock;

use regex::Regex;

/// Category ids stored in the database, with the CLAP prompt used for each.
/// Order matters for name matching only through `NAME_RULES`.
pub const CATEGORIES: &[(&str, &str)] = &[
    ("kick", "a kick drum"),
    ("snare", "a snare drum"),
    ("clap", "a hand clap"),
    ("hihat", "a hi-hat cymbal"),
    ("cymbal", "a crash or ride cymbal"),
    ("tom", "a tom drum"),
    ("percussion", "percussion like shaker, conga or bongo"),
    ("drums", "a drum beat"),
    ("bass", "a bass sound"),
    ("synth", "a synthesizer lead"),
    ("pad", "an atmospheric synth pad"),
    ("keys", "a piano or keyboard"),
    ("guitar", "a guitar"),
    ("strings", "orchestral strings"),
    ("brass", "brass or woodwind instruments"),
    ("vocal", "a human voice singing or speaking"),
    ("fx", "a sound effect like a riser, sweep or impact"),
];

/// Bump when `NAME_PATTERNS` change, so names are categorized again.
pub const NAME_RULES_VERSION: i64 = 1;

/// Categories where a musical key is meaningful.
pub fn is_tonal(category: &str) -> bool {
    matches!(category, "bass" | "synth" | "pad" | "keys" | "guitar" | "strings" | "brass" | "vocal")
}

/// Keyword patterns per category. Compound terms ("bass drum", "808 kick") are
/// listed with the category they belong to; overlaps are resolved by position.
const NAME_PATTERNS: &[(&str, &str)] = &[
    ("kick", r"kicks?|kik|kck|bd|bass ?drums?|808 ?kicks?"),
    ("snare", r"snares?|snar|snr|sn|sd|rim ?shots?"),
    ("clap", r"claps?|clp|snaps?|finger ?snaps?"),
    ("hihat", r"hi[ -]?hats?|hats?|hh|ohh|chh|open ?hats?|closed ?hats?"),
    ("cymbal", r"cymbals?|cym|crash(es)?|rides?|splash|china"),
    ("tom", r"toms?|floor ?toms?"),
    ("percussion", r"percs?|percz|percusn|percussions?|shakers?|congas?|bongos?|bngo|rims?|tambourines?|tamburine?s?|tamb|cowbells?|claves?|guiros?|timbales?|blocks?|triangles?|cabasa|agogo|djembe|tabla|cajons?|log ?drums?"),
    ("drums", r"drums?|drum ?loops?|breaks?|breakbeats?|beats?|grooves?|tops?|fills?"),
    ("bass", r"bass(es)?|808s?|sub ?bass|subs?|reese"),
    ("keys", r"pianos?|keys|keyboards?|rhodes|organs?|e[ -]?pianos?|wurli(tzer)?|clav(inet)?|harpsichord"),
    ("guitar", r"guitars?|gtr|acoustic ?guitars?|akoustik ?guitars?"),
    ("strings", r"strings?|violins?|cellos?|violas?|orch(estra)?|pizz(icato)?"),
    ("brass", r"brass|horns?|trumpets?|trombones?|sax(ophone)?|flutes?|clarinets?|winds?"),
    ("vocal", r"vox|vocals?|voc|voices?|acapellas?|a ?cappella|chants?|adlibs?|choir"),
    ("pad", r"pads?|atmos(phere)?s?|drones?|ambien(t|ce)|textures?"),
    ("synth", r"synths?|leads?|arps?|arpeggios?|plucks?|stabs?|chords?|bells?"),
    ("fx", r"s?fx|risers?|sweeps?|impacts?|whoosh(es)?|noises?|uplifters?|downlifters?|transitions?|reverse|foley|hits?"),
];

/// Keyword anywhere in a file name. Digits and underscores count as separators,
/// so "Kick01", "BD_Hard" and "hihat2" match.
static NAME_RULES: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    NAME_PATTERNS
        .iter()
        .map(|(c, p)| (*c, Regex::new(&format!(r"(?i)(?:^|[^a-z])({p})(?:$|[^a-z])")).unwrap()))
        .collect()
});

/// Splits run-together words at lower→upper case changes: "ADClosedHat006"
/// → "ADClosed Hat006" → matches "hat".
fn split_camel_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 8);
    let mut prev_lower = false;
    for c in name.chars() {
        if prev_lower && c.is_uppercase() {
            out.push(' ');
        }
        prev_lower = c.is_lowercase();
        out.push(c);
    }
    out
}

/// Generic drum words ("Beats", "Drum Kits", "Breaks") often name whole packs
/// ("Beat Butcha Vol. 2", "Hip-Hop Drums Pack"), so they don't count in folders.
const FOLDER_SKIP: &[&str] = &["drums"];

/// The keyword that appears first in the name wins ("Synth Fill" is a synth,
/// "Kick Loop" a kick); on a tie the longer match ("Bass Drum" over "Bass").
fn from_name(name: &str, skip: &[&str]) -> Option<&'static str> {
    let text = split_camel_case(name);
    NAME_RULES
        .iter()
        .filter(|(c, _)| !skip.contains(c))
        .filter_map(|(c, re)| re.captures(&text).and_then(|cap| cap.get(1)).map(|m| (m.start(), m.len(), *c)))
        .min_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, _, c)| c)
}

/// Category from the file name, falling back to folder names from the
/// innermost outwards. `folders` are the directories between the source root
/// and the file.
pub fn from_names(name: &str, folders: &[&str]) -> Option<&'static str> {
    from_name(name, &[]).or_else(|| folders.iter().rev().find_map(|f| from_name(f, FOLDER_SKIP)))
}

// --- Classification from audio embeddings -------------------------------------

/// Below this many name-labelled examples, fall back to text prototypes.
const MIN_LABELLED: usize = 200;
const NEIGHBOURS: usize = 10;
/// Share of neighbour similarity that must agree; 0.6 gave ~83% precision on
/// a real library while classifying about half of the unnamed samples.
const MIN_SHARE: f32 = 0.6;
/// Zero-shot fallback: softmax over prompt similarities at CLAP's logit scale.
const LOGIT_SCALE: f32 = 100.0;
const MIN_ZERO_SHOT_PROB: f32 = 0.5;

/// Category vectors of the CLAP text encoder for "This is a sound of {prompt}.",
/// computed once offline so the app needs no text model.
static PROTOTYPES: LazyLock<Vec<(&'static str, Vec<f32>)>> = LazyLock::new(|| {
    let map: std::collections::HashMap<String, Vec<f32>> =
        serde_json::from_str(include_str!("../resources/clap_text_prototypes.json")).expect("prototype JSON");
    CATEGORIES.iter().map(|(id, _)| (*id, map[*id].clone())).collect()
});

/// Classifies embeddings by their nearest name-labelled neighbours in the same
/// library, which fits the user's sounds far better than text prompts.
pub struct Classifier {
    labelled: Vec<(&'static str, Vec<f32>)>,
}

impl Classifier {
    pub fn new(labelled: Vec<(String, Vec<f32>)>) -> Self {
        let labelled = labelled
            .into_iter()
            .filter_map(|(c, e)| CATEGORIES.iter().find(|(id, _)| *id == c).map(|(id, _)| (*id, e)))
            .collect();
        Self { labelled }
    }

    pub fn labelled_count(&self) -> usize {
        self.labelled.len()
    }

    /// Returns the category and a confidence in 0..1, or `None` when unsure.
    pub fn classify(&self, embedding: &[f32]) -> Option<(&'static str, f32)> {
        if self.labelled.len() >= MIN_LABELLED { self.nearest(embedding) } else { zero_shot(embedding) }
    }

    fn nearest(&self, embedding: &[f32]) -> Option<(&'static str, f32)> {
        let mut top: Vec<(f32, &'static str)> = Vec::with_capacity(NEIGHBOURS + 1);
        for (c, e) in &self.labelled {
            let sim = crate::clap::dot(embedding, e);
            if top.len() < NEIGHBOURS || sim > top[top.len() - 1].0 {
                let pos = top.partition_point(|(s, _)| *s > sim);
                top.insert(pos, (sim, *c));
                top.truncate(NEIGHBOURS);
            }
        }
        let mut votes: Vec<(&'static str, f32)> = Vec::new();
        for (sim, c) in &top {
            let w = sim.max(0.0);
            match votes.iter_mut().find(|(v, _)| v == c) {
                Some(v) => v.1 += w,
                None => votes.push((*c, w)),
            }
        }
        let total: f32 = votes.iter().map(|(_, w)| w).sum();
        let (best, weight) = votes.into_iter().max_by(|a, b| a.1.total_cmp(&b.1))?;
        let share = if total > 0.0 { weight / total } else { 0.0 };
        (share >= MIN_SHARE).then_some((best, share))
    }
}

fn zero_shot(embedding: &[f32]) -> Option<(&'static str, f32)> {
    let logits: Vec<f32> = PROTOTYPES.iter().map(|(_, p)| crate::clap::dot(embedding, p) * LOGIT_SCALE).collect();
    let max = logits.iter().copied().fold(f32::MIN, f32::max);
    let exp: Vec<f32> = logits.iter().map(|l| (l - max).exp()).collect();
    let sum: f32 = exp.iter().sum();
    let (i, p) = exp.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
    let prob = p / sum;
    (prob >= MIN_ZERO_SHOT_PROB).then_some((PROTOTYPES[i].0, prob))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorizes_by_file_name() {
        for (name, cat) in [
            ("Kick 01", Some("kick")),
            ("BD_Hard_02", Some("kick")),
            ("808 Kick Long", Some("kick")),
            ("KSHMR_Punchy_Kick_05_D#", Some("kick")),
            ("snare roll 01", Some("snare")),
            ("Clap_Tight", Some("clap")),
            ("HiHat2", Some("hihat")),
            ("Open Hat 3", Some("hihat")),
            ("D70 CRASH", Some("cymbal")),
            ("TOM Sequence", Some("tom")),
            ("Shaker Loop 95", Some("percussion")),
            ("KSHMR_Acoustic_Drum_Loop_17_128", Some("drums")),
            ("Breakbeat Fave 5 93bpm", Some("drums")),
            ("P - NNF V2 SUB BASS", Some("bass")),
            ("808 Long C", Some("bass")),
            ("organchordz8", None), // run-together words are left to the model
            ("Rhodes Chords Am", Some("keys")),
            ("L-GUITARLOOP", None),
            ("Guitar16", Some("guitar")),
            ("STRING1", Some("strings")),
            ("Nard Vox loop 88bpm", Some("vocal")),
            ("145_A_FilterPad_01_SP", Some("pad")), // split at the case change
            ("Warm Pad C", Some("pad")),
            ("KSHMR_Synth_Fill_05_128_F", Some("synth")), // first keyword wins
            ("Kick Loop 4", Some("kick")),
            ("Bass Drum 01", Some("kick")),
            ("Oh Yeah", None),
            ("Lead Saw 120", Some("synth")),
            ("Riser 8 bars", Some("fx")),
            ("Barry White - I Found Someone", None),
            ("Basso", None),
        ] {
            assert_eq!(from_names(name, &[]), cat, "{name}");
        }
    }

    fn unit(i: usize) -> Vec<f32> {
        let mut v = vec![0.0; 4];
        v[i] = 1.0;
        v
    }

    #[test]
    fn nearest_neighbours_vote_with_a_confidence_threshold() {
        let mut labelled = Vec::new();
        for _ in 0..150 {
            labelled.push(("kick".to_string(), unit(0)));
            labelled.push(("snare".to_string(), unit(1)));
        }
        let c = Classifier::new(labelled);
        assert_eq!(c.classify(&[0.9, 0.1, 0.0, 0.0]).map(|r| r.0), Some("kick"));
        // Equally close to both: no confident answer.
        assert_eq!(c.classify(&[0.7, 0.7, 0.0, 0.0]), None);
    }

    #[test]
    fn prototypes_cover_every_category() {
        assert_eq!(PROTOTYPES.len(), CATEGORIES.len());
        assert!(PROTOTYPES.iter().all(|(_, v)| v.len() == crate::clap::DIM));
    }

    #[test]
    fn falls_back_to_innermost_folder() {
        assert_eq!(from_names("01", &["Trap Pack", "Kicks"]), Some("kick"));
        assert_eq!(from_names("01", &["Drums", "Snares"]), Some("snare"));
        assert_eq!(from_names("Hat 1", &["Kicks"]), Some("hihat"));
        assert_eq!(from_names("01", &["Misc"]), None);
        assert_eq!(from_names("01", &["05 - Hi Hats"]), Some("hihat"));
        assert_eq!(from_names("BG_BASSLINE5", &["URBAN WARFARE - WAV", "RUFF BASS 1"]), Some("bass"));
        assert_eq!(from_names("ADClosedHat006", &[]), Some("hihat"));
        assert_eq!(from_names("Crunch7", &["Dr. Dre Drum Kit Vol 3"]), None);
        assert_eq!(from_names("Sample 3", &["Beat Butcha - Steroid Pack Vol. 2"]), None);
    }
}

#[cfg(test)]
mod report {
    /// Name-based coverage on a real library:
    /// `LIBRARY_DB=…/library.db cargo test -- --ignored --nocapture name_coverage`
    #[test]
    #[ignore]
    fn name_coverage() {
        let conn = rusqlite::Connection::open(std::env::var("LIBRARY_DB").unwrap()).unwrap();
        let mut stmt = conn
            .prepare("SELECT s.name, substr(s.path, length(src.path) + 2), s.path FROM samples s JOIN sources src ON src.id = s.source_id")
            .unwrap();
        let rows: Vec<(String, String, String)> =
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect();
        let mut labels = serde_json::Map::new();
        let mut counts = std::collections::BTreeMap::new();
        let mut unmatched = Vec::new();
        for (name, rel, path) in &rows {
            let folders: Vec<&str> = rel.split('/').collect();
            let folders = &folders[..folders.len().saturating_sub(1)];
            match super::from_names(name, folders) {
                Some(c) => {
                    *counts.entry(c).or_insert(0) += 1;
                    labels.insert(path.clone(), c.into());
                }
                None => unmatched.push(rel.clone()),
            }
        }
        println!("{} samples, {} categorized by name", rows.len(), rows.len() - unmatched.len());
        // For the Python zero-shot check of the CLAP model.
        if let Ok(dir) = std::env::var("LABELS_OUT") {
            std::fs::write(format!("{dir}/labels.json"), serde_json::to_string(&labels).unwrap()).unwrap();
            std::fs::write(format!("{dir}/categories.json"), serde_json::to_string(super::CATEGORIES).unwrap()).unwrap();
        }
        for (c, n) in counts {
            println!("  {c:<11} {n}");
        }
        unmatched.sort();
        for u in unmatched.iter().step_by((unmatched.len() / 25).max(1)) {
            println!("  ? {u}");
        }
    }
}
