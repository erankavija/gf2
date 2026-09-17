//! The frozen cell table of addendum `2037941f-dense-parity-v1`.
//!
//! [`cells`] returns every cell the addendum declares, in the addendum's own
//! order: the cell-matrix sections in document order, and within each section
//! the normative primary Cartesian product followed by the exact exploratory
//! additions in listed order. Ordinal zero carries the campaign seed; ordinal
//! `i` carries the `i`-th subsequent output of one canonical SplitMix64 stream
//! started at the campaign seed.
//!
//! The addendum's `scalar-reference` question owns no ledger: its cells are the
//! allocated family's `scalar-reference` exploratory rows, whose baseline arm is
//! the executable built without the `simd` feature.

use tuning_campaign_support::abtest::SplitMix64;

/// Repository-relative path of the frozen prose addendum this table transcribes.
pub const ADDENDUM_PATH: &str =
    "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md";
/// Content identity of that frozen document, verified before a timed run.
pub const ADDENDUM_SHA256: &str =
    "fcc13c23a18db121f7d7ff85f80c422da21883ddedf151080e61a2ae9e10dfdb";
/// Addendum identity string the frozen document declares.
pub const ADDENDUM_IDENTITY: &str = "2037941f-dense-parity-v1";
/// Freeze time the frozen document declares.
pub const ADDENDUM_FROZEN_UTC: &str = "2026-09-16T16:16:04Z";
/// Repository-relative path of the qualified M4RI matched-operation specification.
pub const COMPARATOR_PATH: &str =
    "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/m4ri-operation-match.md";
/// Repository-relative path of the executed M4RI qualification record, whose
/// pinned library digest the comparator arm verifies against what it loads.
pub const QUALIFICATION_RECORD: &str =
    "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/m4ri-probe-record.txt";
/// Name stem of the external shared object a comparator arm loads.
pub const COMPARATOR_LIBRARY_STEM: &str = "m4ri";

/// Campaign seed of the frozen addendum.
pub const CAMPAIGN_SEED: u64 = 0x2037_941f_96c9_4b81;
/// Anchor word counts; these cells form the confirmatory families.
pub const ANCHOR_WORDS: [usize; 5] = [8, 9, 63, 64, 65];
/// Neighbouring word counts bounding the cutover and the story's upper edge.
pub const NEIGHBOR_WORDS: [usize; 2] = [7, 66];
/// Every word count the oracle exercises.
pub const ALL_WORDS: [usize; 7] = [7, 8, 9, 63, 64, 65, 66];
/// Logical column counts the oracle exercises at the word boundary.
pub const BOUNDARY_BITS: [usize; 5] = [0, 1, 63, 64, 65];
/// Rows in every gf2-only allocated cell.
pub const MATVEC_ROWS: usize = 1024;
/// Word count at and above which the production selector takes the SIMD lane.
pub const SIMD_LANE_MIN_WORDS: usize = 8;
/// Smallest byte size of one streaming fixture bank.
pub const STREAMING_BANK_BYTES: usize = 8 << 20;
/// Fixture banks in a streaming working set; the canonical timing rotation.
pub const STREAMING_BANKS: usize = tuning_campaign_support::timing::FIXTURE_BANKS;

/// One of the addendum's canonical ledger-owning questions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Question {
    /// Isolated call to the bundle's fused AND-population-count entry.
    IsolatedFusedParity,
    /// Public `BitMatrix::matvec` with its output allocation.
    AllocatedMatvec,
    /// Whole-consumer gap against the qualified external M4RI product.
    MatvecVsM4ri,
}

impl Question {
    /// Every question in the addendum's document order.
    pub const ALL: [Self; 3] = [
        Self::IsolatedFusedParity,
        Self::AllocatedMatvec,
        Self::MatvecVsM4ri,
    ];

    /// The addendum's name for this question.
    pub fn id(self) -> &'static str {
        match self {
            Self::IsolatedFusedParity => "isolated-fused-parity",
            Self::AllocatedMatvec => "allocated-matvec",
            Self::MatvecVsM4ri => "matvec-vs-m4ri",
        }
    }

    /// The family identity that owns this question's ledger.
    pub fn family_id(self) -> &'static str {
        match self {
            Self::IsolatedFusedParity => "2037941f-dense-isolated-fused-parity",
            Self::AllocatedMatvec => "2037941f-dense-allocated-matvec",
            Self::MatvecVsM4ri => "2037941f-dense-matvec-vs-m4ri",
        }
    }

    /// The canonical append-only family ledger.
    pub fn ledger_path(self) -> &'static str {
        match self {
            Self::IsolatedFusedParity => {
                "dev/bench_results/2037941f/dense-isolated-fused-parity-ledger.jsonl"
            }
            Self::AllocatedMatvec => {
                "dev/bench_results/2037941f/dense-allocated-matvec-ledger.jsonl"
            }
            Self::MatvecVsM4ri => "dev/bench_results/2037941f/dense-matvec-vs-m4ri-ledger.jsonl",
        }
    }

    /// Resolves a family identity back to its question.
    pub fn from_family_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|question| question.family_id() == id)
    }
}

