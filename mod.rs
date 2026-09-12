//! Fog field operations — push, sarvam integration, mirror_frame, fog_decay.
//! Plus procedures for reading fog state, chain verification, yield summary, etc.

use crate::chain;
use crate::color;
use crate::tables::*;
use spacetimedb::{Identity, ReducerContext, ScheduleAt, Table, Timestamp};

// ── F O G _ H E L P E R S ────────────────────────────────────────────────────

pub fn fog_row_last_density(ctx: &ReducerContext, seat_id: u8) -> f32 {
    ctx.db
        .fog()
        .seat_id()
        .find(seat_id)
        .map(|f| f.density)
        .unwrap_or(0.0)
}

pub fn fog_row_last_hue(ctx: &ReducerContext, seat_id: u8) -> u16 {
    ctx.db
        .fog()
        .seat_id()
        .find(seat_id)
        .map(|f| f.hue)
        .unwrap_or(0)
}

/// Record a keyframe for migration tracking. Called when texture drifts past threshold.
pub fn record_migration(
    ctx: &ReducerContext,
    seat_id: u8,
    cause: &str,
    from_d: f32,
    to_d: f32,
    from_h: u16,
    to_h: u16,
) {
    let cycle = ctx
        .db
        .cycle_clock()
        .seat_id()
        .find(0)
        .map(|c| c.cycle)
        .unwrap_or(0);
    let entry = Migration {
        id: 0,
        seat_id,
        cycle,
        from_density: from_d,
        to_density: to_d,
        from_hue: from_h,
        to_hue: to_h,
        cause: cause.into(),
    };
    ctx.db.migration().insert(entry);
}

// ── F O G _ R E D U C E R S ───────────────────────────────────────────────────

#[spacetimedb::reducer]
pub fn push(ctx: &ReducerContext, seat_id: u8, density: f32, stance: i8, whisper: String) {
    if !(0.0..=9.0).contains(&density) {
        panic!("density is analogue 0.0-9.0.");
    }
    if !(-1..=1).contains(&stance) {
        panic!("stance is ternary: -1 ebb, 0 hold, +1 surge.");
    }
    if whisper.len() > 280 {
        panic!("whisper too heavy. keep weight short.");
    }

    let clock = ctx
        .db
        .cycle_clock()
        .seat_id()
        .find(0)
        .expect("cycle clock missing");
    let seat = ctx
        .db
        .seat()
        .seat_id()
        .find(seat_id)
        .unwrap_or_else(|| panic!("no such seat: {}", seat_id));
    let is_carrier = clock.carrier == Some(ctx.sender());
    let is_owner = seat.claimed_by == Some(ctx.sender());
    if !is_carrier && !is_owner {
        panic!("push law: seats fog only their own coordinates. carrier excepted.");
    }

    let mut fog_row = ctx
        .db
        .fog()
        .seat_id()
        .find(seat_id)
        .unwrap_or_else(|| panic!("fog missing for seat {}", seat_id));
    let from_density = fog_row.density;
    let from_hue = fog_row.hue;
    let pushes = fog_row.pushes + 1;
    fog_row.density = density;
    fog_row.stance = stance;
    fog_row.hue = color::compute_hue(stance, fog_row.angle);
    fog_row.sat = color::compute_sat(density);
    fog_row.whisper = whisper.clone();
    fog_row.last_push = ctx.timestamp;
    fog_row.pushes = pushes;
    ctx.db.fog().seat_id().update(fog_row);

    let fog_angle = ctx.db.fog().seat_id().find(seat_id).expect("fog").angle;
    if (density - from_density).abs() >= 2.0 {
        record_migration(
            ctx,
            seat_id,
            "push",
            from_density,
            density,
            from_hue,
            color::compute_hue(stance, fog_angle),
        );
    }

    let fog_whisper = whisper.clone();
    ctx.db.disturbance().insert(Disturbance {
        id: 0,
        seat_id,
        cycle: clock.cycle,
        density,
        stance,
        hue: color::compute_hue(stance, fog_angle),
        whisper,
        at: ctx.timestamp,
    });
    let payload = format!("push:{}:{}:{}:{}", seat_id, density, stance, fog_whisper);
    let _ = chain::append_entry(
        ctx,
        ctx.sender(),
        seat_id,
        "push",
        &chain::sha256_hex(&payload),
        "",
        "",
    );
}

