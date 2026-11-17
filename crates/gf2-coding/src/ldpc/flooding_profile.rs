//! Flooding-normalization identities, quantized table keys and constructor
//! witnesses for the standard-code universe (@/issue/f970febd).

use super::dvb_t2::{concat::DEFAULT_MAX_LDPC_ITERATIONS, dvb_t2_matrices::CODE_TABLES, FrameSize};
use super::nr_5g::{self, all_lifting_sizes, kb_for_z_selection, SOFT_DECODE_MAX_ITERATIONS};
use crate::bch::CodeRate;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// Number of key-grid subdivisions per NR base column (`de-configuration`
/// in @/issue/3461a52a).
pub const NR_KEY_RESOLUTION: u32 = 4;

/// A nonnegative exact rational in reduced form, with a positive denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReducedRational {
    numerator: u32,
    denominator: u32,
}

impl ReducedRational {
    /// Reduces a rational by Euclid's algorithm in O(log denominator) time.
    ///
    /// # Panics
    ///
    /// Panics if `denominator` is zero.
    pub fn new(numerator: u32, denominator: u32) -> Self {
        assert!(denominator != 0, "denominator must be positive");
        let (mut a, mut b) = (numerator, denominator);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        Self {
            numerator: numerator / a,
            denominator: denominator / a,
        }
    }

    /// The reduced numerator.
    pub fn numerator(self) -> u32 {
        self.numerator
    }

    /// The positive reduced denominator.
    pub fn denominator(self) -> u32 {
        self.denominator
    }

    fn integer(value: usize) -> Self {
        Self::new(value as u32, 1)
    }

    fn complement(self) -> Self {
        Self::new(self.denominator - self.numerator, self.denominator)
    }

    // Length of [column, column + 1) lying strictly before this boundary.
    fn before(self, column: usize) -> Self {
        let d = u64::from(self.denominator);
        let n = u64::from(self.numerator)
            .saturating_sub(column as u64 * d)
            .min(d);
        Self::new(n as u32, self.denominator)
    }

    fn quantized(self) -> Self {
        let d = u64::from(self.denominator);
        let scaled = u64::from(self.numerator) * u64::from(NR_KEY_RESOLUTION);
        Self::new(((2 * scaled + d) / (2 * d)) as u32, NR_KEY_RESOLUTION)
    }
}

impl Ord for ReducedRational {
    fn cmp(&self, other: &Self) -> Ordering {
        (u64::from(self.numerator) * u64::from(other.denominator))
            .cmp(&(u64::from(other.numerator) * u64::from(self.denominator)))
    }
}

impl PartialOrd for ReducedRational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// NR base-graph identity, independent of lifting size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NrBaseGraph {
    /// Base graph 1.
    Bg1,
    /// Base graph 2.
    Bg2,
}

impl NrBaseGraph {
    /// The graph number accepted by the NR constructors.
    pub fn number(self) -> u8 {
        match self {
            Self::Bg1 => 1,
            Self::Bg2 => 2,
        }
    }

    /// Number of systematic base columns (K_b, not the Z-selection K_b').
    pub fn systematic_columns(self) -> usize {
        match self {
            Self::Bg1 => nr_5g::bg1::BG1_KB,
            Self::Bg2 => nr_5g::bg2::BG2_KB,
        }
    }

    /// Number of base columns, including parity.
    pub fn columns(self) -> usize {
        match self {
            Self::Bg1 => nr_5g::bg1::BG1_COLS,
            Self::Bg2 => nr_5g::bg2::BG2_COLS,
        }
    }

    pub(crate) fn from_number(number: u8) -> Self {
        match number {
            1 => Self::Bg1,
            2 => Self::Bg2,
            _ => panic!("base_graph must be 1 or 2"),
        }
    }
}

/// Exact channel-state fractions of a base column; the three sum to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnFractions {
    /// Fraction supplied by channel observations.
    pub transmitted: ReducedRational,
    /// Fraction fixed to zero by shortening.
    pub filler: ReducedRational,
    /// Fraction with no channel information.
    pub punctured: ReducedRational,
}

/// Bit-granular NR transmission pattern in units of base columns.
///
/// Filler occupies `[filler_start, K_b)`, transmitted parity occupies
/// `[K_b, parity_end)`, and leading systematic puncturing applies only before
/// the filler boundary. The lifting size is deliberately absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NrFloodingProfile {
    base_graph: NrBaseGraph,
    punctured_systematic_columns: u8,
    filler_start: ReducedRational,
    parity_end: ReducedRational,
}

impl NrFloodingProfile {
    /// Constructs an exact transmission pattern in base-column units.
    ///
    /// # Panics
    ///
    /// Panics if the punctured-column count or filler start exceeds K_b, or
    /// if the parity end lies outside `[K_b, N_b]`.
    pub fn new(
        base_graph: NrBaseGraph,
        punctured_systematic_columns: u8,
        filler_start: ReducedRational,
        parity_end: ReducedRational,
    ) -> Self {
        let kb = base_graph.systematic_columns();
        assert!(
            usize::from(punctured_systematic_columns) <= kb,
            "puncturing exceeds systematic section"
        );
        assert!(
            filler_start <= ReducedRational::integer(kb),
            "filler start exceeds systematic section"
        );
        assert!(
            parity_end >= ReducedRational::integer(kb)
                && parity_end <= ReducedRational::integer(base_graph.columns()),
            "parity end outside parity section"
        );
        Self {
            base_graph,
            punctured_systematic_columns,
            filler_start,
            parity_end,
        }
    }

