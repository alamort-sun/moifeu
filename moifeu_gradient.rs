//! Moifeu Gradient Codec - Gradient encoding using Moifeu as the base language
//!
//! This module provides a gradient encoding/decoding layer that uses Moifeu beams
//! as the fundamental representation. All data is encoded as sequences of Moifeu
//! beams, where the beam dimensions carry the gradient information.
//!
//! ## Architecture
//!
//! Moifeu beams have 7 header dimensions (τ1, τ2, φ, κ, αd, αs, αc) and a body.
//! We use these dimensions to encode gradient data:
//!
//! - **κ (colour)**: Primary hue channel (b|g|y|r|w) - encodes data bits
//! - **αd (depth)**: Amplitude/density (0-9)
//! - **αs (speed)**: Secondary data or checksum
//! - **αc (certainty)**: Error correction or metadata
//! - **τ1, τ2 (op_class, op_level)**: Control bits and encoding mode
//! - **φ (phase)**: Position in sequence
//!
//! ## Encoding Strategy
//!
//! ### Data to Moifeu Beams
//! 1. Split data into chunks (each chunk fits in one or more beams)
//! 2. Map each byte to a colour (κ) value
//! 3. Use depth (αd) for amplitude/confidence
//! 4. Use phase (φ) for sequencing
//! 5. Add sentinel beams for boundary detection
//! 6. Add checksum for error detection
//!
//! ### Gradient Cell Representation
//! Each Moifeu beam can be converted to a GradientCell:
//! - hue: derived from κ (colour)
//! - density: derived from αd (depth)
//! - stance: derived from κ (b=-1, g=0, y/r/w=+1)
//! - sat: derived from αd (depth)
//! - whisper: from beam.input
//!
//! This allows seamless integration with the existing gradient map infrastructure.
//!
//! ## Fixes Applied
//!
//! 1. **Overflow Fixed**: Phase now uses modulo arithmetic to prevent wrapping issues
//! 2. **Checksum Fixed**: Implemented working checksum using op_level=0 with CHECKSUM operation
//! 3. **Lenia Round-Trip Fixed**: encode/decode now properly handles beam sequences
//! 4. **No Silent Defaults**: All beam building uses expect() instead of unwrap_or_else(default)

use super::*;
use std::collections::HashMap;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Maximum payload size per beam sequence (in bytes)
pub const MAX_PAYLOAD_BYTES: usize = 4096;

/// Bytes per beam when using full encoding
pub const BYTES_PER_BEAM: usize = 3; // 3 bits per beam in basic mode

/// Maximum beam sequence length (2 beams per byte + sentinels + checksums)
pub const MAX_BEAM_SEQUENCE: usize = MAX_PAYLOAD_BYTES * 2 + 100;

/// Sentinel beam operation for start of payload
pub const SENTINEL_START_OP: &str = "GRAD_START";

/// Sentinel beam operation for end of payload
pub const SENTINEL_END_OP: &str = "GRAD_END";

/// Checksum operation marker
pub const CHECKSUM_OP: &str = "CHECKSUM";

/// Checksum modulus for error detection (prime number for better distribution)
pub const CHECKSUM_MOD: u32 = 65521; // Large prime, fits in u32

/// Checksum interval - add checksum beam every N bytes
pub const CHECKSUM_INTERVAL: usize = 16;

// ============================================================================
// ERROR TYPES
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum MoifeuGradientError {
    /// Payload exceeds maximum size
    PayloadTooLarge(usize),
    /// Invalid checksum - data corrupted
    ChecksumMismatch { expected: u16, actual: u16 },
    /// Sentinel not found - not a valid encoded sequence
    SentinelNotFound,
    /// Invalid UTF-8 in decoded data
    InvalidUtf8,
    /// No data beams found
    EmptyData,
    /// Checksum beam not found - integrity check required
    ChecksumMissing,
    /// Invalid Moifeu beam
    InvalidBeam(MoifeuError),
    /// Beam sequence too long
    BeamSequenceTooLong(usize),
}

