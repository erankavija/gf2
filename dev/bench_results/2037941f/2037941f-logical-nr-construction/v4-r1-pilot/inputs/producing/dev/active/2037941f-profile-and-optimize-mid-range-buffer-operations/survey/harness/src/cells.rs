//! The frozen cell table of addendum `2037941f-logical-buffer-v1`.
//!
//! [`cells`] returns every cell the addendum declares, in the addendum's own
//! order: the four questions in document order, and within each question the
//! normative primary Cartesian product followed by the exact exploratory
//! additions in listed order. Ordinal zero carries the campaign seed; ordinal
//! `i` carries the `i`-th subsequent output of one canonical SplitMix64 stream
//! started at the campaign seed.

use tuning_campaign_support::abtest::SplitMix64;

/// Repository-relative path of the frozen prose addendum this table transcribes.
pub const ADDENDUM_PATH: &str =
    "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/logical-buffer-addendum.md";
/// Content identity of that frozen document, verified before a timed run.
pub const ADDENDUM_SHA256: &str =
    "290685bf5ab2837e9a38602acead3d33005c2c14b8b883a30bcf863d8c2b7074";
/// Addendum identity string the frozen document declares.
pub const ADDENDUM_IDENTITY: &str = "2037941f-logical-buffer-v1";
/// Freeze time the frozen document declares.
pub const ADDENDUM_FROZEN_UTC: &str = "2026-09-15T03:56:39Z";
/// Repository-relative path of the qualified ISA-L comparator specification.
pub const COMPARATOR_PATH: &str =
    "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/isal-comparator.md";

/// Campaign seed of the frozen addendum.
pub const CAMPAIGN_SEED: u64 = 0x2037_941f_3ea1_22df;
/// Anchor word counts; these cells form the confirmatory families.
pub const ANCHOR_WORDS: [usize; 5] = [8, 9, 63, 64, 65];
/// Neighbouring word counts bounding the cutover and the story's upper edge.
pub const NEIGHBOR_WORDS: [usize; 2] = [7, 66];
/// Every word count the oracle exercises.
pub const ALL_WORDS: [usize; 7] = [7, 8, 9, 63, 64, 65, 66];
/// Smallest row stride of the story's declared mid-range band, in words.
pub const MID_RANGE_MIN_WORDS: usize = 8;
/// Largest row stride of that band.
pub const MID_RANGE_MAX_WORDS: usize = 64;
/// Logical bit lengths the oracle exercises at the word boundary.
pub const BOUNDARY_BITS: [usize; 5] = [0, 1, 63, 64, 65];
/// Rows in every public row-XOR matrix.
pub const ROW_MATRIX_ROWS: usize = 64;
/// The frozen directed `(dst, src)` row-pair cycle.
pub const ROW_PAIRS: [(usize, usize); 8] = [
    (1, 0),
    (0, 1),
    (17, 16),
    (16, 17),
    (33, 32),
    (32, 33),
    (49, 48),
    (48, 49),
];
/// Smallest byte size of one streaming fixture bank.
pub const STREAMING_BANK_BYTES: usize = 8 << 20;
/// Fixture banks in a streaming working set; the canonical timing rotation.
pub const STREAMING_BANKS: usize = tuning_campaign_support::timing::FIXTURE_BANKS;

/// One of the addendum's four canonical questions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Question {
    /// Isolated public `xor_inplace`.
    IsolatedXor,
    /// Public `BitMatrix::row_xor`.
    PublicRowXor,
    /// Whole `QuasiCyclicLdpc::nr_5g_rate_matched` constructor.
    NrConstruction,
    /// ISA-L scalar `xor_gen_base` gap.
    IsalBaseGap,
}

impl Question {
    /// Every question in the addendum's document order.
    pub const ALL: [Self; 4] = [
        Self::IsolatedXor,
        Self::PublicRowXor,
        Self::NrConstruction,
        Self::IsalBaseGap,
    ];