    /// The protograph to which the pattern applies.
    pub fn base_graph(self) -> NrBaseGraph {
        self.base_graph
    }

    /// Number of punctured leading systematic base columns.
    pub fn punctured_systematic_columns(self) -> u8 {
        self.punctured_systematic_columns
    }

    /// Filler start k/Z, in base-column units.
    pub fn filler_start(self) -> ReducedRational {
        self.filler_start
    }

    /// End of kept parity K_b + p/Z, in base-column units.
    pub fn parity_end(self) -> ReducedRational {
        self.parity_end
    }

    /// Derives one column's exact partition in constant time apart from
    /// rational reduction. Filler takes precedence over leading puncturing.
    ///
    /// # Panics
    ///
    /// Panics if `column >= N_b`.
    pub fn column_fractions(self, column: usize) -> ColumnFractions {
        assert!(
            column < self.base_graph.columns(),
            "base column out of range"
        );
        let zero = ReducedRational::integer(0);
        if column < self.base_graph.systematic_columns() {
            let before_filler = self.filler_start.before(column);
            let (transmitted, punctured) =
                if column < usize::from(self.punctured_systematic_columns) {
                    (zero, before_filler)
                } else {
                    (before_filler, zero)
                };
            ColumnFractions {
                transmitted,
                filler: before_filler.complement(),
                punctured,
            }
        } else {
            let transmitted = self.parity_end.before(column);
            ColumnFractions {
                transmitted,
                filler: zero,
                punctured: transmitted.complement(),
            }
        }
    }

    pub(crate) fn mother(base_graph: NrBaseGraph) -> Self {
        Self::new(
            base_graph,
            0,
            ReducedRational::integer(base_graph.systematic_columns()),
            ReducedRational::integer(base_graph.columns()),
        )
    }

    pub(crate) fn rate_matched(
        base_graph: NrBaseGraph,
        k: usize,
        z: usize,
        parity_kept: usize,
    ) -> Self {
        Self::new(
            base_graph,
            2,
            ReducedRational::new(k as u32, z as u32),
            ReducedRational::new(
                (base_graph.systematic_columns() * z + parity_kept) as u32,
                z as u32,
            ),
        )
    }
}

/// Standard-code identity used by flooding normalization. It carries no
/// decoder setting and does not change decoding behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloodingProfile {
    /// A DVB-T2 code-table row, which is already its own table key.
    DvbT2 {
        /// FEC frame size.
        frame_size: FrameSize,
        /// LDPC code rate.
        rate: CodeRate,
    },
    /// An exact NR transmission pattern.
    Nr5g(NrFloodingProfile),
}

impl FloodingProfile {
    /// The shared generation/lookup key: NR boundaries round independently
    /// to the nearest multiple of `1 / NR_KEY_RESOLUTION`, with ties upward.
    /// DVB-T2 identities are unchanged.
    pub fn table_key(self) -> FloodingTableKey {
        FloodingTableKey(match self {
            Self::DvbT2 { .. } => self,
            Self::Nr5g(p) => Self::Nr5g(NrFloodingProfile {
                filler_start: p.filler_start.quantized(),
                parity_end: p.parity_end.quantized(),
                ..p
            }),
        })
    }
}

/// Canonical factor-table key, constructed only by [`FloodingProfile::table_key`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloodingTableKey(FloodingProfile);

impl FloodingTableKey {
    /// The DVB-T2 identity or quantized NR pattern represented by this key.
    pub fn profile(self) -> FloodingProfile {
        self.0
    }
}

/// Constructor arguments witnessing reachability of a table key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloodingWitness {
    /// Arguments for `LdpcCode::dvb_t2_short` or `LdpcCode::dvb_t2_normal`.
    DvbT2 {
        /// Frame-size constructor choice.
        frame_size: FrameSize,
        /// Constructor code rate.
        rate: CodeRate,
    },
    /// Arguments for `QuasiCyclicLdpc::nr_5g`.
    Nr5gMother {
        /// Constructor graph choice.
        base_graph: NrBaseGraph,
        /// Constructor lifting factor.
        lifting_factor: usize,
    },
    /// Arguments for `QuasiCyclicLdpc::nr_5g_rate_matched`.
    Nr5gRateMatched {
        /// Constructor graph choice.
        base_graph: NrBaseGraph,
        /// Transmitted codeword length.
        n: usize,
        /// Message length.
        k: usize,
    },
}