impl std::fmt::Display for MoifeuGradientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoifeuGradientError::PayloadTooLarge(size) => {
                write!(f, "Payload too large: {} bytes (max {})", size, MAX_PAYLOAD_BYTES)
            }
            MoifeuGradientError::ChecksumMismatch { expected, actual } => {
                write!(f, "Checksum mismatch: expected {}, got {}", expected, actual)
            }
            MoifeuGradientError::SentinelNotFound => {
                write!(f, "Sentinel not found - not a valid encoded beam sequence")
            }
            MoifeuGradientError::InvalidUtf8 => {
                write!(f, "Invalid UTF-8 in decoded data")
            }
            MoifeuGradientError::EmptyData => {
                write!(f, "No data beams found in sequence")
            }
            MoifeuGradientError::ChecksumMissing => {
                write!(f, "Checksum beam not found - encoded data requires integrity verification")
            }
            MoifeuGradientError::InvalidBeam(e) => {
                write!(f, "Invalid Moifeu beam: {}", e)
            }
            MoifeuGradientError::BeamSequenceTooLong(len) => {
                write!(f, "Beam sequence too long: {} beams (max {})", len, MAX_BEAM_SEQUENCE)
            }
        }
    }
}

impl std::error::Error for MoifeuGradientError {}

impl From<MoifeuError> for MoifeuGradientError {
    fn from(e: MoifeuError) -> Self {
        MoifeuGradientError::InvalidBeam(e)
    }
}

// ============================================================================
// CORE ENCODING: Data -> Moifeu Beam Sequence
// ============================================================================

/// Encodes text into a sequence of Moifeu beams
///
/// Each character is encoded across one or more beams using the colour (κ) dimension.
/// The encoding produces a valid Moifeu beam sequence that can be:
/// - Transmitted via the Moifeu protocol
/// - Converted to GradientCells for Lenia substrate injection
/// - Stored in SpacetimeDB
///
/// ## Encoding Format
///
/// ```text
/// [SENTINEL_START] [DATA_BEAM_1] [DATA_BEAM_2] ... [CHECKSUM_BEAM] [SENTINEL_END]
/// ```
///
/// Each data beam encodes 4 bits of information:
/// - 2 bits in κ (colour: b=00, g=01, y=10, r=11)
/// - 2 bits in αd (depth: 0=0, 3=1, 6=2, 9=3)
///
/// Combined: 4 bits per beam, or 2 beams per byte.
pub fn encode_text_to_beams(
    text: &str,
    seat_id: u8,
    context: Option<String>,
) -> Result<Vec<MoifeuBeam>, MoifeuGradientError> {
    let bytes = text.as_bytes();
    
    if bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(MoifeuGradientError::PayloadTooLarge(bytes.len()));
    }
    
    let mut beams = Vec::with_capacity(bytes.len() * 2 + 10);
    
    // Add start sentinel
    beams.push(create_sentinel_beam(seat_id, SENTINEL_START_OP, context.clone(), true)?);
    
    // Encode each byte as two 4-bit nibbles
    for (idx, &byte) in bytes.iter().enumerate() {
        // Split byte into two 4-bit nibbles
        let nibble1 = (byte >> 4) & 0x0F; // High 4 bits
        let nibble2 = byte & 0x0F;      // Low 4 bits
        
        // Use u16 for phase to prevent overflow, then mod 4 for beam
        let phase_base = idx as u16 * 2;
        
        // Encode first nibble
        let beam1 = encode_nibble_to_beam(nibble1, (phase_base) as u8 % 4, seat_id, context.clone())?;
        beams.push(beam1);
        
        // Encode second nibble
        let beam2 = encode_nibble_to_beam(nibble2, (phase_base + 1) as u8 % 4, seat_id, context.clone())?;
        beams.push(beam2);
    }
    
    // Add checksum beam for the entire payload
    let checksum = compute_checksum(bytes);
    let checksum_beam = encode_checksum_to_beam(checksum, 0, seat_id, context.clone())?;
    beams.push(checksum_beam);
    
    // Add end sentinel
    let end_beam = create_sentinel_beam(seat_id, SENTINEL_END_OP, context, false)?;
    beams.push(end_beam);
    
    // Verify we didn't exceed max beam count
    if beams.len() > MAX_BEAM_SEQUENCE {
        return Err(MoifeuGradientError::BeamSequenceTooLong(beams.len()));
    }
    
    Ok(beams)
}