    /// The addendum's name for this question.
    pub fn id(self) -> &'static str {
        match self {
            Self::IsolatedXor => "isolated-xor",
            Self::PublicRowXor => "public-row-xor",
            Self::NrConstruction => "nr-bg2-construction",
            Self::IsalBaseGap => "isal-base-gap",
        }
    }

    /// The family identity that owns this question's ledger.
    pub fn family_id(self) -> &'static str {
        match self {
            Self::IsolatedXor => "2037941f-logical-isolated-xor",
            Self::PublicRowXor => "2037941f-logical-public-row-xor",
            Self::NrConstruction => "2037941f-logical-nr-construction",
            Self::IsalBaseGap => "2037941f-logical-isal-base-gap",
        }
    }

    /// The canonical append-only family ledger.
    pub fn ledger_path(self) -> &'static str {
        match self {
            Self::IsolatedXor => "dev/bench_results/2037941f/logical-isolated-xor-ledger.jsonl",
            Self::PublicRowXor => "dev/bench_results/2037941f/logical-public-row-xor-ledger.jsonl",
            Self::NrConstruction => {
                "dev/bench_results/2037941f/logical-nr-construction-ledger.jsonl"
            }
            Self::IsalBaseGap => "dev/bench_results/2037941f/logical-isal-base-gap-ledger.jsonl",
        }
    }

    /// Resolves a family identity back to its question.
    pub fn from_family_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|q| q.family_id() == id)
    }
}

/// Address layout of an isolated-XOR or ISA-L fixture view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Every source and destination begins at address `0 (mod 64)`.
    A64,
    /// Both views begin eight bytes after separate 64-byte-aligned slab bases.
    O8,
}

impl Layout {
    /// The addendum's name for this layout.
    pub fn id(self) -> &'static str {
        match self {
            Self::A64 => "a64",
            Self::O8 => "o8",
        }
    }

    /// Byte offset of a view from its 64-byte-aligned slab base.
    pub fn offset_bytes(self) -> usize {
        match self {
            Self::A64 => 0,
            Self::O8 => 8,
        }
    }
}

/// Column shape of a public row-XOR matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowShape {
    /// Exactly `64 * words` logical columns.
    Full,
    /// `64 * (words - 1) + 63` logical columns and one zero padding bit.
    Tail63,
}

impl RowShape {
    /// The addendum's name for this shape.
    pub fn id(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Tail63 => "tail63",
        }
    }

    /// Logical column count for `words` words in this shape.
    pub fn columns(self, words: usize) -> usize {
        match self {
            Self::Full => 64 * words,
            Self::Tail63 => 64 * (words - 1) + 63,
        }
    }
}

/// Cache state of a cell, exactly as the addendum defines it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cache {
    /// One untimed pass of the measured operation over the working set.
    Warm,
    /// Eight banks of at least 8 MiB, rotated once per operation.
    Streaming,
    /// A fresh child, a fixed one-call window, no measured warmup.
    Cold,
}

impl Cache {
    /// The protocol's name for this state.
    pub fn id(self) -> &'static str {
        match self {
            Self::Warm => "warm",
            Self::Streaming => "streaming",
            Self::Cold => "cold",
        }
    }

    /// Fixture banks in this state's working set.
    pub fn banks(self) -> usize {
        match self {
            Self::Warm | Self::Cold => 1,
            Self::Streaming => STREAMING_BANKS,
        }
    }

    /// Frozen fixed call count, or `None` when the cell calibrates.
    pub fn cold_calls(self) -> Option<u64> {
        match self {
            Self::Cold => Some(1),
            Self::Warm | Self::Streaming => None,
        }
    }

    /// Decodes a request's declared cache state.
    pub fn from_request(state: &str) -> Result<Self, String> {
        match state {
            "warm" => Ok(Self::Warm),
            "streaming" => Ok(Self::Streaming),
            "cold" => Ok(Self::Cold),
            other => Err(format!("cache state {other:?} is not a protocol state")),
        }
    }
}

