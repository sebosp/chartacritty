//! Repeated text emphasis.
//!
//! Commands like `ping` print the same text over and over, i.e. "64 bytes from 1.1.1.1:
//! icmp_seq=3 ttl=57 time=12.3 ms", where only a few pieces change between lines. This module
//! finds visible lines that are similar to each other and classifies their pieces of text as
//! repeated, to be dimmed more the more often they repeat, or unique, to be slightly highlighted.

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

/// Minimum number of tokens a line needs to be compared with other lines.
const MIN_TOKENS: usize = 3;

/// Minimum Dice similarity, over the tokens, for two lines to be considered repetitions.
const MIN_SIMILARITY: f32 = 0.5;

/// Number of other similar lines needed for a line to be considered repeated.
const MIN_PEERS: usize = 2;

/// Fraction of the similar lines that must share a token for it to be considered repeated.
const SHARED_FRACTION: f32 = 0.5;

/// How much the text is dimmed towards the background, for text repeated in a few lines and for
/// text repeated in `SATURATION_LINES` or more lines.
const MIN_DIM: f32 = 0.3;
const MAX_DIM: f32 = 0.65;
const SATURATION_LINES: usize = 10;

/// Configuration of the repeated text emphasis.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(default)]
pub struct RepeatedTextConfig {
    /// Whether repeated text is dimmed and the unique parts of it highlighted.
    pub enabled: bool,
}

impl Default for RepeatedTextConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// How a cell of a repeated line should be shown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Emphasis {
    /// Text shared with other similar lines.
    Repeated {
        /// How much to dim the text, in `0..1`.
        dim: f32,
        /// Identifies the repeated pattern, stable across frames, i.e. to pick its color.
        pattern: u64,
    },
    /// Text that differs from the other similar lines.
    Unique,
}

/// A run of repeated text of a line, from the first to the last of consecutive repeated tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PatternSpan {
    /// Viewport line.
    pub line: usize,
    /// First column of the span.
    pub start: usize,
    /// Column after the end of the span.
    pub end: usize,
    pub pattern: u64,
}

/// The result of analyzing the visible lines.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Analysis {
    /// For every line, the emphasis of every column, `None` for columns that are left as is.
    pub emphasis: Vec<Vec<Option<Emphasis>>>,
    /// The runs of repeated text, in order of lines and columns.
    pub spans: Vec<PatternSpan>,
}

/// Finds the representative of the group of `line`, compressing the path to it.
fn find(groups: &mut [usize], mut line: usize) -> usize {
    while groups[line] != line {
        groups[line] = groups[groups[line]];
        line = groups[line];
    }
    line
}

/// A token of a line: a run of alphanumeric characters or a single other non-space character.
#[derive(Debug)]
struct Token {
    start: usize,
    end: usize,
    /// Interned text of the token, tokens with the same text have the same id.
    id: u32,
}

/// The tokens of a line and a hash of their texts, identifying the line across frames.
#[derive(Debug)]
struct LineTokens {
    tokens: Vec<Token>,
    hash: u64,
}

fn tokenize(line: &[char], interner: &mut HashMap<String, u32>) -> LineTokens {
    let mut texts: Vec<(usize, usize, String)> = vec![];
    for (column, &c) in line.iter().enumerate() {
        if c.is_whitespace() {
            continue;
        }
        match texts.last_mut() {
            // Extend the alphanumeric run, i.e. "bytes" or "1234".
            Some((_, end, text))
                if c.is_alphanumeric()
                    && *end == column
                    && text.chars().all(char::is_alphanumeric) =>
            {
                text.push(c);
                *end = column + 1;
            },
            _ => texts.push((column, column + 1, c.to_string())),
        }
    }

    let mut hasher = DefaultHasher::new();
    let tokens = texts
        .into_iter()
        .map(|(start, end, text)| {
            text.hash(&mut hasher);
            let next_id = interner.len() as u32;
            let id = *interner.entry(text).or_insert(next_id);
            Token { start, end, id }
        })
        .collect();
    LineTokens { tokens, hash: hasher.finish() }
}