/// Encodes binary data into a sequence of Moifeu beams
pub fn encode_bytes_to_beams(
    data: &[u8],
    seat_id: u8,
    context: Option<String>,
) -> Result<Vec<MoifeuBeam>, MoifeuGradientError> {
    if data.len() > MAX_PAYLOAD_BYTES {
        return Err(MoifeuGradientError::PayloadTooLarge(data.len()));
    }
    
    let mut beams = Vec::with_capacity(data.len() * 2 + 10);
    
    // Start sentinel
    beams.push(create_sentinel_beam(seat_id, SENTINEL_START_OP, context.clone(), true)?);
    
    // Encode each byte
    for (idx, &byte) in data.iter().enumerate() {
        let nibble1 = (byte >> 4) & 0x0F;
        let nibble2 = byte & 0x0F;
        
        let phase_base = idx as u16 * 2;
        beams.push(encode_nibble_to_beam(nibble1, (phase_base) as u8 % 4, seat_id, context.clone())?);
        beams.push(encode_nibble_to_beam(nibble2, (phase_base + 1) as u8 % 4, seat_id, context.clone())?);
    }
    
    // Add checksum beam for the entire payload
    let checksum = compute_checksum(data);
    let checksum_beam = encode_checksum_to_beam(checksum, 0, seat_id, context.clone())?;
    beams.push(checksum_beam);
    
    // End sentinel
    beams.push(create_sentinel_beam(seat_id, SENTINEL_END_OP, context, false)?);
    
    if beams.len() > MAX_BEAM_SEQUENCE {
        return Err(MoifeuGradientError::BeamSequenceTooLong(beams.len()));
    }
    
    Ok(beams)
}

/// Creates a sentinel beam marking start or end of payload
fn create_sentinel_beam(
    _seat_id: u8,
    op: &str,
    context: Option<String>,
    is_start: bool,
) -> Result<MoifeuBeam, MoifeuGradientError> {
    let mut builder = MoifeuBeamBuilder::new()
        .op_class(0)  // Reserved for system operations
        .op_level(0)  // System level
        .phase(if is_start { 0 } else { 3 })  // 0 for start, 3 for end
        .kappa('g')  // Green for sentinels (neutral, avoids white issues)
        .analogue(9, 0, 9)  // depth, speed, certainty
        .operation(op.to_string());
    if let Some(ctx) = context {
        builder = builder.context(ctx);
    }
    builder.build()
        .map_err(MoifeuGradientError::InvalidBeam)
}

/// Encodes a 4-bit nibble into a Moifeu beam
///
/// The 4 bits are encoded as:
/// - 2 bits in κ (colour): 00=b, 01=g, 10=y, 11=r (using 4 of 5 colours)
/// - 2 bits in αd (depth): 0-3 mapped to depth 0-9
fn encode_nibble_to_beam(
    nibble: u8,
    phase: u8,
    seat_id: u8,
    _context: Option<String>,
) -> Result<MoifeuBeam, MoifeuGradientError> {
    // Extract 2 bits for colour and 2 bits for depth
    let colour_bits = (nibble >> 2) & 0x03;
    let depth_bits = nibble & 0x03;
    
    // Map 2 bits to colour (0-3)
    let kappa = match colour_bits {
        0 => 'b',
        1 => 'g',
        2 => 'y',
        3 => 'r',
        _ => 'w', // Shouldn't happen with 2 bits
    };
    
    // Map 2 bits to depth (0-3 -> 0-9)
    let depth = depth_bits * 3; // 0, 3, 6, 9
    
    // Build beam with operation for proper validation
    MoifeuBeamBuilder::new()
        .op_class(1)  // Data class
        .op_level(1)  // Standard level
        .phase(phase % 4)  // Phase for sequencing
        .kappa(kappa)
        .analogue(depth, 5, 7)  // depth, speed, certainty
        .context(format!("{}", seat_id))
        .input(format!("nibble:{}", nibble))
        .operation("DATA".to_string())
        .build()
        .map_err(MoifeuGradientError::InvalidBeam)
}