/// One selected NR construction target from the addendum's route table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NrTarget {
    /// Cell-identifier suffix.
    pub suffix: &'static str,
    /// Public constructor base graph.
    pub base_graph: u8,
    /// Public constructor target codeword length.
    pub target_n: usize,
    /// Public constructor target message length.
    pub target_k: usize,
    /// Lifting factor the public constructor must select.
    pub lifting_factor: usize,
    /// Dense parity-check rows the constructor must produce.
    pub dense_rows: usize,
    /// Dense parity-check columns the constructor must produce.
    pub dense_cols: usize,
    /// Dense row stride in words.
    pub stride_words: usize,
    /// True for the primary improvement route, false for a non-regression control.
    pub primary: bool,
}

/// The complete selected-route table of the frozen addendum.
pub const NR_TARGETS: [NrTarget; 5] = [
    NrTarget {
        suffix: "bg2-256-49-z9-8w",
        base_graph: 2,
        target_n: 256,
        target_k: 49,
        lifting_factor: 9,
        dense_rows: 378,
        dense_cols: 468,
        stride_words: 8,
        primary: true,
    },
    NrTarget {
        suffix: "bg2-256-121-z22-18w",
        base_graph: 2,
        target_n: 256,
        target_k: 121,
        lifting_factor: 22,
        dense_rows: 924,
        dense_cols: 1144,
        stride_words: 18,
        primary: false,
    },
    NrTarget {
        suffix: "bg2-625-225-z30-25w",
        base_graph: 2,
        target_n: 625,
        target_k: 225,
        lifting_factor: 30,
        dense_rows: 1260,
        dense_cols: 1560,
        stride_words: 25,
        primary: false,
    },
    NrTarget {
        suffix: "bg1-1024-640-z30-32w",
        base_graph: 1,
        target_n: 1024,
        target_k: 640,
        lifting_factor: 30,
        dense_rows: 1380,
        dense_cols: 2040,
        stride_words: 32,
        primary: false,
    },
    NrTarget {
        suffix: "bg2-1024-441-z56-46w",
        base_graph: 2,
        target_n: 1024,
        target_k: 441,
        lifting_factor: 56,
        dense_rows: 2352,
        dense_cols: 2912,
        stride_words: 46,
        primary: false,
    },
];

/// Workload of one cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Workload {
    /// Isolated in-place XOR of `words` words at `layout`.
    Xor {
        /// Words consumed by one logical operation.
        words: usize,
        /// Address layout of both views.
        layout: Layout,
    },
    /// Public row XOR over a 64-row matrix.
    RowXor {
        /// Row stride in words.
        words: usize,
        /// Column shape.
        shape: RowShape,
    },
    /// Whole public NR constructor.
    Nr(NrTarget),
    /// Operation-equivalent ISA-L scalar gap over `words` words.
    IsalGap {
        /// Words consumed by one logical operation.
        words: usize,
    },
}

impl Workload {
    /// Words one logical operation consumes, for every buffer workload.
    pub fn words(self) -> Option<usize> {
        match self {
            Self::Xor { words, .. } | Self::RowXor { words, .. } | Self::IsalGap { words } => {
                Some(words)
            }
            Self::Nr(_) => None,
        }
    }
}

/// One declared cell of the frozen addendum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Position in the addendum's single global cell order.
    pub ordinal: usize,
    /// Owning canonical question.
    pub question: Question,
    /// Frozen cell identifier.
    pub cell_id: String,
    /// Frozen workload.
    pub workload: Workload,
    /// Frozen cache state.
    pub cache: Cache,
    /// True when the cell belongs to its question's confirmatory family.
    pub anchor: bool,
    /// Workload seed derived from the campaign seed stream.
    pub seed: u64,
}

impl Cell {
    /// Whole-consumer cells charge conversion costs; kernel cells do not.
    pub fn whole_consumer(&self) -> bool {
        matches!(self.workload, Workload::Nr(_))
    }

    /// The addendum's objective for this cell.
    pub fn objective(&self) -> &'static str {
        match self.workload {
            Workload::IsalGap { .. } => "comparator-gap",
            Workload::Nr(target) if !target.primary => "non-regression",
            _ => "improvement",
        }
    }

    /// Workload identity recorded in the campaign addendum.
    pub fn workload_identity(&self) -> String {
        format!("{}-{}", self.question.id(), self.cell_id)
    }
}