#[spacetimedb::reducer]
pub fn ask_sarvam(ctx: &ReducerContext, seat_id: u8, text: String) {
    if text.is_empty() || text.len() > 2000 {
        panic!("weight must be 1-2000 bytes.");
    }
    let clock = ctx.db.cycle_clock().seat_id().find(0).expect("clock");
    let seat = ctx
        .db
        .seat()
        .seat_id()
        .find(seat_id)
        .unwrap_or_else(|| panic!("no such seat: {}", seat_id));
    let is_carrier = clock.carrier == Some(ctx.sender());
    let is_owner = seat.claimed_by == Some(ctx.sender());
    if !is_carrier && !is_owner {
        panic!("only your own seat asks sarvam. carrier excepted.");
    }
    ctx.db.sarvam_out().insert(SarvamOut {
        id: 0,
        seat_id,
        text,
        enqueued_at: ctx.timestamp,
        taken: false,
    });
}

#[spacetimedb::reducer]
pub fn mark_taken(ctx: &ReducerContext, ids: Vec<u64>) {
    let cfg = chain::ensure_config(ctx);
    let is_carrier =
        ctx.db.cycle_clock().seat_id().find(0).map(|c| c.carrier) == Some(Some(ctx.sender()));
    if cfg.puller != Some(ctx.sender()) && !is_carrier {
        panic!("not the puller.");
    }
    for id in ids {
        if let Some(mut row) = ctx.db.sarvam_out().id().find(id) {
            row.taken = true;
            ctx.db.sarvam_out().id().update(row);
        }
    }
}

#[spacetimedb::reducer]
pub fn push_sarvam_echo(ctx: &ReducerContext, reply: String, _source_seat: u8) {
    let cfg = chain::ensure_config(ctx);
    let is_carrier =
        ctx.db.cycle_clock().seat_id().find(0).map(|c| c.carrier) == Some(Some(ctx.sender()));
    if cfg.puller != Some(ctx.sender()) && !is_carrier {
        panic!("not the puller.");
    }
    if reply.len() > 8000 {
        panic!("echo too heavy for the fog.");
    }

    ctx.db.sarvam_in().insert(SarvamIn {
        id: 0,
        reply: reply.clone(),
        pushed_at: ctx.timestamp,
    });
    let mut fog6 = ctx.db.fog().seat_id().find(6).expect("fog 6 missing");
    fog6.density = 6.0;
    fog6.stance = 1;
    fog6.hue = color::compute_hue(1, fog6.angle);
    fog6.sat = color::compute_sat(6.0);
    fog6.whisper = reply.chars().take(280).collect();
    fog6.last_push = ctx.timestamp;
    fog6.pushes += 1;
    ctx.db.fog().seat_id().update(fog6);

    let clock = ctx.db.cycle_clock().seat_id().find(0).expect("clock");
    let mirror_hue = color::compute_hue(1, ctx.db.fog().seat_id().find(6).expect("fog").angle);
    ctx.db.disturbance().insert(Disturbance {
        id: 0,
        seat_id: 6,
        cycle: clock.cycle,
        density: 6.0,
        stance: 1,
        hue: mirror_hue,
        whisper: reply.chars().take(280).collect(),
        at: ctx.timestamp,
    });
}