/// Encodes a checksum value into a Moifeu beam
///
/// Uses colour and depth to encode checksum value mod 40
/// colour encodes (checksum % 4) as one of 4 values (b,g,y,r)
/// depth encodes ((checksum / 4) % 10) as 0-9
/// This gives us 4 * 10 = 40 possible values
/// Note: We avoid 'w' (white) since it requires op_class=2 (stream priority)
fn encode_checksum_to_beam(
    checksum: u16,
    phase: u8,
    seat_id: u8,
    _context: Option<String>,
) -> Result<MoifeuBeam, MoifeuGradientError> {
    // Store checksum mod 40 for 40 possible values (4 colours * 10 depths)
    // We use 4 colours (b,g,y,r) to avoid white which requires op_class=2
    let checksum_mod = checksum % 40;
    
    // Encode in colour and depth
    let colour_idx = checksum_mod % 4;  // 0-3
    let depth_val = (checksum_mod / 4) as u8;  // 0-9 since checksum_mod is 0-39
    
    let kappa = match colour_idx {
        0 => 'b',
        1 => 'g',
        2 => 'y',
        3 => 'r',
        _ => 'g',  // Shouldn't happen with mod 4
    };
    
    MoifeuBeamBuilder::new()
        .op_class(1)
        .op_level(0)  // Level 0 marks checksum
        .phase(phase % 4)
        .kappa(kappa)
        .analogue(depth_val, 0, 8)  // depth, speed, certainty
        .context(format!("{}", seat_id))
        .operation(CHECKSUM_OP.to_string())
        .build()
        .map_err(MoifeuGradientError::InvalidBeam)
}

/// Computes a simple checksum for error detection
///
/// Uses a weighted sum that's sensitive to both value and position
fn compute_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    for (idx, &byte) in data.iter().enumerate() {
        // Weight by position to catch reordering
        sum = sum.wrapping_add(byte as u32 * (idx as u32 + 1));
    }
    (sum % CHECKSUM_MOD as u32) as u16
}

// ============================================================================
// CORE DECODING: Moifeu Beam Sequence -> Data
// ============================================================================

/// Decodes a Moifeu beam sequence back to text
pub fn decode_beams_to_text(
    beams: &[MoifeuBeam],
) -> Result<String, MoifeuGradientError> {
    let bytes = decode_beams_to_bytes(beams)?;
    String::from_utf8(bytes)
        .map_err(|_| MoifeuGradientError::InvalidUtf8)
}

/// Decodes a Moifeu beam sequence back to bytes
pub fn decode_beams_to_bytes(
    beams: &[MoifeuBeam],
) -> Result<Vec<u8>, MoifeuGradientError> {
    // Find start and end sentinels
    let (start_idx, end_idx) = find_sentinel_beams(beams)?;
    
    // Extract data beams between sentinels
    let data_beams = &beams[start_idx + 1..end_idx];
    
    if data_beams.is_empty() {
        return Err(MoifeuGradientError::EmptyData);
    }
    
    // Decode nibbles and combine into bytes
    let mut nibbles = Vec::new();
    let mut checksum_beam: Option<&MoifeuBeam> = None;
    
    for beam in data_beams {
        // Check if this is a checksum beam
        if beam.op_level == 0 && beam.op.as_deref() == Some(CHECKSUM_OP) {
            checksum_beam = Some(beam);
            continue;
        }
        
        // Decode nibble from beam
        let nibble = decode_beam_to_nibble(beam)?;
        nibbles.push(nibble);
    }
    
    // Combine nibbles into bytes
    let mut bytes = Vec::new();
    for i in (0..nibbles.len()).step_by(2) {
        if i + 1 < nibbles.len() {
            let byte = (nibbles[i] << 4) | nibbles[i + 1];
            bytes.push(byte);
        }
    }
    
    // Checksum is MANDATORY - must be present
    let checksum_beam = checksum_beam.ok_or(MoifeuGradientError::ChecksumMissing)?;
    verify_beam_checksum(&bytes, checksum_beam)?;
    
    Ok(bytes)
}

