//! Narrow UOR trait implementations: content addressing for IPFS CIDs and trace metrics for runs.

use crate::primitives::AppPrimitives;
use uor_foundation::bridge::trace::TraceMetrics;
use uor_foundation::kernel::address::Address;

/// Wraps an IPFS CID (and optional multihash metadata) as a UOR [`Address`].
///
/// v0 maps the textual CID into UOR address fields without full Braille glyph encoding;
/// `glyph` and `addresses` both carry the CID string for interoperability with gateways.
#[derive(Debug, Clone)]
pub struct CidAddress {
    /// Braille placeholder: same as CID until a dedicated Braille projection is added.
    glyph: String,
    addresses: String,
    digest: String,
    digest_algorithm: String,
    canonical_bytes_b64: String,
    quantum: u64,
}

impl CidAddress {
    pub fn from_cid(cid: &str) -> Self {
        Self {
            glyph: cid.to_string(),
            addresses: cid.to_string(),
            digest: cid.to_string(),
            digest_algorithm: "cidv1".to_string(),
            canonical_bytes_b64: String::new(),
            quantum: 8,
        }
    }

    /// CID string (same as [`Address::glyph`] in v0).
    pub fn cid(&self) -> &str {
        &self.glyph
    }

    pub fn with_canonical_digest(
        cid: &str,
        digest_algorithm: &str,
        digest_hex: &str,
        canonical_bytes_b64: &str,
        quantum: u64,
    ) -> Self {
        Self {
            glyph: cid.to_string(),
            addresses: cid.to_string(),
            digest: digest_hex.to_string(),
            digest_algorithm: digest_algorithm.to_string(),
            canonical_bytes_b64: canonical_bytes_b64.to_string(),
            quantum,
        }
    }
}

impl Address<AppPrimitives> for CidAddress {
    fn glyph(&self) -> &<AppPrimitives as uor_foundation::Primitives>::String {
        &self.glyph
    }

    fn length(&self) -> <AppPrimitives as uor_foundation::Primitives>::NonNegativeInteger {
        self.glyph.chars().count() as u64
    }

    fn addresses(&self) -> &<AppPrimitives as uor_foundation::Primitives>::String {
        &self.addresses
    }

    fn digest(&self) -> &<AppPrimitives as uor_foundation::Primitives>::String {
        &self.digest
    }

    fn digest_algorithm(&self) -> &<AppPrimitives as uor_foundation::Primitives>::String {
        &self.digest_algorithm
    }

    fn canonical_bytes(&self) -> &<AppPrimitives as uor_foundation::Primitives>::String {
        &self.canonical_bytes_b64
    }

    fn quantum(&self) -> <AppPrimitives as uor_foundation::Primitives>::PositiveInteger {
        self.quantum
    }
}

/// Maps a Wasm run to [`TraceMetrics`] (ring/Hamming distances are host-estimated from I/O volume for v0).
#[derive(Debug, Clone, Copy)]
pub struct SandboxTraceMetrics {
    pub step_count: u64,
    pub total_ring_distance: u64,
    pub total_hamming_distance: u64,
}

impl TraceMetrics<AppPrimitives> for SandboxTraceMetrics {
    fn step_count(&self) -> <AppPrimitives as uor_foundation::Primitives>::NonNegativeInteger {
        self.step_count
    }

    fn total_ring_distance(&self) -> <AppPrimitives as uor_foundation::Primitives>::NonNegativeInteger {
        self.total_ring_distance
    }

    fn total_hamming_distance(&self) -> <AppPrimitives as uor_foundation::Primitives>::NonNegativeInteger {
        self.total_hamming_distance
    }
}