/// The longest common subsequence of the tokens of two lines.
#[derive(Debug, Clone)]
struct PairMatch {
    common: usize,
    /// Which tokens of each line are part of the common subsequence.
    a: Vec<bool>,
    b: Vec<bool>,
}

fn lcs_matches(a: &[Token], b: &[Token]) -> PairMatch {
    let (n, m) = (a.len(), b.len());
    // lengths[i][j] is the LCS length of a[i..] and b[j..].
    let mut lengths = vec![0u16; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lengths[idx(i, j)] = if a[i].id == b[j].id {
                lengths[idx(i + 1, j + 1)] + 1
            } else {
                lengths[idx(i + 1, j)].max(lengths[idx(i, j + 1)])
            };
        }
    }

    let mut res =
        PairMatch { common: lengths[idx(0, 0)] as usize, a: vec![false; n], b: vec![false; m] };
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i].id == b[j].id {
            res.a[i] = true;
            res.b[j] = true;
            i += 1;
            j += 1;
        } else if lengths[idx(i + 1, j)] >= lengths[idx(i, j + 1)] {
            i += 1;
        } else {
            j += 1;
        }
    }
    res
}

/// Token matches between pairs of lines, keyed by the hashes of both lines, smallest first.
type PairMemo = HashMap<(u64, u64), PairMatch>;

/// Dim amount for text shared by `lines` lines.
fn dim_for(lines: usize) -> f32 {
    let saturation =
        (lines.saturating_sub(MIN_PEERS + 1)) as f32 / (SATURATION_LINES - (MIN_PEERS + 1)) as f32;
    MIN_DIM + (MAX_DIM - MIN_DIM) * saturation.min(1.)
}

/// Classifies the text of the visible `lines`, each made of one character per column.
///
/// Lines similar to at least `MIN_PEERS` other lines form a pattern, the text they share is
/// repeated and the rest unique. The first, topmost, line of every pattern is left as is.
pub fn analyze(lines: &[Vec<char>]) -> Analysis {
    analyze_with_memo(lines, &mut PairMemo::new())
}

