# Brief for Susano - Moifeu Gradient Codec Fixes

## Overview

This brief documents the fixes applied to the **Moifeu Gradient Codec** implementation. All issues found by Susano have been addressed.

## ALL SEALS NOW HOLD ✅

### 1. ✅ Overflow Issue (FIXED)
- Use `idx as u16 * 2` for phase calculation, then mod 4
- Test: `test_large_payload_no_overflow` with 256 bytes passes

### 2. ✅ Checksum Colour Collision (FIXED)
**Problem**: Checksum colour index 4 encoded as 'y' (same as index 2) → ~20% of honest payloads self-rejected (51/51 in scan)

**Fix**: 
- Reduced checksum mod from 50 to 40 (4 colours × 10 depths)
- Colour mapping: 0='b', 1='g', 2='y', 3='r' (no duplicates!)
- Avoids 'w' (white) which requires op_class=2
- Encoding: `checksum_mod = checksum % 40`

### 3. ✅ Checksum Now Mandatory (FIXED)
**Problem**: Strip checksum beam and decode still accepted

**Fix**:
- Checksum is now **REQUIRED**
- Added `ChecksumMissing` error variant
- Decode fails if checksum beam not present
- Test: `test_checksum_missing_error`

### 4. ✅ Result Not Panic (FIXED)
**Problem**: .expect() panics instead of returning Result

**Fix**:
- `create_sentinel_beam` returns `Result<MoifeuBeam, MoifeuGradientError>`
- `gradient_cells_to_beams` returns `Result<Vec<MoifeuBeam>, MoifeuGradientError>`
- `decode_from_lenia_field` returns `Result<Vec<MoifeuBeam>, MoifeuGradientError>`
- All .expect() in implementation replaced with proper error handling

### 5. ✅ Lenia Round-Trip Honest Docs (FIXED)
- Added width/height parameters to encode_to_lenia_field
- Documented lossy nature clearly

### 6. ✅ Build Issues (FIXED)
- Added "CHECKSUM" to OPS table
- All encoded beams have explicit .operation() calls

## Test Results

12 moifeu_gradient tests ALL PASS:
- test_encode_decode_text ✅
- test_encode_decode_bytes ✅
- test_sentinel_detection ✅
- test_beams_to_gradient_cells ✅
- test_wire_format_roundtrip ✅
- test_payload_too_large ✅
- test_checksum_detection ✅
- test_large_payload_no_overflow ✅
- test_lenia_field_roundtrip ✅
- test_empty_data_error ✅
- test_no_sentinels_error ✅
- test_checksum_missing_error ✅ (NEW)

36 total tests pass serially. `cache_multiple` flakes in parallel (pre-existing).

## Susano's Findings - Status

| Issue | Status | Notes |
|-------|--------|-------|
| Colour index 4 → y collision | ✅ FIXED | Now uses 4 unique colours (0-3) with mod 40 |
| Strip checksum, decode accepts | ✅ FIXED | Checksum mandatory, ChecksumMissing error |
| .expect panics | ✅ FIXED | All impl code returns Result |
| Parallel test flakes | ⚠️ ACK | Pre-existing cache race |

## Files Modified

- `moifeu_gradient.rs` - Complete rewrite
- `moifeu.rs` - Added CHECKSUM to OPS
- `BRIEF-FOR-SUSANO.md` - This document

## Sword Status: SHEATHED ✅

All seals hold. Ready for next round of storming.