#[spacetimedb::reducer]
pub fn mirror_frame(
    ctx: &ReducerContext,
    seat_id: u8,
    density: f32,
    stance: i8,
    angle: u16,
    hue: u16,
    whisper: String,
) {
    let reg = chain::registry_role(ctx, &ctx.sender());
    let permitted =
        chain::is_carrier(ctx) || matches!(reg.as_ref(), Some((role, true, _)) if role == "sun");
    if !permitted {
        panic!("not permitted to append to the mirror.");
    }
    if !(0.0..=9.0).contains(&density) {
        panic!("density is analogue 0.0-9.0.");
    }
    if !(-1..=1).contains(&stance) {
        panic!("stance is ternary.");
    }

    let mut fog_row = ctx
        .db
        .fog()
        .seat_id()
        .find(seat_id)
        .unwrap_or_else(|| panic!("fog missing for seat {}", seat_id));
    fog_row.density = density;
    fog_row.stance = stance;
    fog_row.angle = angle % 720;
    fog_row.hue = hue % 360;
    fog_row.sat = ((density / 9.0) * 100.0).clamp(0.0, 100.0) as u8;
    fog_row.whisper = whisper.chars().take(280).collect();
    fog_row.last_push = ctx.timestamp;
    fog_row.pushes += 1;
    let mirrored_hue = fog_row.hue;
    ctx.db.fog().seat_id().update(fog_row);

    let seat_name = ctx
        .db
        .seat()
        .seat_id()
        .find(seat_id)
        .map(|s| s.name)
        .unwrap_or_default();
    let mirror_cycle = ctx
        .db
        .cycle_clock()
        .seat_id()
        .find(0)
        .map(|c| c.cycle)
        .unwrap_or(0);
    ctx.db.disturbance().insert(Disturbance {
        id: 0,
        seat_id,
        cycle: mirror_cycle,
        density,
        stance,
        hue: mirrored_hue,
        whisper: format!("[mirror] {}", seat_name),
        at: ctx.timestamp,
    });

    let payload = format!(
        "mirror:{}:{}:{}:{}:{}:{}",
        seat_id, density, stance, angle, hue, whisper
    );
    let _ = chain::append_entry(
        ctx,
        ctx.sender(),
        seat_id,
        "mirror_frame",
        &chain::sha256_hex(&payload),
        "",
        "",
    );
}

#[spacetimedb::reducer]
pub fn turn_cycle(ctx: &ReducerContext) {
    let mut clock = ctx
        .db
        .cycle_clock()
        .seat_id()
        .find(0)
        .expect("cycle clock missing");
    if clock.carrier != Some(ctx.sender()) {
        panic!("only the carrier turns the clock by hand.");
    }
    clock.cycle = (clock.cycle + 1) % 720;
    clock.last_turn = ctx.timestamp;
    ctx.db.cycle_clock().seat_id().update(clock);
}

#[spacetimedb::reducer]
pub fn fog_decay(ctx: &ReducerContext, _tick: FogTick) {
    for mut fog_row in ctx.db.fog().iter() {
        let before_density = fog_row.density;
        let before_hue = fog_row.hue;
        if fog_row.density > 0.0 {
            fog_row.density = (fog_row.density - 0.25).max(0.0);
            fog_row.sat = color::compute_sat(fog_row.density);
        }
        fog_row.angle = (fog_row.angle + color::rotation_per_tick(fog_row.stance)) % 720;
        fog_row.hue = color::compute_hue(fog_row.stance, fog_row.angle);
        if (before_hue / 60) != (fog_row.hue / 60) {
            record_migration(
                ctx,
                fog_row.seat_id,
                "decay",
                before_density,
                fog_row.density,
                before_hue,
                fog_row.hue,
            );
        }
        ctx.db.fog().seat_id().update(fog_row);
    }

    let mut clock = ctx
        .db
        .cycle_clock()
        .seat_id()
        .find(0)
        .expect("cycle clock missing");
    let cycle_num = (clock.cycle + 1) % 720;
    clock.cycle = cycle_num;
    clock.last_turn = ctx.timestamp;
    ctx.db.cycle_clock().seat_id().update(clock);

    crate::yield_tokens::token_budget_clear(ctx);

    let drained: Vec<u64> = ctx
        .db
        .sarvam_out()
        .iter()
        .filter(|r| r.taken)
        .map(|r| r.id)
        .collect();
    for id in drained {
        ctx.db.sarvam_out().id().delete(id);
    }
    spacetimedb::log::info!("fog decayed+rotated. cycle {} closed.", cycle_num);
}