/// Same as `analyze`, re-using the matches of line pairs from previous calls in `memo`, since most
/// lines are still visible, maybe scrolled, from one frame to the next.
fn analyze_with_memo(lines: &[Vec<char>], memo: &mut PairMemo) -> Analysis {
    let mut interner = HashMap::new();
    let lines_tokens: Vec<LineTokens> =
        lines.iter().map(|line| tokenize(line, &mut interner)).collect();
    // Token ids are only valid for this call, the memo only keeps positions within the lines.
    let mut texts = vec![String::new(); interner.len()];
    for (text, id) in interner {
        texts[id as usize] = text;
    }

    // For every line, the number of similar lines and, for every token, how many of them share it.
    let mut peers = vec![0usize; lines.len()];
    let mut shared: Vec<Vec<usize>> =
        lines_tokens.iter().map(|line| vec![0; line.tokens.len()]).collect();
    // Whether a similar line is above the line, otherwise it is the first instance of a pattern.
    let mut has_earlier_peer = vec![false; lines.len()];
    // Similar lines are grouped together into patterns.
    let mut groups: Vec<usize> = (0..lines.len()).collect();
    let mut used = HashSet::new();
    for i in 0..lines_tokens.len() {
        let (a, ha) = (&lines_tokens[i].tokens, lines_tokens[i].hash);
        if a.len() < MIN_TOKENS {
            continue;
        }
        for j in i + 1..lines_tokens.len() {
            let (b, hb) = (&lines_tokens[j].tokens, lines_tokens[j].hash);
            if b.len() < MIN_TOKENS {
                continue;
            }
            // Even if all the tokens of the shortest line matched, they would not be similar.
            let total = (a.len() + b.len()) as f32;
            if 2. * a.len().min(b.len()) as f32 / total < MIN_SIMILARITY {
                continue;
            }

            let identical;
            let (common, mask_a, mask_b) = if ha == hb && a.len() == b.len() {
                identical = vec![true; a.len()];
                (a.len(), &identical, &identical)
            } else {
                let key = (ha.min(hb), ha.max(hb));
                used.insert(key);
                let (first, second) = if ha <= hb { (a, b) } else { (b, a) };
                let pair = memo.entry(key).or_insert_with(|| lcs_matches(first, second));
                // Recompute on the unlikely hash collision of different lines.
                if pair.a.len() != first.len() || pair.b.len() != second.len() {
                    *pair = lcs_matches(first, second);
                }
                if ha <= hb {
                    (pair.common, &pair.a, &pair.b)
                } else {
                    (pair.common, &pair.b, &pair.a)
                }
            };
            if 2. * common as f32 / total < MIN_SIMILARITY {
                continue;
            }

            peers[i] += 1;
            peers[j] += 1;
            has_earlier_peer[j] = true;
            let (root_i, root_j) = (find(&mut groups, i), find(&mut groups, j));
            groups[root_j.max(root_i)] = root_j.min(root_i);
            for (count, &matched) in shared[i].iter_mut().zip(mask_a) {
                *count += matched as usize;
            }
            for (count, &matched) in shared[j].iter_mut().zip(mask_b) {
                *count += matched as usize;
            }
        }
    }
    memo.retain(|key, _| used.contains(key));

    let is_repeated = |i: usize, count: usize| count as f32 >= peers[i] as f32 * SHARED_FRACTION;

    // The pattern of a group is identified by the text its lines repeat, the most common one
    // amongst its lines, so that it remains the same while the lines scroll.
    let mut signatures: HashMap<usize, HashMap<u64, usize>> = HashMap::new();
    for (i, line_tokens) in lines_tokens.iter().enumerate() {
        if peers[i] < MIN_PEERS {
            continue;
        }
        let mut hasher = DefaultHasher::new();
        for (token, &count) in line_tokens.tokens.iter().zip(&shared[i]) {
            if is_repeated(i, count) {
                texts[token.id as usize].hash(&mut hasher);
            }
        }
        let root = find(&mut groups, i);
        *signatures.entry(root).or_default().entry(hasher.finish()).or_default() += 1;
    }
    let patterns: HashMap<usize, u64> = signatures
        .into_iter()
        .map(|(root, counts)| {
            let pattern = counts
                .into_iter()
                .max_by_key(|&(signature, count)| (count, std::cmp::Reverse(signature)))
                .map(|(signature, _)| signature)
                .unwrap_or_default();
            (root, pattern)
        })
        .collect();

    let mut res = Analysis {
        emphasis: lines.iter().map(|line| vec![None; line.len()]).collect(),
        spans: vec![],
    };
    for (i, line_tokens) in lines_tokens.iter().enumerate() {
        if peers[i] < MIN_PEERS || !has_earlier_peer[i] {
            continue;
        }
        let pattern = patterns[&find(&mut groups, i)];
        let mut span: Option<PatternSpan> = None;
        for (token, &count) in line_tokens.tokens.iter().zip(&shared[i]) {
            let emphasis = if is_repeated(i, count) {
                // The line itself also contains the token.
                let span = span.get_or_insert(PatternSpan {
                    line: i,
                    start: token.start,
                    end: token.end,
                    pattern,
                });
                span.end = token.end;
                Emphasis::Repeated { dim: dim_for(count + 1), pattern }
            } else {
                res.spans.extend(span.take());
                Emphasis::Unique
            };
            res.emphasis[i][token.start..token.end].fill(Some(emphasis));
        }
        res.spans.extend(span);
    }
    res
}

/// Caches the analysis of the visible lines, since they rarely change between frames.
#[derive(Debug, Default)]
pub struct RepeatedText {
    hash: Option<u64>,
    analysis: Analysis,
    memo: PairMemo,
}