/// Column shape of an allocated `matvec` cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatvecShape {
    /// Exactly `64 * words` logical columns; the logical tail is empty.
    Full,
    /// `64 * words - 1` logical columns and one zero padding bit.
    Tail1,
}

impl MatvecShape {
    /// The addendum's name for this shape.
    pub fn id(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Tail1 => "tail1",
        }
    }

    /// Logical column count for `words` words in this shape.
    pub fn columns(self, words: usize) -> usize {
        match self {
            Self::Full => 64 * words,
            Self::Tail1 => 64 * words - 1,
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

/// One qualified M4RI shape of the matched-operation specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct M4riShape {
    /// Cell-identifier suffix naming the shape.
    pub suffix: &'static str,
    /// Rows of the M4RI left operand and of the gf2 matrix.
    pub rows: usize,
    /// Columns of the M4RI left operand and of the gf2 matrix.
    pub cols: usize,
    /// True for the two confirmatory mid-range shapes.
    pub confirmatory: bool,
}

impl M4riShape {
    /// gf2 row stride in words.
    pub fn stride_words(self) -> usize {
        self.cols.div_ceil(64)
    }
}

/// The six qualified M4RI shapes, in the addendum's cell order: the two
/// confirmatory mid-range shapes first, then the remaining qualified shapes.
pub const M4RI_SHAPES: [M4riShape; 6] = [
    M4riShape { suffix: "65x512", rows: 65, cols: 512, confirmatory: true },
    M4riShape { suffix: "65x4096", rows: 65, cols: 4096, confirmatory: true },
    M4riShape { suffix: "1x1", rows: 1, cols: 1, confirmatory: false },
    M4riShape { suffix: "63x63", rows: 63, cols: 63, confirmatory: false },
    M4riShape { suffix: "64x64", rows: 64, cols: 64, confirmatory: false },
    M4riShape { suffix: "65x65", rows: 65, cols: 65, confirmatory: false },
];

/// A comparator row the addendum declares unavailable in advance.
///
/// The row carries a reason and zero samples, takes no ordinal and generates no
/// seed, and is not a runner cell; the family's receipt retains it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnavailableRow {
    /// Frozen row identifier.
    pub row_id: &'static str,
    /// gf2 row stride the anchor would use.
    pub stride_words: usize,
    /// Why the matched-operation specification admits no such cell.
    pub reason: &'static str,
}

/// The three anchor strides the M4RI specification verifies no shape for.
pub const UNAVAILABLE_ROWS: [UnavailableRow; 3] = [
    UnavailableRow {
        row_id: "m4ri-gap-65x576-unqualified",
        stride_words: 9,
        reason: "The matched-operation specification verifies no such shape",
    },
    UnavailableRow {
        row_id: "m4ri-gap-65x4032-unqualified",
        stride_words: 63,
        reason: "The matched-operation specification verifies no such shape",
    },
    UnavailableRow {
        row_id: "m4ri-gap-65x4160-unqualified",
        stride_words: 65,
        reason: "The matched-operation specification verifies no such shape",
    },
];

/// Workload of one cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Workload {
    /// One bundle call over one `words`-word row and the `words`-word vector.
    AndPopcnt {
        /// Words one logical operation consumes.
        words: usize,
    },
    /// One public `BitMatrix::matvec` over a `MATVEC_ROWS`-row matrix.
    Matvec {
        /// Row stride in words.
        words: usize,
        /// Column shape.
        shape: MatvecShape,
    },
    /// One whole-consumer comparison against the external M4RI product.
    M4riGap {
        /// Qualified shape.
        shape: M4riShape,
        /// True when both arms reuse their converted inputs across calls.
        retained: bool,
    },
}

impl Workload {
    /// The canonical question whose cells carry this workload.
    pub fn question(self) -> Question {
        match self {
            Self::AndPopcnt { .. } => Question::IsolatedFusedParity,
            Self::Matvec { .. } => Question::AllocatedMatvec,
            Self::M4riGap { .. } => Question::MatvecVsM4ri,
        }
    }