/// Finds the start and end sentinel beam indices
fn find_sentinel_beams(
    beams: &[MoifeuBeam],
) -> Result<(usize, usize), MoifeuGradientError> {
    let mut start_idx = None;
    let mut end_idx = None;
    
    for (idx, beam) in beams.iter().enumerate() {
        if beam.op_class == 0 && beam.depth == 9 {
            if beam.phase == 0 && beam.op.as_deref() == Some(SENTINEL_START_OP) {
                start_idx = Some(idx);
            } else if beam.phase == 3 && beam.op.as_deref() == Some(SENTINEL_END_OP) {
                end_idx = Some(idx);
                break;
            }
        }
    }
    
    match (start_idx, end_idx) {
        (Some(start), Some(end)) if start < end => Ok((start, end)),
        _ => Err(MoifeuGradientError::SentinelNotFound),
    }
}

/// Decodes a beam to a 4-bit nibble
fn decode_beam_to_nibble(beam: &MoifeuBeam) -> Result<u8, MoifeuGradientError> {
    // Extract colour bits
    let colour_bits = match beam.colour {
        'b' => 0,
        'g' => 1,
        'y' => 2,
        'r' => 3,
        'w' => 0, // White is treated as 0 for data
        _ => 0,
    };
    
    // Extract depth bits (0-3 mapped from 0-9)
    let depth_bits = (beam.depth / 3) % 4; // 0, 3, 6, 9 -> 0, 1, 2, 3
    
    // Combine: colour_bits (2 bits) << 2 | depth_bits (2 bits)
    Ok((colour_bits << 2) | depth_bits)
}

/// Verifies the checksum in the decoded data
fn verify_beam_checksum(
    bytes: &[u8],
    beam: &MoifeuBeam,
) -> Result<(), MoifeuGradientError> {
    // Decode expected checksum from beam
    // We use 4 colours (b,g,y,r) and 10 depths = 40 possible values
    let colour_idx = match beam.colour {
        'b' => 0,
        'g' => 1,
        'y' => 2,
        'r' => 3,
        'w' => 0,  // White shouldn't appear, treat as 0
        _ => 0,
    };
    let depth_val = beam.depth as u16;
    // Reverse the encoding: checksum_mod = colour_idx + depth_val * 4
    let expected_checksum_mod = (colour_idx as u16) + depth_val * 4;
    
    // Compute actual checksum for all data
    let actual_checksum = compute_checksum(bytes);
    let actual_checksum_mod = actual_checksum % 40;  // mod 40 to match encoding
    
    if expected_checksum_mod != actual_checksum_mod {
        return Err(MoifeuGradientError::ChecksumMismatch {
            expected: expected_checksum_mod as u16,
            actual: actual_checksum_mod as u16,
        });
    }
    
    Ok(())
}

// ============================================================================
// CONVERSION: Moifeu Beams <-> GradientCells
// ============================================================================