impl FloodingWitness {
    /// Derives the exact constructor profile without expanding a matrix.
    /// NR variants scan the lifting sets; DVB-T2 takes constant time.
    ///
    /// # Panics
    ///
    /// Panics if these arguments are not accepted by the corresponding constructor.
    pub fn profile(self) -> FloodingProfile {
        match self {
            Self::DvbT2 { frame_size, rate } => FloodingProfile::DvbT2 { frame_size, rate },
            Self::Nr5gMother {
                base_graph,
                lifting_factor,
            } => {
                assert!(
                    all_lifting_sizes().contains(&(lifting_factor as u16))
                        && lifting_factor <= u16::MAX as usize,
                    "invalid lifting factor"
                );
                FloodingProfile::Nr5g(NrFloodingProfile::mother(base_graph))
            }
            Self::Nr5gRateMatched { base_graph, n, k } => {
                assert!(n > k, "n must exceed k");
                let z = nr_5g::select_lifting_factor(base_graph.number(), n, k)
                    .expect("no valid lifting factor");
                assert!(
                    k >= 2 * z,
                    "payload smaller than punctured systematic section"
                );
                FloodingProfile::Nr5g(NrFloodingProfile::rate_matched(
                    base_graph,
                    k,
                    z,
                    n - k + 2 * z,
                ))
            }
        }
    }
}

/// A reachable key and one accepted constructor input producing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloodingProfileEntry {
    /// Quantized table identity.
    pub key: FloodingTableKey,
    /// Constructor input retaining the exact, unrounded pattern.
    pub witness: FloodingWitness,
}

/// Enumerates the finite table-key universe, sorted and deduplicated by key.
///
/// The first call derives witnesses from the DVB-T2 tables and all NR payload
/// lengths and lifting-size selection intervals; later calls borrow the cached
/// result. No parity-check matrices are expanded. For P possible payload lengths,
/// L lifting sizes, C base columns and U keys, the initial cost is bounded by
/// O(P L C Q log U) time and O(U) space, where Q is [`NR_KEY_RESOLUTION`].
/// Unpunctured mother profiles have mother-constructor witnesses.
pub fn flooding_profile_universe() -> &'static [FloodingProfileEntry] {
    static UNIVERSE: OnceLock<Vec<FloodingProfileEntry>> = OnceLock::new();
    UNIVERSE.get_or_init(|| {
        let mut entries = BTreeMap::new();
        let mut insert = |witness: FloodingWitness, profile: FloodingProfile| {
            entries.entry(profile.table_key()).or_insert(witness);
        };
        for &(frame_size, rate, _) in CODE_TABLES {
            let witness = FloodingWitness::DvbT2 { frame_size, rate };
            insert(witness, witness.profile());
        }
        let lifting = all_lifting_sizes();
        for bg in [NrBaseGraph::Bg1, NrBaseGraph::Bg2] {
            let witness = FloodingWitness::Nr5gMother {
                base_graph: bg,
                lifting_factor: usize::from(lifting[0]),
            };
            insert(witness, witness.profile());
            let capacity = bg.columns() - bg.systematic_columns() - 2;
            let max_k = bg.systematic_columns() * usize::from(*lifting.last().unwrap());
            for k in 1..=max_k {
                let mut previous = 0;
                for z in lifting.iter().copied().map(usize::from) {
                    if kb_for_z_selection(bg.number(), k) * z < k {
                        continue;
                    }
                    // This z is selected precisely when the preceding eligible z
                    // cannot meet the transmitted-bit budget. Smaller payloads
                    // underflow the constructor's remaining-systematic count.
                    if k < 2 * z {
                        break;
                    }
                    let mut n = k + capacity * previous + 1;
                    let end = k + capacity * z;
                    while n <= end {
                        let profile = FloodingProfile::Nr5g(NrFloodingProfile::rate_matched(
                            bg,
                            k,
                            z,
                            n - k + 2 * z,
                        ));
                        insert(
                            FloodingWitness::Nr5gRateMatched {
                                base_graph: bg,
                                n,
                                k,
                            },
                            profile,
                        );
                        // Jump to the next rounded parity boundary; keep an exact
                        // constructor witness even when z is off the key grid.
                        let FloodingProfile::Nr5g(key) = profile.table_key().profile() else {
                            unreachable!()
                        };
                        let tick = u64::from(key.parity_end.numerator)
                            * u64::from(NR_KEY_RESOLUTION)
                            / u64::from(key.parity_end.denominator);
                        let next_parity_end =
                            ((2 * tick + 1) * z as u64).div_ceil(2 * u64::from(NR_KEY_RESOLUTION));
                        n = next_parity_end as usize - (bg.systematic_columns() + 2) * z + k;
                    }
                    previous = z;
                }
            }
        }
        entries
            .into_iter()
            .map(|(key, witness)| FloodingProfileEntry { key, witness })
            .collect()
    })
}

/// Sorted, distinct default flooding iteration caps used by the standard
/// codecs. The result derives from the decoder constants, not a separate registry.
pub fn flooding_default_caps() -> Vec<usize> {
    let mut caps = vec![DEFAULT_MAX_LDPC_ITERATIONS, SOFT_DECODE_MAX_ITERATIONS];
    caps.sort_unstable();
    caps.dedup();
    caps
}