impl RepeatedText {
    /// Analyzes the `lines`, returns whether the emphasis changed since the last call.
    pub fn update(&mut self, lines: &[Vec<char>]) -> bool {
        let mut hasher = DefaultHasher::new();
        lines.hash(&mut hasher);
        let hash = hasher.finish();
        if self.hash == Some(hash) {
            return false;
        }
        self.hash = Some(hash);
        let analysis = analyze_with_memo(lines, &mut self.memo);
        let changed = analysis != self.analysis;
        self.analysis = analysis;
        changed
    }

    /// Emphasis of the cell at the viewport `line` and `column`.
    pub fn get(&self, line: usize, column: usize) -> Option<Emphasis> {
        self.analysis.emphasis.get(line).and_then(|line| line.get(column).copied().flatten())
    }

    /// The runs of repeated text of the last analysis.
    pub fn spans(&self) -> &[PatternSpan] {
        &self.analysis.spans
    }

    /// Forgets the analysis, i.e. when the feature is disabled.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &[&str]) -> Vec<Vec<char>> {
        text.iter().map(|line| line.chars().collect()).collect()
    }

    /// Text of the line with the given emphasis.
    fn text_with(line: &[char], emphasis: &[Option<Emphasis>], unique: bool) -> String {
        line.iter()
            .zip(emphasis)
            .map(|(&c, e)| match e {
                Some(Emphasis::Unique) if unique => c,
                Some(Emphasis::Repeated { .. }) if !unique => c,
                _ => ' ',
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn ping_unique_parts() {
        let lines = lines(&[
            "PING 1.1.1.1 (1.1.1.1) 56(84) bytes of data.",
            "64 bytes from 1.1.1.1: icmp_seq=1 ttl=57 time=12.3 ms",
            "64 bytes from 1.1.1.1: icmp_seq=2 ttl=57 time=11.8 ms",
            "64 bytes from 1.1.1.1: icmp_seq=3 ttl=57 time=13.1 ms",
            "64 bytes from 1.1.1.1: icmp_seq=4 ttl=57 time=12.9 ms",
            "64 bytes from 1.1.1.1: icmp_seq=10 ttl=57 time=9.42 ms",
            "$ ls",
        ]);
        let res = analyze(&lines);

        // The header and the prompt are not repeated, the first reply is the first instance.
        for line in [0, 1, 6] {
            assert!(res.emphasis[line].iter().all(Option::is_none), "line {line}");
        }

        assert_eq!(text_with(&lines[2], &res.emphasis[2], true), "2 11 8");
        assert_eq!(text_with(&lines[5], &res.emphasis[5], true), "10 9 42");
        assert_eq!(
            text_with(&lines[2], &res.emphasis[2], false),
            "64 bytes from 1.1.1.1: icmp_seq= ttl=57 time= . ms"
        );

        // Shared by all the five lines, the same pattern on all of them.
        let Some(Emphasis::Repeated { dim, pattern }) = res.emphasis[2][3] else {
            panic!("{:?}", res.emphasis[2][3])
        };
        assert_eq!(dim, dim_for(5));
        assert!(res.spans.iter().all(|span| span.pattern == pattern));

        // The spans are interrupted by the unique parts.
        let spans: Vec<String> = res
            .spans
            .iter()
            .filter(|span| span.line == 2)
            .map(|span| lines[2][span.start..span.end].iter().collect())
            .collect();
        assert_eq!(spans, ["64 bytes from 1.1.1.1: icmp_seq=", "ttl=57 time=", ".", "ms"]);
    }

    #[test]
    fn patterns_have_their_own_id() {
        let mut text = vec![];
        for i in 0..4 {
            text.push(format!("Compiling crate{i} v0.1.{i}"));
            text.push(format!("warning: unused variable `x{i}` in module"));
        }
        let lines = lines(&text.iter().map(String::as_str).collect::<Vec<_>>());
        let res = analyze(&lines);
        let pattern = |line: usize| match res.emphasis[line][0] {
            Some(Emphasis::Repeated { pattern, .. }) => pattern,
            other => panic!("line {line}: {other:?}"),
        };
        // The first line of each pattern is left as is.
        assert!(res.emphasis[0].iter().all(Option::is_none));
        assert!(res.emphasis[1].iter().all(Option::is_none));
        assert_eq!(pattern(2), pattern(4));
        assert_eq!(pattern(3), pattern(5));
        assert_ne!(pattern(2), pattern(3));

        // The pattern stays the same when scrolling.
        let scrolled = analyze(&lines[2..]);
        let Some(Emphasis::Repeated { pattern: scrolled, .. }) = scrolled.emphasis[2][0] else {
            panic!("expected repeated text");
        };
        assert_eq!(scrolled, pattern(4));
    }

    #[test]
    fn more_repetitions_are_dimmer() {
        let few: Vec<String> = (0..3).map(|i| format!("building crate number {i}")).collect();
        let many: Vec<String> = (0..12).map(|i| format!("building crate number {i}")).collect();
        let few = analyze(&lines(&few.iter().map(String::as_str).collect::<Vec<_>>()));
        let many = analyze(&lines(&many.iter().map(String::as_str).collect::<Vec<_>>()));
        let (Some(Emphasis::Repeated { dim: few, .. }), Some(Emphasis::Repeated { dim: many, .. })) =
            (few.emphasis[1][0], many.emphasis[1][0])
        else {
            panic!("expected repeated text");
        };
        assert!(few < many, "{few} < {many}");
        assert_eq!(many, MAX_DIM);
    }

    #[test]
    fn identical_lines_are_fully_dimmed() {
        let lines = lines(&[
            "warning: unused variable",
            "warning: unused variable",
            "warning: unused variable",
        ]);
        let res = analyze(&lines);
        assert!(res.emphasis[0].iter().all(Option::is_none));
        assert_eq!(text_with(&lines[1], &res.emphasis[1], false), "warning: unused variable");
        assert_eq!(text_with(&lines[1], &res.emphasis[1], true), "");
        assert_eq!(res.spans, [
            PatternSpan { line: 1, start: 0, end: 24, pattern: res.spans[0].pattern },
            PatternSpan { line: 2, start: 0, end: 24, pattern: res.spans[0].pattern },
        ]);
    }

    #[test]
    fn cache_reports_changes() {
        let mut repeated = RepeatedText::default();
        let a = lines(&["a b c 1", "a b c 2", "a b c 3"]);
        assert!(repeated.update(&a));
        assert!(!repeated.update(&a));
        assert_eq!(repeated.get(0, 6), None);
        assert_eq!(repeated.get(1, 6), Some(Emphasis::Unique));
        assert!(matches!(repeated.get(1, 0), Some(Emphasis::Repeated { .. })));
        assert_eq!(repeated.get(9, 0), None);
        assert_eq!(repeated.spans().len(), 2);
    }
}

#[cfg(test)]
mod bench {
    use super::*;

    #[test]
    #[ignore]
    fn full_screen_timing() {
        let lines: Vec<Vec<char>> = (0..80)
            .map(|i| {
                format!(
                    "64 bytes from 1.1.1.1: icmp_seq={i} ttl=57 time={}.{} ms {:<120}",
                    i % 17,
                    i % 7,
                    "x"
                )
                .chars()
                .take(200)
                .collect()
            })
            .collect();
        let start = std::time::Instant::now();
        let res = analyze(&lines);
        eprintln!("analyze 80x200: {:?}", start.elapsed());

        // Scroll by one line, as a new line of output arrives.
        let mut memo = PairMemo::new();
        analyze_with_memo(&lines, &mut memo);
        let mut scrolled = lines[1..].to_vec();
        scrolled.push("64 bytes from 1.1.1.1: icmp_seq=80 ttl=57 time=1.2 ms".chars().collect());
        let start = std::time::Instant::now();
        analyze_with_memo(&scrolled, &mut memo);
        eprintln!("analyze 80x200 scrolled with memo: {:?}", start.elapsed());
        assert!(res.emphasis.iter().any(|l| l.iter().any(Option::is_some)));
    }
}