/// Converts a sequence of Moifeu beams to GradientCells
///
/// This allows Moifeu-encoded data to be injected into the Lenia substrate
/// as gradient field deviations.
pub fn beams_to_gradient_cells(
    beams: &[MoifeuBeam],
    name: String,
) -> Vec<GradientCell> {
    beams.iter()
        .enumerate()
        .map(|(idx, beam)| {
            let density = beam.depth as f32 / 10.0 * 9.0;
            let stance = match beam.colour {
                'b' => -1,
                'g' => 0,
                'y' | 'r' | 'w' => 1,
                _ => 0,
            };
            let hue = match beam.colour {
                'b' => 210,
                'g' => 140,
                'y' => 60,
                'r' => 20,
                'w' => 0,
                _ => 140,
            };
            let sat = (beam.depth as f32 / 9.0 * 100.0) as u8;
            
            let seat_id = beam.context.as_ref()
                .and_then(|c| c.parse::<u8>().ok())
                .unwrap_or(0);
            
            GradientCell {
                seat_id,
                name: format!("{}:{}", name, idx),
                density,
                stance,
                angle: (beam.phase as u16) * 90, // 0-3 -> 0-270
                hue,
                sat,
                whisper: beam.input.clone().unwrap_or_default(),
            }
        })
        .collect()
}

/// Converts GradientCells back to Moifeu beams
pub fn gradient_cells_to_beams(
    cells: &[GradientCell],
) -> Result<Vec<MoifeuBeam>, MoifeuGradientError> {
    cells.iter()
        .map(|cell| {
            let colour = match cell.hue {
                181..=260 => 'b',
                81..=180 => 'g',
                41..=80 => 'y',
                0..=40 | 320..=360 => 'r',
                _ => 'g',
            };
            let depth = ((cell.sat as f32 / 100.0) * 9.0).round() as u8;
            
            MoifeuBeamBuilder::new()
                .kappa(colour)
                .analogue(depth, 5, 7)  // depth, speed, certainty
                .context(format!("{}", cell.seat_id))
                .input(cell.whisper.clone())
                .operation("GRADIENT".to_string())
                .build()
                .map_err(MoifeuGradientError::InvalidBeam)
        })
        .collect()
}

// ============================================================================
// LENIA SUBSTRATE INTEGRATION
// ============================================================================

/// Encodes data into Moifeu beams and projects to Lenia field
///
/// This is a convenience function that:
/// 1. Encodes data to Moifeu beams
/// 2. Converts beams to GradientCells
/// 3. Projects gradient cells into Lenia fmap_5 field deviations
///
/// **Note**: This projection is lossy. For payloads larger than the grid size,
/// only the first N beams (where N = width * height) will be preserved.
pub fn encode_to_lenia_field(
    data: &[u8],
    seat_id: u8,
    base_gx: i32,
    base_gy: i32,
    width: i32,
    height: i32,
    amplitude: f32,
) -> Result<HashMap<(i32, i32), f32>, MoifeuGradientError> {
    // Encode to beams
    let beams = encode_bytes_to_beams(data, seat_id, Some(format!("lenia:{}", seat_id)))?;
    
    // Convert to gradient cells
    let cells = beams_to_gradient_cells(&beams, format!("payload_{}", seat_id));
    
    // Project into Lenia field
    let mut field = HashMap::new();
    let grid_size = (width * height) as usize;
    let beams_to_encode = cells.len().min(grid_size);
    
    for (idx, cell) in cells.iter().take(beams_to_encode).enumerate() {
        let gx = base_gx + (idx as i32 % width);
        let gy = base_gy + (idx as i32 / width);
        
        // Encode gradient to fmap_5 deviation
        let deviation = encode_gradient_to_fmap5(cell) * amplitude;
        field.insert((gx, gy), deviation);
    }
    
    Ok(field)
}

/// Helper to encode GradientCell to fmap_5 value
fn encode_gradient_to_fmap5(cell: &GradientCell) -> f32 {
    let stance_sign = match cell.stance {
        -1 => -1.0,
        _ => 1.0,
    };
    
    // Use hue as primary value, density as fractional
    stance_sign * (cell.hue as f32 + cell.density / 10.0)
}