    /// gf2 row stride in words, for every workload.
    pub fn stride_words(self) -> usize {
        match self {
            Self::AndPopcnt { words } | Self::Matvec { words, .. } => words,
            Self::M4riGap { shape, .. } => shape.stride_words(),
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
    /// True when the baseline arm is the executable built without `simd`.
    pub scalar_reference: bool,
    /// Workload seed derived from the campaign seed stream.
    pub seed: u64,
}

impl Cell {
    /// Whole-consumer cells charge conversion costs; the kernel cell does not.
    pub fn whole_consumer(&self) -> bool {
        !matches!(self.workload, Workload::AndPopcnt { .. })
    }

    /// The addendum's objective for this cell.
    pub fn objective(&self) -> &'static str {
        match self.workload {
            Workload::M4riGap { .. } => "comparator-gap",
            _ => "improvement",
        }
    }

    /// Workload identity recorded in the campaign addendum.
    pub fn workload_identity(&self) -> String {
        format!("{}-{}", self.question.id(), self.cell_id)
    }
}

struct Spec {
    question: Question,
    cell_id: String,
    workload: Workload,
    cache: Cache,
    anchor: bool,
    scalar_reference: bool,
}

fn spec(
    question: Question,
    cell_id: String,
    workload: Workload,
    cache: Cache,
    anchor: bool,
) -> Spec {
    Spec { question, cell_id, workload, cache, anchor, scalar_reference: false }
}

/// The complete frozen cell table, ordinal-ordered and seeded.
///
/// The table is a pure function of the frozen addendum: it reads no file, no
/// environment and no clock, so two invocations in any process produce equal
/// vectors.
pub fn cells() -> Vec<Cell> {
    let mut table: Vec<Spec> = Vec::new();

    // Isolated fused parity: the primary product, then the exact exploratory
    // additions in the addendum's listed order.
    let isolated = |words: usize, cache: Cache, anchor: bool| {
        spec(
            Question::IsolatedFusedParity,
            format!("and-popcnt-{words}w-{}", cache.id()),
            Workload::AndPopcnt { words },
            cache,
            anchor,
        )
    };
    table.extend(ANCHOR_WORDS.map(|w| isolated(w, Cache::Warm, true)));
    table.extend(NEIGHBOR_WORDS.map(|w| isolated(w, Cache::Warm, false)));
    table.extend(ANCHOR_WORDS.map(|w| isolated(w, Cache::Streaming, false)));

    // Allocated whole-matvec.
    let allocated = |words: usize, shape: MatvecShape, cache: Cache, anchor: bool| {
        let tail = match shape {
            MatvecShape::Full => String::new(),
            MatvecShape::Tail1 => "-tail1".to_owned(),
        };
        spec(
            Question::AllocatedMatvec,
            format!("matvec-r{MATVEC_ROWS}-{words}w{tail}-{}", cache.id()),
            Workload::Matvec { words, shape },
            cache,
            anchor,
        )
    };
    table.extend(ANCHOR_WORDS.map(|w| allocated(w, MatvecShape::Full, Cache::Warm, true)));
    table.extend(NEIGHBOR_WORDS.map(|w| allocated(w, MatvecShape::Full, Cache::Warm, false)));
    table.extend(ANCHOR_WORDS.map(|w| allocated(w, MatvecShape::Tail1, Cache::Warm, false)));
    table.extend([8, 64].map(|w| allocated(w, MatvecShape::Full, Cache::Cold, false)));
    table.extend(ANCHOR_WORDS.map(|w| allocated(w, MatvecShape::Full, Cache::Streaming, false)));
    table.extend(ANCHOR_WORDS.map(|words| Spec {
        cell_id: format!("matvec-r{MATVEC_ROWS}-{words}w-scalar-reference-warm"),
        scalar_reference: true,
        ..allocated(words, MatvecShape::Full, Cache::Warm, false)
    }));

    // M4RI comparator: the two confirmatory qualified shapes, the remaining
    // qualified shapes, then the two retained-state cells.
    let comparator = |shape: M4riShape, retained: bool| {
        let suffix = if retained { "-retained" } else { "" };
        spec(
            Question::MatvecVsM4ri,
            format!("m4ri-gap-{}{suffix}-warm", shape.suffix),
            Workload::M4riGap { shape, retained },
            Cache::Warm,
            shape.confirmatory && !retained,
        )
    };
    table.extend(M4RI_SHAPES.map(|shape| comparator(shape, false)));
    table.extend(
        M4RI_SHAPES
            .into_iter()
            .filter(|shape| shape.confirmatory)
            .map(|shape| comparator(shape, true)),
    );

    // Ordinal zero uses the campaign seed; ordinal i the i-th subsequent
    // output of one canonical SplitMix64 stream started at that seed.
    let mut mixer = SplitMix64::new(CAMPAIGN_SEED);
    table
        .into_iter()
        .enumerate()
        .map(|(ordinal, spec)| Cell {
            ordinal,
            question: spec.question,
            cell_id: spec.cell_id,
            workload: spec.workload,
            cache: spec.cache,
            anchor: spec.anchor,
            scalar_reference: spec.scalar_reference,
            seed: if ordinal == 0 { CAMPAIGN_SEED } else { mixer.next_u64() },
        })
        .collect()
}

/// The frozen cells of one question, in ordinal order.
pub fn family_cells(question: Question) -> Vec<Cell> {
    cells()
        .into_iter()
        .filter(|cell| cell.question == question)
        .collect()
}