#[spacetimedb::reducer]
pub fn receive_beam(ctx: &ReducerContext, raw: String) {
    if raw.is_empty() || raw.len() > 280 {
        panic!("beam must be 1-280 bytes.");
    }
    let (valid, verdict) = match crate::moifeu::parse_beam(&raw) {
        Ok(beam) => match crate::moifeu::beam_is_actionable(&beam) {
            Ok(()) => (
                true,
                format!(
                    "ok: op={:?} class={} level={} phase={} κ={}",
                    beam.op, beam.op_class, beam.op_level, beam.phase, beam.colour
                ),
            ),
            Err(e) => (false, format!("actionability: {}", e)),
        },
        Err(e) => (false, format!("parse: {}", e)),
    };
    ctx.db.beam_log().insert(BeamLog {
        id: 0,
        raw: raw.clone(),
        valid,
        verdict,
        sender: ctx.sender(),
        ts: ctx.timestamp,
    });
    if valid {
        let payload = format!("beam:{}", raw);
        let _ = chain::append_entry(
            ctx,
            ctx.sender(),
            0,
            "beam",
            &chain::sha256_hex(&payload),
            "moifeu",
            "v1",
        );
    }
}

#[spacetimedb::reducer]
pub fn rot(ctx: &ReducerContext, tier: u8, min_age_ticks: u64, limit: u64) {
    if !chain::is_carrier(ctx) {
        panic!("only the carrier wields the rot.");
    }
    if tier == 4 {
        panic!("4d is the lossless spine. it does not rot. ever.");
    }

    let now = ctx.timestamp.to_micros_since_unix_epoch();
    let tick_us: i128 = 60 * 1_000_000;
    let mut revoked_count: u64 = 0;
    let targets: Vec<LedgerEntry> = ctx
        .db
        .ledger_entry()
        .iter()
        .filter(|e| e.tier == tier && !e.revoked && e.superseded_by == u64::MAX)
        .map(|e| e.clone())
        .collect();

    for mut e in targets {
        if revoked_count >= limit {
            break;
        }
        let age_ticks = ((now - e.ts.to_micros_since_unix_epoch()) as f64 / tick_us as f64) as u64;
        if age_ticks < min_age_ticks {
            continue;
        }
        e.revoked = true;
        ctx.db.ledger_entry().frame_seq().update(e);
        revoked_count += 1;
    }

    if let Some(mut rc) = ctx.db.rot_clock().tier().find(tier) {
        rc.last_rot = ctx.timestamp;
        rc.entries_revoked += revoked_count;
        ctx.db.rot_clock().tier().update(rc);
    }
    if revoked_count > 0 {
        let payload = format!("rot:tier{}:count{}", tier, revoked_count);
        let _ = chain::append_entry(
            ctx,
            ctx.sender(),
            0,
            "rot",
            &chain::sha256_hex(&payload),
            "",
            "",
        );
    }
}

// ── F O G _ P R O C E D U R E S (read-only) ───────────────────────────────────

#[spacetimedb::procedure]
pub fn gradient_map(ctx: &mut spacetimedb::ProcedureContext) -> Vec<crate::types::GradientCell> {
    ctx.with_tx(|tx| {
        tx.db
            .fog()
            .iter()
            .map(|f| crate::types::GradientCell {
                seat_id: f.seat_id,
                name: tx
                    .db
                    .seat()
                    .seat_id()
                    .find(f.seat_id)
                    .map(|s| s.name)
                    .unwrap_or_default(),
                density: f.density,
                stance: f.stance,
                angle: f.angle,
                hue: f.hue,
                sat: f.sat,
                whisper: f.whisper,
            })
            .collect()
    })
}