fn spec(
    question: Question,
    cell_id: String,
    workload: Workload,
    cache: Cache,
    anchor: bool,
) -> Cell {
    Cell {
        ordinal: 0,
        question,
        cell_id,
        workload,
        cache,
        anchor,
        seed: 0,
    }
}

/// The complete frozen cell table, ordinal-ordered and seeded.
///
/// The table is a pure function of the frozen addendum: it reads no file, no
/// environment and no clock, so two invocations in any process produce equal
/// vectors.
pub fn cells() -> Vec<Cell> {
    let mut table = Vec::new();

    // Question 1: isolated XOR. Primary product, then the exact exploratory
    // additions in the addendum's listed order.
    let xor = |words: usize, layout: Layout, cache: Cache, anchor: bool| {
        spec(
            Question::IsolatedXor,
            format!("xor-{words}w-{}-{}", layout.id(), cache.id()),
            Workload::Xor { words, layout },
            cache,
            anchor,
        )
    };
    table.extend(ANCHOR_WORDS.map(|w| xor(w, Layout::A64, Cache::Warm, true)));
    table.extend(NEIGHBOR_WORDS.map(|w| xor(w, Layout::A64, Cache::Warm, false)));
    table.extend(ANCHOR_WORDS.map(|w| xor(w, Layout::O8, Cache::Warm, false)));
    table.extend(ANCHOR_WORDS.map(|w| xor(w, Layout::A64, Cache::Streaming, false)));

    // Question 2: public row XOR.
    let row = |words: usize, shape: RowShape, cache: Cache, anchor: bool| {
        spec(
            Question::PublicRowXor,
            format!("row-xor-{words}w-{}-{}", shape.id(), cache.id()),
            Workload::RowXor { words, shape },
            cache,
            anchor,
        )
    };
    table.extend(ANCHOR_WORDS.map(|w| row(w, RowShape::Full, Cache::Warm, true)));
    table.extend(NEIGHBOR_WORDS.map(|w| row(w, RowShape::Full, Cache::Warm, false)));
    table.extend(ANCHOR_WORDS.map(|w| row(w, RowShape::Tail63, Cache::Warm, false)));
    table.extend(ANCHOR_WORDS.map(|w| row(w, RowShape::Full, Cache::Streaming, false)));

    // Question 3: NR construction. The five selected routes are the whole
    // confirmatory family; the exploratory matrix repeats the primary once.
    let nr = |target: NrTarget, cache: Cache, anchor: bool| {
        spec(
            Question::NrConstruction,
            format!("nr-construct-{}-{}", target.suffix, cache.id()),
            Workload::Nr(target),
            cache,
            anchor,
        )
    };
    table.extend(NR_TARGETS.map(|target| nr(target, Cache::Warm, true)));
    table.push(nr(NR_TARGETS[0], Cache::Cold, false));

    // Question 4: ISA-L scalar gap.
    let isal = |words: usize, cache: Cache, anchor: bool| {
        spec(
            Question::IsalBaseGap,
            format!("isal-base-gap-{words}w-a64-{}", cache.id()),
            Workload::IsalGap { words },
            cache,
            anchor,
        )
    };
    table.extend(ANCHOR_WORDS.map(|w| isal(w, Cache::Warm, true)));
    table.extend(NEIGHBOR_WORDS.map(|w| isal(w, Cache::Warm, false)));
    table.extend(ANCHOR_WORDS.map(|w| isal(w, Cache::Streaming, false)));

    // Ordinal zero uses the campaign seed; ordinal i the i-th subsequent
    // output of one canonical SplitMix64 stream started at that seed.
    let mut mixer = SplitMix64::new(CAMPAIGN_SEED);
    for (ordinal, cell) in table.iter_mut().enumerate() {
        cell.ordinal = ordinal;
        cell.seed = if ordinal == 0 {
            CAMPAIGN_SEED
        } else {
            mixer.next_u64()
        };
    }
    table
}

/// The frozen cells of one question, in ordinal order.
pub fn family_cells(question: Question) -> Vec<Cell> {
    cells()
        .into_iter()
        .filter(|cell| cell.question == question)
        .collect()
}