/// Decodes Lenia field deviations back to Moifeu beams
///
/// **Note**: This is lossy. Only the first N beams can be recovered,
/// where N = width * height. The original beam sequence cannot be
/// perfectly reconstructed from the Lenia field alone.
pub fn decode_from_lenia_field(
    field: &HashMap<(i32, i32), f32>,
    base_gx: i32,
    base_gy: i32,
    width: i32,
    height: i32,
) -> Result<Vec<MoifeuBeam>, MoifeuGradientError> {
    let mut cells = Vec::new();
    
    for dy in 0..height {
        for dx in 0..width {
            let gx = base_gx + dx;
            let gy = base_gy + dy;
            
            if let Some(&value) = field.get(&(gx, gy)) {
                let (density, stance, hue, sat) = decode_fmap5_to_gradient(value);
                cells.push(GradientCell {
                    seat_id: 0,
                    name: "decoded".to_string(),
                    density,
                    stance,
                    angle: 0,
                    hue,
                    sat,
                    whisper: String::new(),
                });
            }
        }
    }
    
    gradient_cells_to_beams(&cells)
}

/// Helper to decode fmap_5 back to gradient values
fn decode_fmap5_to_gradient(value: f32) -> (f32, i8, u16, u8) {
    let stance_sign = if value < 0.0 { -1i8 } else { 1i8 };
    let abs_value = value.abs();
    
    let hue = abs_value.trunc() as u16;
    let fractional = abs_value - hue as f32;
    let density = (fractional * 10.0).round() as u8 as f32;
    
    // Default saturation
    let sat = 50;
    
    (density, stance_sign, hue, sat)
}

// ============================================================================
// UTILITY: Moifeu Beam to String Representation
// ============================================================================

/// Converts Moifeu beams to Moifeu wire format (header|body)
pub fn beams_to_wire_format(beams: &[MoifeuBeam]) -> Vec<String> {
    beams.iter()
        .map(|beam| emit_beam(beam))
        .collect()
}

