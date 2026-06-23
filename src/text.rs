//! Layout-aware, typo-tolerant text matching.
//!
//! Two problems solved here:
//!  1. **Layout independence** — if you have a Greek layout active and type the
//!     keys for "bottles" you actually get "βοττλεσ". We map characters between
//!     the Greek and Latin keyboard layouts so the query matches either way.
//!  2. **Fuzzy matching** — prefix / substring / subsequence / bounded
//!     Levenshtein, so small typos still match.

/// Standard Greek keyboard layout: (latin key, greek char it produces).
const KEYMAP: &[(char, char)] = &[
    ('a', 'α'),
    ('b', 'β'),
    ('c', 'ψ'),
    ('d', 'δ'),
    ('e', 'ε'),
    ('f', 'φ'),
    ('g', 'γ'),
    ('h', 'η'),
    ('i', 'ι'),
    ('j', 'ξ'),
    ('k', 'κ'),
    ('l', 'λ'),
    ('m', 'μ'),
    ('n', 'ν'),
    ('o', 'ο'),
    ('p', 'π'),
    ('r', 'ρ'),
    ('s', 'σ'),
    ('t', 'τ'),
    ('u', 'θ'),
    ('v', 'ω'),
    ('w', 'ς'),
    ('x', 'χ'),
    ('y', 'υ'),
    ('z', 'ζ'),
];

fn latin_to_greek(s: &str) -> String {
    s.chars()
        .map(|c| {
            KEYMAP
                .iter()
                .find(|(l, _)| *l == c)
                .map(|(_, g)| *g)
                .unwrap_or(c)
        })
        .collect()
}

fn greek_to_latin(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            // Final sigma maps to 's' (more useful than its raw key 'w').
            'ς' => 's',
            _ => KEYMAP
                .iter()
                .find(|(_, g)| *g == c)
                .map(|(l, _)| *l)
                .unwrap_or(c),
        })
        .collect()
}

/// Lowercase and strip Greek accent marks so "Έγγραφα" ~ "εγγραφα".
pub fn normalize(s: &str) -> String {
    s.to_lowercase().chars().map(strip_accent).collect()
}

fn strip_accent(c: char) -> char {
    match c {
        'ά' => 'α',
        'έ' => 'ε',
        'ή' => 'η',
        'ί' | 'ϊ' | 'ΐ' => 'ι',
        'ό' => 'ο',
        'ύ' | 'ϋ' | 'ΰ' => 'υ',
        'ώ' => 'ω',
        _ => c,
    }
}

/// The query plus its cross-layout interpretations.
fn variants(input: &str) -> Vec<String> {
    let mut v = vec![input.to_string()];
    let g2l = greek_to_latin(input);
    if g2l != input {
        v.push(g2l);
    }
    let l2g = latin_to_greek(input);
    if l2g != input {
        v.push(l2g);
    }
    v
}

/// Best relevance of `query` against `candidate` across every layout variant,
/// in [0.0, 1.0]. `None` means "not a match".
pub fn relevance(query: &str, candidate: &str) -> Option<f32> {
    let cand = normalize(candidate);
    let mut best = 0.0f32;
    for v in variants(query) {
        let q = normalize(&v);
        if q.is_empty() {
            continue;
        }
        if let Some(s) = score_one(&q, &cand) {
            if s > best {
                best = s;
            }
        }
    }
    if best >= 0.30 {
        Some(best)
    } else {
        None
    }
}