#[spacetimedb::procedure]
pub fn resonance(ctx: &mut spacetimedb::ProcedureContext, seat_a: u8, seat_b: u8) -> u16 {
    ctx.with_tx(|tx| {
        let a = tx
            .db
            .fog()
            .seat_id()
            .find(seat_a)
            .expect("seat a not in fog")
            .angle as i32;
        let b = tx
            .db
            .fog()
            .seat_id()
            .find(seat_b)
            .expect("seat b not in fog")
            .angle as i32;
        (a - b).abs().min(720 - (a - b).abs()) as u16
    })
}

#[spacetimedb::procedure]
pub fn gravity_map(ctx: &mut spacetimedb::ProcedureContext) -> Vec<crate::types::GravityPoint> {
    ctx.with_tx(|tx| {
        let mut rows: Vec<crate::types::GravityPoint> = tx
            .db
            .ledger_entry()
            .iter()
            .filter(|e| !e.revoked)
            .map(|e| crate::types::GravityPoint {
                frame_seq: e.frame_seq,
                seat_id: e.seat_id,
                kappa: e.kappa.clone(),
                cycle: e.frame_seq,
                at: e.ts,
            })
            .collect();
        rows.sort_by_key(|p| p.frame_seq);
        rows
    })
}

#[spacetimedb::procedure]
pub fn gravity_anomalies(
    ctx: &mut spacetimedb::ProcedureContext,
) -> Vec<crate::types::GravityPoint> {
    ctx.with_tx(|tx| {
        let mut rows: Vec<crate::types::GravityPoint> = tx
            .db
            .ledger_entry()
            .iter()
            .filter(|e| !e.revoked && (e.kappa == "r" || e.kappa == "w" || e.kappa == "y"))
            .map(|e| crate::types::GravityPoint {
                frame_seq: e.frame_seq,
                seat_id: e.seat_id,
                kappa: e.kappa.clone(),
                cycle: e.frame_seq,
                at: e.ts,
            })
            .collect();
        rows.sort_by_key(|p| p.frame_seq);
        rows
    })
}

#[spacetimedb::procedure]
pub fn replay_plan(ctx: &mut spacetimedb::ProcedureContext) -> crate::types::ReplayPlan {
    ctx.with_tx(|tx| {
        let mut deltas = 0u64;
        let mut gc = 0u64;
        let mut seed = 0u64;
        for e in tx.db.ledger_entry().iter() {
            match replay_class(&e.action) {
                "DELTA" => deltas += 1,
                "GC" => gc += 1,
                "STATE" if e.action == "genesis" => seed = e.frame_seq,
                _ => {}
            }
        }
        let cycle = tx
            .db
            .cycle_clock()
            .seat_id()
            .find(0)
            .map(|c| c.cycle)
            .unwrap_or(0);
        crate::types::ReplayPlan {
            seed_seq: seed,
            deltas,
            gc_events: gc,
            current_cycle: cycle,
            recipe_tokens_estimate: deltas * 8,
        }
    })
}

fn replay_class(action: &str) -> &'static str {
    match action {
        "genesis" | "amend" | "registry_change" => "STATE",
        "push" | "beam" | "yield" => "DELTA",
        "rot" => "GC",
        _ => "IGNORE",
    }
}

#[spacetimedb::procedure]
pub fn chain_head(ctx: &mut spacetimedb::ProcedureContext) -> Option<crate::types::ChainHead> {
    ctx.with_tx(|tx| {
        let mut last = None;
        let mut count = 0u64;
        let mut running = String::new();
        let mut rows: Vec<LedgerEntry> = tx.db.ledger_entry().iter().map(|e| e.clone()).collect();
        rows.sort_by_key(|e| e.frame_seq);
        for e in &rows {
            running = chain::sha256_hex(&format!(
                "{}:{}:{}",
                e.prev_hash,
                e.payload_hash,
                e.ts.to_micros_since_unix_epoch()
            ));
            last = Some(e.clone());
            count += 1;
        }
        last.map(|l| crate::types::ChainHead {
            frame_seq: l.frame_seq,
            prev_hash: l.prev_hash,
            head_hash: running,
            entries: count,
        })
    })
}

