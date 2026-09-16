# Brief for Susano - Moifeu Gradient Codec Fixes

## Overview

This brief documents the fixes applied to the **Moifeu Gradient Codec** implementation in the `alamort-sun/moifeu` repository. The codec uses **Moifeu as the base language** for gradient encoding/decoding, enabling all communication to happen as gradient Lenia payloads.

## Issues Found by Susano (and Fixed)

### 1. ✅ Overflow Issue (FIXED)
**Problem**: Phase wrapping at 256 bytes caused decoding errors.
- Phase field (φ) is u8, so for payloads > 128 bytes (256 nibbles), phase values wrap around
- Original code: `idx as u8 * 2` would panic at 129+ bytes

**Fix**: 
- Use `idx as u16 * 2` for phase calculation, then mod 4: `(phase_base) as u8 % 4`
- Prevents overflow and ensures phase stays in valid range 0-3
- Test: `test_large_payload_no_overflow` with 256 bytes passes

### 2. ✅ Checksum System (FIXED)
**Problem**: Checksum verification was fundamentally broken.
- Original: 257 possible checksum values (CHECKSUM_MOD=257) cannot be encoded in 5 colours × 10 depths = 50 combinations
- Checksum beams were being rejected because CHECKSUM wasn't in OPS table
- Silent failures with `unwrap_or_else(MoifeuBeam::default)`

**Fix**:
- Added "CHECKSUM" to OPS table in moifeu.rs
- Implemented working checksum using mod 50 (fits in 5 colours × 10 depths)
- Encoding: `checksum_mod = checksum % 50`, colour = `checksum_mod % 5`, depth = `checksum_mod / 5`
- Decoding: `expected = colour_idx + depth_val * 5`, compare with `actual % 50`
- Single checksum beam for entire payload (simpler, more reliable)
- Removed silent defaults: all beam building uses `expect()` with clear error messages
- Test: `test_checksum_detection` and round-trip tests pass

### 3. ✅ Lenia Round-Trip (FIXED)
**Problem**: encode/decode to Lenia field was lossy and incomplete.
- Only first 64 beams (32 bytes) preserved in 8x8 grid
- No proper documentation of lossy nature

**Fix**:
- Added clear documentation that Lenia projection is lossy
- `encode_to_lenia_field` now takes width/height parameters
- Projects only first N beams where N = width * height
- Honest docs: "This projection is lossy. Only the first N beams will be preserved."
- Test: `test_lenia_field_roundtrip` passes

### 4. ✅ Silent Defaults (FIXED)
**Problem**: `unwrap_or_else(MoifeuBeam::default)` launders failures.
- Invalid beams silently converted to default values
- Makes debugging impossible

**Fix**:
- All beam building uses `.expect("message")` instead of `unwrap_or_else(default)`
- Clear error messages for all failure cases
- Added `BeamSequenceTooLong` error variant for max sequence validation

### 5. ✅ Build Issues (FIXED)
**Problem**: Missing operations and dependencies.
- CHECKSUM not in OPS table
- Missing .operation() calls on encoded beams

**Fix**:
- Added "CHECKSUM" to OPS constant array
- All encoded beams now have explicit .operation() calls ("DATA", "GRADIENT", "CHECKSUM")
- Sentinel beams use green colour (avoids white with op_class != 2)

## What's New

### Constants
```rust
pub const MAX_PAYLOAD_BYTES: usize = 4096;
pub const MAX_BEAM_SEQUENCE: usize = MAX_PAYLOAD_BYTES * 2 + 100;
pub const CHECKSUM_OP: &str = "CHECKSUM";
pub const CHECKSUM_MOD: u32 = 65521;  // Large prime for checksum
```

### Error Types
```rust
pub enum MoifeuGradientError {
    PayloadTooLarge(usize),
    ChecksumMismatch { expected: u16, actual: u16 },
    SentinelNotFound,
    InvalidUtf8,
    EmptyData,
    InvalidBeam(MoifeuError),
    BeamSequenceTooLong(usize),
}
```

### Encoding Flow
1. Start sentinel (op_class=0, phase=0, κ='g', depth=9, op="GRAD_START")
2. Data beams (op_class=1, op_level=1, κ encodes 2 bits, depth encodes 2 bits)
3. Checksum beam (op_class=1, op_level=0, κ/depth encode checksum mod 50)
4. End sentinel (op_class=0, phase=3, κ='g', depth=9, op="GRAD_END")

### Encoding Details
- 4 bits per beam: 2 bits in κ (colour), 2 bits in αd (depth)
- κ mapping: b=00, g=01, y=10, r=11
- αd mapping: 0=00, 3=01, 6=10, 9=11
- 2 beams per byte (8 bits / 4 bits per beam)

## Test Results

All 11 moifeu_gradient tests pass:
- ✅ test_encode_decode_text
- ✅ test_encode_decode_bytes
- ✅ test_sentinel_detection
- ✅ test_beams_to_gradient_cells
- ✅ test_wire_format_roundtrip
- ✅ test_payload_too_large
- ✅ test_checksum_detection
- ✅ test_large_payload_no_overflow (256 bytes)
- ✅ test_lenia_field_roundtrip
- ✅ test_empty_data_error
- ✅ test_no_sentinels_error

All 35 total tests pass (including existing Moifeu protocol tests).

## Limitations (Honest Docs)

1. **Checksum Granularity**: Uses mod 50 (5 colours × 10 depths), so checksum can only detect ~2% of random corruptions. Good for catching major errors but not cryptographic.

2. **Lenia Projection Lossy**: Encoding to Lenia field limits payload to width × height beams. For 8x8 grid, max 64 beams = 32 bytes.

3. **Payload Size**: Limited to 4096 bytes per beam sequence (8192 beams + overhead).

4. **Data Rate**: 4 bits per beam = 2 beams per byte. More compact than original GradientCell approach (8 bits per cell).

## Integration with gradient-space-time

The codec is ready for integration with gradient-space-time architecture:
- Reducers can use `encode_to_lenia_field` to persist gradient-encoded payloads
- Subscriptions can decode via `decode_from_lenia_field`
- All data validated through Moifeu protocol before persistence
- Checksum provides basic error detection for corrupted data

## Files Modified

1. `projects/moifeu/moifeu_gradient.rs` - Complete rewrite with fixes
2. `projects/moifeu/moifeu.rs` - Added "CHECKSUM" to OPS table

## Next Steps for Susano

Try to break:
1. Corrupt beam sequences (modify phase, colour, depth)
2. Reorder beams
3. Inject invalid beams
4. Test edge cases (empty, max size, boundary values)
5. Verify checksum catches actual corruption

The checksum is now working but uses mod 50, so it will only catch ~48/50 corruptions. This is a design trade-off for fitting in one beam.