fn score_one(q: &str, cand: &str) -> Option<f32> {
    if cand.starts_with(q) {
        return Some(1.0);
    }
    let mut best = 0.0f32;
    let mut upd = |v: f32| {
        if v > best {
            best = v;
        }
    };
    if cand.contains(q) {
        upd(0.85);
    }
    let q_len = q.chars().count().max(1);
    for tok in cand.split(|ch: char| !ch.is_alphanumeric()).filter(|t| !t.is_empty()) {
        if tok == q {
            upd(0.95);
        } else if tok.starts_with(q) {
            upd(0.9);
        } else if tok.contains(q) {
            upd(0.72);
        }
        // Fuzzy: edits to turn the query into a *prefix* of this word,
        // normalized by query length (so a long name's tail isn't penalized).
        let sim = 1.0 - prefix_distance(q, tok) as f32 / q_len as f32;
        if sim >= 0.66 {
            upd(sim * 0.85);
        }
        if is_subsequence(q, tok) {
            upd(0.5);
        }
    }
    if is_subsequence(q, cand) {
        upd(0.45);
    }
    if best > 0.0 {
        Some(best)
    } else {
        None
    }
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut hs = haystack.chars();
    'next: for nc in needle.chars() {
        loop {
            match hs.next() {
                Some(hc) if hc == nc => continue 'next,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

/// Minimum edits to transform `q` into *some prefix* of `text`. Matching the
/// empty query to any prefix is free, so unmatched tail characters of `text`
/// cost nothing — ideal for "type the start of a long name" search.
fn prefix_distance(q: &str, text: &str) -> usize {
    let q: Vec<char> = q.chars().collect();
    let t: Vec<char> = text.chars().collect();
    if q.is_empty() {
        return 0;
    }
    // dp[j] = edits to match q[..i] against t[..j]; row 0 (empty q) is all 0.
    let mut prev = vec![0usize; t.len() + 1];
    let mut cur = vec![0usize; t.len() + 1];
    for (i, &cq) in q.iter().enumerate() {
        cur[0] = i + 1;
        for (j, &ct) in t.iter().enumerate() {
            let cost = if cq == ct { 0 } else { 1 };
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    // Best over all prefix lengths j.
    *prev.iter().min().unwrap_or(&q.len())
}

/// Match a query against free text (an app's description/keywords): word-level
/// only — a word must equal, start with, or contain the query — so we avoid the
/// loose whole-string subsequence noise that `relevance` allows. Layout-aware.
pub fn keyword_match(query: &str, text: &str) -> Option<f32> {
    let mut best = 0.0f32;
    for v in variants(query) {
        let q = normalize(&v);
        if q.chars().count() < 2 {
            continue;
        }
        for word in text.split(|c: char| !c.is_alphanumeric()) {
            if word.is_empty() {
                continue;
            }
            let w = normalize(word);
            let s = if w == q {
                1.0
            } else if w.starts_with(&q) {
                0.9
            } else if q.chars().count() >= 3 && w.contains(&q) {
                0.75
            } else {
                0.0
            };
            if s > best {
                best = s;
            }
        }
    }
    if best >= 0.7 {
        Some(best)
    } else {
        None
    }
}

/// Match `name` against a glob with `*` wildcards (case-insensitive), trying
/// the query in its original and cross-layout forms.
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let name = normalize(name);
    for v in variants(pattern) {
        if glob_one(&normalize(&v), &name) {
            return true;
        }
    }
    false
}

fn glob_one(pat: &str, name: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    let last = parts.len() - 1;
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !name[pos..].starts_with(part) {
                return false;
            }
            pos += part.len();
        } else if i == last {
            if !name[pos..].ends_with(part) {
                return false;
            }
        } else {
            match name[pos..].find(part) {
                Some(f) => pos += f + part.len(),
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(query: &str, candidate: &str) -> bool {
        relevance(query, candidate).is_some()
    }

    #[test]
    fn cross_layout_greek_keys_match_latin_name() {
        // Typed on a Greek layout, meaning the Latin word.
        assert!(matches("βοττλεσ", "Bottles"));
        assert!(matches("τηανδερ", "Thunderbird"));
        assert!(matches("φιρεφοχ", "Firefox"));
    }

    #[test]
    fn latin_keys_match_greek_name() {
        // Typed on a Latin layout, meaning a Greek folder name.
        assert!(matches("eggrafa", "Έγγραφα"));
    }

    #[test]
    fn fuzzy_tolerates_typos() {
        assert!(matches("thunderbird", "Thunderbird Mail"));
        assert!(matches("thanderbuid", "Thunderbird"));
        assert!(matches("firfox", "Firefox"));
    }

    #[test]
    fn glob_matches_extension() {
        assert!(glob_match("*.pdf", "master.pdf"));
        assert!(glob_match("master*", "master.pdf"));
        assert!(!glob_match("*.png", "master.pdf"));
    }

    #[test]
    fn unrelated_does_not_match() {
        assert!(!matches("xkcd", "Calculator"));
    }
}