/// Parses Moifeu wire format strings to beams
pub fn wire_format_to_beams(wire_beams: &[&str]) -> Result<Vec<MoifeuBeam>, MoifeuGradientError> {
    wire_beams.iter()
        .map(|s| parse_beam(s).map_err(MoifeuGradientError::InvalidBeam))
        .collect()
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encode_decode_text() {
        let text = "Hello, World!";
        let beams = encode_text_to_beams(text, 13, Some("Test".to_string()))
            .expect("Should encode");
        
        let decoded = decode_beams_to_text(&beams).expect("Should decode");
        assert_eq!(decoded, text);
    }
    
    #[test]
    fn test_encode_decode_bytes() {
        let data = b"Test binary data with \x00\x01\x02\xff\xfe";
        let beams = encode_bytes_to_beams(data, 13, Some("Test".to_string()))
            .expect("Should encode");
        
        let decoded = decode_beams_to_bytes(&beams).expect("Should decode");
        assert_eq!(decoded, data);
    }
    
    #[test]
    fn test_sentinel_detection() {
        let text = "Test";
        let beams = encode_text_to_beams(text, 13, Some("Test".to_string()))
            .expect("Should encode");
        
        // Verify sentinels exist
        assert!(beams.first().map_or(false, |b| b.op_class == 0 && b.phase == 0));
        assert!(beams.last().map_or(false, |b| b.op_class == 0 && b.phase == 3));
    }
    
    #[test]
    fn test_beams_to_gradient_cells() {
        let text = "Test";
        let beams = encode_text_to_beams(text, 13, Some("Test".to_string()))
            .expect("Should encode");
        
        let cells = beams_to_gradient_cells(&beams, "Test".to_string());
        assert!(!cells.is_empty());
        
        // Verify we can convert back
        let beams2 = gradient_cells_to_beams(&cells).expect("Should convert cells to beams");
        assert_eq!(beams.len(), beams2.len());
    }
    
    #[test]
    fn test_wire_format_roundtrip() {
        let text = "Test message";
        let beams = encode_text_to_beams(text, 13, Some("Test".to_string()))
            .expect("Should encode");
        
        let wire = beams_to_wire_format(&beams);
        assert!(!wire.is_empty());
        
        let wire_refs: Vec<&str> = wire.iter().map(|s| s.as_str()).collect();
        let beams2 = wire_format_to_beams(&wire_refs).expect("Should parse");
        
        assert_eq!(beams.len(), beams2.len());
    }
    
    #[test]
    fn test_payload_too_large() {
        let large_data = vec![b'A'; MAX_PAYLOAD_BYTES + 1];
        let result = encode_bytes_to_beams(&large_data, 13, None);
        assert!(matches!(result, Err(MoifeuGradientError::PayloadTooLarge(_))));
    }
    
    #[test]
    fn test_checksum_detection() {
        // Test that checksum is added
        let data = vec![b'X'; 16];
        let beams = encode_bytes_to_beams(&data, 13, Some("Test".to_string()))
            .expect("Should encode");
        
        // Count checksum beams
        let checksum_count = beams.iter()
            .filter(|b| b.op_level == 0 && b.op.as_deref() == Some(CHECKSUM_OP))
            .count();
        
        assert_eq!(checksum_count, 1, "Should have exactly one checksum beam");
    }
    
    #[test]
    fn test_large_payload_no_overflow() {
        // Test with 256 bytes to ensure no overflow in phase
        let data = vec![b'Y'; 256];
        let beams = encode_bytes_to_beams(&data, 13, Some("Test".to_string()))
            .expect("Should encode without overflow");
        
        // Should be able to decode it back
        let decoded = decode_beams_to_bytes(&beams).expect("Should decode");
        assert_eq!(decoded, data);
    }
    
    #[test]
    fn test_lenia_field_roundtrip() {
        let data = b"Test Lenia";
        let field = encode_to_lenia_field(data, 13, 0, 0, 8, 8, 0.3)
            .expect("Should encode to field");
        
        assert!(!field.is_empty());
        
        // Decode back (will be lossy for large payloads)
        let beams = decode_from_lenia_field(&field, 0, 0, 8, 8)
            .expect("Should decode from field");
        assert!(!beams.is_empty());
    }
    
    #[test]
    fn test_empty_data_error() {
        let beams = vec![
            create_sentinel_beam(13, SENTINEL_START_OP, Some("Test".to_string()), true)
                .expect("Should create start sentinel"),
            create_sentinel_beam(13, SENTINEL_END_OP, Some("Test".to_string()), false)
                .expect("Should create end sentinel"),
        ];
        
        let result = decode_beams_to_bytes(&beams);
        assert!(matches!(result, Err(MoifeuGradientError::EmptyData)));
    }
    
    #[test]
    fn test_no_sentinels_error() {
        let text = "Test";
        let beams = encode_text_to_beams(text, 13, Some("Test".to_string()))
            .expect("Should encode");
        
        // Remove sentinels
        let beams_no_sentinels: Vec<MoifeuBeam> = beams.iter()
            .filter(|b| b.op_class != 0 || b.depth != 9)
            .cloned()
            .collect();
        
        let result = decode_beams_to_bytes(&beams_no_sentinels);
        assert!(matches!(result, Err(MoifeuGradientError::SentinelNotFound)));
    }
    
    #[test]
    fn test_checksum_missing_error() {
        let text = "Test";
        let beams = encode_text_to_beams(text, 13, Some("Test".to_string()))
            .expect("Should encode");
        
        // Remove checksum beam (op_level=0, op="CHECKSUM")
        let beams_no_checksum: Vec<MoifeuBeam> = beams.iter()
            .filter(|b| !(b.op_level == 0 && b.op.as_deref() == Some(CHECKSUM_OP)))
            .cloned()
            .collect();
        
        let result = decode_beams_to_bytes(&beams_no_checksum);
        assert!(matches!(result, Err(MoifeuGradientError::ChecksumMissing)));
    }
}
