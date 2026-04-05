//! Decreasing Reed-Muller (dRM) codes for GRAND decoding.
//!
//! # Background
//!
//! Decreasing Reed-Muller codes were introduced by Camion and Poli (2021) as a
//! modification of classical Reed-Muller codes with improved minimum distance
//! properties. The dRM(32, 21) code is used as a component code in product codes
//! decoded by SOGRAND.
//!
//! # Construction (Research Notes)
//!
//! The dRM(32, 21) code is derived from the first-order Reed-Muller code RM(1, 5)
//! (which is a (32, 6) code) and second-order RM(2, 5) (which is a (32, 16) code).
//! The "decreasing" construction selects specific rows from the RM generator matrix
//! to achieve the (32, 21) parameters with improved distance properties compared
//! to a simple punctured or shortened Reed-Muller code.
//!
//! The precise row-selection rule from Camion-Poli requires further research into
//! the original paper to implement correctly. The generator matrix must satisfy:
//! - G is 21 x 32 over GF(2)
//! - H * G^T = 0 where H is 11 x 32
//! - Minimum distance should be determined from the construction
//!
//! # Status
//!
//! **TODO**: The dRM(32, 21) construction requires deeper study of the Camion-Poli
//! paper to determine the exact generator matrix. The SOGRAND product code
//! construction (Fig. 1 in the GRAND literature) requires this code, but all
//! other component codes (eBCH, CRC) are complete and functional.
//!
//! Once the generator matrix is determined, the implementation follows the same
//! pattern as `CrcCode`: construct G and H, provide encoding via matrix
//! multiplication, and expose the GRAND interface (H, n, k, is_even).

// TODO(jit:1474a3ca): Implement dRM(32, 21) once the Camion-Poli construction
// is fully researched. The module structure is ready — only the generator matrix
// needs to be determined.