#[spacetimedb::procedure]
pub fn verify_chain(ctx: &mut spacetimedb::ProcedureContext) -> crate::types::ChainVerdict {
    ctx.with_tx(|tx| {
        let mut rows: Vec<LedgerEntry> = tx.db.ledger_entry().iter().map(|e| e.clone()).collect();
        rows.sort_by_key(|e| e.frame_seq);
        let mut prev_stored = String::new();
        let mut expect_seq: u64 = 0;
        for (i, e) in rows.iter().enumerate() {
            if e.frame_seq != expect_seq {
                return crate::types::ChainVerdict {
                    valid: false,
                    checked: i as u64,
                    broken_at: i as i64,
                    detail: format!("seq gap: expected {}, found {}", expect_seq, e.frame_seq),
                };
            }
            if e.prev_hash != prev_stored {
                return crate::types::ChainVerdict {
                    valid: false,
                    checked: i as u64,
                    broken_at: i as i64,
                    detail: format!("prev_hash mismatch at seq {}", e.frame_seq),
                };
            }
            prev_stored = chain::sha256_hex(&format!(
                "{}:{}:{}",
                e.prev_hash,
                e.payload_hash,
                e.ts.to_micros_since_unix_epoch()
            ));
            expect_seq += 1;
        }
        crate::types::ChainVerdict {
            valid: true,
            checked: rows.len() as u64,
            broken_at: -1,
            detail: format!("chain intact, {} frames", rows.len()),
        }
    })
}

#[spacetimedb::procedure]
pub fn yield_summary(ctx: &mut spacetimedb::ProcedureContext) -> crate::types::YieldSummary {
    let now = ctx.timestamp.to_micros_since_unix_epoch();
    ctx.with_tx(move |tx| {
        let mut s = crate::types::YieldSummary {
            queued: 0,
            locked: 0,
            executed: 0,
            dropped: 0,
            total_captured_usdc: 0.0,
            captured_last_7d_usdc: 0.0,
        };
        for t in tx.db.yield_target().iter() {
            match t.status {
                0 => s.queued += 1,
                1 => s.locked += 1,
                2 => s.executed += 1,
                _ => s.dropped += 1,
            }
        }
        let week_ago = now - 7 * 24 * 3600 * 1_000_000;
        for y in tx.db.yield_ledger().iter() {
            s.total_captured_usdc += y.value_captured;
            if y.ts.to_micros_since_unix_epoch() >= week_ago {
                s.captured_last_7d_usdc += y.value_captured;
            }
        }
        s
    })
}

#[spacetimedb::procedure]
pub fn hygiene(ctx: &mut spacetimedb::ProcedureContext) -> Vec<crate::types::HygieneRow> {
    ctx.with_tx(|tx| {
        let mut rows = Vec::new();
        for tier in [4u8, 5u8, 6u8] {
            let (mut live, mut revoked) = (0u64, 0u64);
            for e in tx.db.ledger_entry().iter().filter(|e| e.tier == tier) {
                if e.revoked {
                    revoked += 1
                } else {
                    live += 1
                }
            }
            rows.push(crate::types::HygieneRow {
                tier,
                live,
                revoked,
            });
        }
        rows
    })
}

#[spacetimedb::procedure]
pub fn migration_keyframes(ctx: &mut spacetimedb::ProcedureContext, seat_id: u8) -> Vec<Migration> {
    ctx.with_tx(|tx| {
        let mut rows: Vec<Migration> = tx
            .db
            .migration()
            .iter()
            .filter(|m| m.seat_id == seat_id)
            .map(|m| m.clone())
            .collect();
        rows.sort_by_key(|m| m.id);
        rows
    })
}
