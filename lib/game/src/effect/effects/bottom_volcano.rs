//! Bottom_Volcano family — ground-anchored "blooming flower" ring of
//! tapered ribbon petals.
//!
//! Despite the `BottomVo` / `BottomDe` / `BottomVi` / `BottomSuiton`
//! naming, the original game dispatcher routes all four ids to
//! `Bottom_Volcano`, not `Bottom_Music` — the visible effect is a ring
//! of upright "petals" pulsing outward from the caster's feet.
//!
//! original game `Bottom_Volcano(tName, PW=0)` (RagEffect2.cpp:16797)
//! launches `PP_GI_4` with one active `m_GI[0]` cell seeded:
//!   * `full_display_angle = 360`
//!   * `max_height         = 15`
//!   * `distance           = 2.6`
//!   * `rise_angle         = 80°`
//!   * `RotStart           = random(360)`
//!   * `alphaB             = random(100)`
//!   * `height[i]          = 0` for all i  (integrator fills them)
//!
//! `g_prim->m_size = PW` is stored but neither `PrimGI4` nor
//! `Render3DCasting_LowPolygon` reads it, so Suiton (`PW = 14`)
//! visually equals BottomDe (both blue rings).
//!
//! Per-frame integrator (`PrimGI4`, RagEffect2.cpp:13568):
//!   distance += 0.04                            // bloom outward
//!   rise_angle -= 1 (floor 10)                  // tilt more outward
//!   if distance >= 2.5:
//!       alphaB -= 2
//!       if alphaB <= 0:                         // RESTART cycle
//!           distance   = 2.14                   // 2.5 - 0.36
//!           rise_angle = 74
//!   else:
//!       alphaB += 20                            // fade in fast
//!   for i in 0..E_DIVISION:
//!       SinLimit  = 90 + (i - middle) * m2      // middle=10, m2=9
//!       height[i] = max_height * (1 + sin(SinLimit) * 0.3 * sin(pr))
//!       where pr = (process + ec*90) % 360
//!
//! Render (`Render3DCasting_LowPolygon`, RagEffect2.cpp:6393):
//!   E_DIV = 10 segments around the ring. Per segment, bottom vertex
//!   sits at `(cos*distance, 0, sin*distance)` (GROUND, vec2.y = 0),
//!   top vertex offset radially-out by `cos(rise_angle)*height[order]`
//!   and upward by `sin(rise_angle)*height[order]`. The closed-loop
//!   wrap forces position E_DIV back to position 0's angle.

use crate::effect::draw::{BlendKind, EffectDrawList, EffectPrimitiveDraw, EffectStatus};
use crate::effect::effect_trait::{Effect, EffectRenderCtx, EffectUpdateCtx};
use crate::effect::radial_emitter::{
    RADIAL_EMITTER_DIVISION, RADIAL_EMITTER_SLOTS, RadialEmitter, RadialEmitterSlot,
};

#[derive(Clone, Copy, Debug)]
pub struct BottomVolcanoParams {
    pub texture: &'static str,
}

/// `EF_BOTTOM_VO` (`Bottom_Volcano("ring_red.tga")`).
pub const VOLCANO_RED: BottomVolcanoParams = BottomVolcanoParams {
    texture: "ring_red.tga",
};
/// `EF_BOTTOM_DE` (`Bottom_Volcano("ring_blue.tga")`).
pub const VOLCANO_BLUE: BottomVolcanoParams = BottomVolcanoParams {
    texture: "ring_blue.tga",
};
/// `EF_BOTTOM_VI` (`Bottom_Volcano("magic_green.tga")`).
pub const VOLCANO_GREEN: BottomVolcanoParams = BottomVolcanoParams {
    texture: "magic_green.tga",
};
/// `EF_BOTTOM_SUITON` (`Bottom_Volcano("ring_blue.tga", 14)`).
/// `PW=14` is dead code in original game `PrimGI4` / `Render3DCasting_LowPolygon`,
/// so Suiton visually equals BottomDe (same blue ring).
pub const SUITON: BottomVolcanoParams = BottomVolcanoParams {
    texture: "ring_blue.tga",
};

pub const TEXTURES: &[&str] = &["ring_red.tga", "ring_blue.tga", "magic_green.tga"];

const FRAMES_PER_SECOND: f32 = 60.0;
const BASE_DISTANCE: f32 = 2.6;
const BASE_RISE_ANGLE_DEG: f32 = 80.0;
const MAX_HEIGHT: f32 = 15.0;
/// `Render3DCasting_LowPolygon` slices the ring into 10 quads, not the
/// `E_DIVISION-1 = 20` of `Render3DCasting`.
const E_DIV: u32 = 10;
const SEGMENTS: u32 = E_DIV;
const HEIGHT_SCALE: f32 = 1.0;

/// Bloom geometry constants — distance bounds and rise-angle reset values
/// from `PrimGI4`.
const DISTANCE_GROWTH_PER_FRAME: f32 = 0.04;
const DISTANCE_MAX: f32 = 2.5;
const DISTANCE_RESET: f32 = DISTANCE_MAX - 0.36;
const RISE_ANGLE_FLOOR_DEG: f32 = 10.0;
const RISE_ANGLE_RESET_DEG: f32 = 74.0;

const ALPHA_FADE_IN_PER_FRAME: f32 = 20.0 / 255.0;
const ALPHA_FADE_OUT_PER_FRAME: f32 = 2.0 / 255.0;

/// Petal wobble shape constants. `MIDDLE` and `M2` follow `PrimGI4`'s
/// `middle = (E_DIVISION-1)/2` and `m2 = 90/middle`. The wobble is
/// recomputed every frame, scaled by `sin(pr)` where `pr = process % 360`.
const MIDDLE: f32 = ((RADIAL_EMITTER_DIVISION - 1) / 2) as f32;
const M2: f32 = 90.0 / MIDDLE;
const WOBBLE_AMPLITUDE: f32 = 0.3;

pub struct BottomVolcanoEffect {
    world_pos: [f32; 3],
    params: BottomVolcanoParams,
    age_frames: f32,
    last_processed_frame: u32,
    emitter: RadialEmitter,
}

impl BottomVolcanoEffect {
    pub fn new(world_pos: [f32; 3], params: BottomVolcanoParams) -> Self {
        let hash = position_hash(&world_pos);
        // original game `random(360)` for the per-cast rotation seed.
        let rot_start_deg = (hash % 360) as f32;
        // original game `random(100)` for the initial alpha. Raw byte 0..99
        // maps to `0..0.39` in our 0..1 alpha range.
        let alpha_init = ((hash / 360) % 100) as f32 / 255.0;

        let mut slots = [RadialEmitterSlot::dormant(); RADIAL_EMITTER_SLOTS];
        let mut slot = RadialEmitterSlot::spawn(BASE_DISTANCE, BASE_RISE_ANGLE_DEG, MAX_HEIGHT);
        slot.rot_start_deg = rot_start_deg;
        slot.full_display_angle_deg = 360.0;
        slot.alpha_b = alpha_init;
        // Heights are filled by the first integrator pass; original game leaves
        // them at 0 too, then `PrimGI4` populates them on tick 1.
        slots[0] = slot;

        let mut effect = Self {
            world_pos,
            params,
            age_frames: 0.0,
            last_processed_frame: 0,
            emitter: RadialEmitter::from_slots(slots),
        };
        // Seed heights for frame 0 so the very first draw isn't a flat
        // zero-thickness ribbon.
        effect.update_petal_heights();
        effect
    }

    fn integrate_frames(&mut self, target_frame: u32) {
        while self.last_processed_frame < target_frame {
            self.emitter.tick();
            for slot in self.emitter.slots.iter_mut().filter(|s| s.alive) {
                slot.distance += DISTANCE_GROWTH_PER_FRAME;

                slot.rise_angle_deg -= 1.0;
                if slot.rise_angle_deg < RISE_ANGLE_FLOOR_DEG {
                    slot.rise_angle_deg = RISE_ANGLE_FLOOR_DEG;
                    slot.alpha_b = 0.0;
                }

                if slot.distance >= DISTANCE_MAX {
                    slot.alpha_b -= ALPHA_FADE_OUT_PER_FRAME;
                    if slot.alpha_b <= 0.0 {
                        slot.alpha_b = 0.0;
                        slot.distance = DISTANCE_RESET;
                        slot.rise_angle_deg = RISE_ANGLE_RESET_DEG;
                    }
                } else {
                    slot.alpha_b += ALPHA_FADE_IN_PER_FRAME;
                }
            }
            self.update_petal_heights();
            self.last_processed_frame += 1;
        }
    }

    fn update_petal_heights(&mut self) {
        for (ec, slot) in self.emitter.slots.iter_mut().enumerate() {
            if !slot.alive {
                continue;
            }
            // `pr` is `process + ec*90` mod 360 for ec<2 (the single
            // active slot is ec=0 in our setup).
            let pr_deg = ((slot.process as f32) + (ec as f32) * 90.0).rem_euclid(360.0);
            let pr_sin = pr_deg.to_radians().sin();
            for i in 0..RADIAL_EMITTER_DIVISION {
                let sin_limit_deg = 90.0 + (i as f32 - MIDDLE) * M2;
                let amplitude = sin_limit_deg.to_radians().sin();
                slot.height[i] = slot.max_height
                    + slot.max_height * amplitude * WOBBLE_AMPLITUDE * pr_sin;
            }
        }
    }

    pub fn distance(&self) -> f32 {
        self.emitter.slots[0].distance
    }

    pub fn rise_angle_deg(&self) -> f32 {
        self.emitter.slots[0].rise_angle_deg
    }
}

impl Effect for BottomVolcanoEffect {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        self.age_frames += ctx.delta * FRAMES_PER_SECOND;
        let target = self.age_frames as u32;
        self.integrate_frames(target);
        // Duration (299990 ms in table.rs) is enforced by the holder.
        EffectStatus::Running
    }

    fn collect_draws(&self, out: &mut EffectDrawList, _ctx: &EffectRenderCtx) {
        // Ground anchor: `Render3DCasting_LowPolygon` sets `vec2.y = 0`
        // for the standard `max_height = 15` branch, so the ribbon base
        // sits at the caster's feet — NOT lifted by max_height.
        let center = self.world_pos;
        for (_ec, slot) in self.emitter.active() {
            if slot.alpha_b <= 0.0 {
                continue;
            }
            out.push(EffectPrimitiveDraw::RadialRing {
                center,
                distance: slot.distance,
                rise_angle_rad: slot.rise_angle_deg.to_radians(),
                rot_start_rad: slot.rot_start_deg.to_radians(),
                full_arc_rad: slot.full_display_angle_deg.to_radians(),
                segments: SEGMENTS,
                height_scale: HEIGHT_SCALE,
                heights: slot.height,
                texture: self.params.texture,
                color: [1.0, 1.0, 1.0, slot.alpha_b.clamp(0.0, 1.0)],
                blend: BlendKind::Alpha,
            });
        }
    }
}

fn position_hash(pos: &[f32; 3]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    pos[0].to_bits().hash(&mut h);
    pos[1].to_bits().hash(&mut h);
    pos[2].to_bits().hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_ctx() -> EffectRenderCtx {
        EffectRenderCtx {
            camera: Default::default(),
            screen_w: 800.0,
            screen_h: 600.0,
            elapsed: 0.0,
        }
    }

    fn step(e: &mut BottomVolcanoEffect, frames: f32) -> EffectStatus {
        e.update(&EffectUpdateCtx {
            delta: frames / FRAMES_PER_SECOND,
            camera_target: None, caster_yaw: None,
        })
    }

    fn draws(e: &BottomVolcanoEffect) -> Vec<EffectPrimitiveDraw> {
        let mut list = EffectDrawList::new();
        e.collect_draws(&mut list, &render_ctx());
        list.primitives
    }

    #[test]
    fn volcano_emits_one_ground_anchored_ribbon_with_petal_heights() {
        // Sociable: a freshly spawned BottomVolcano emits a single
        // RadialRing whose ring centre is at the caster's feet (NOT
        // lifted to `-max_height` like Wind), with 10 segments matching
        // `Render3DCasting_LowPolygon`'s E_DIV. Heights are populated by
        // the wobble formula so the first frame is already visible
        // (non-zero somewhere).
        let mut e = BottomVolcanoEffect::new([7.0, 3.5, -2.0], VOLCANO_RED);
        step(&mut e, 1.0);
        let prims = draws(&e);
        assert_eq!(prims.len(), 1);
        match &prims[0] {
            EffectPrimitiveDraw::RadialRing {
                center,
                segments,
                heights,
                texture,
                ..
            } => {
                assert_eq!(*center, [7.0, 3.5, -2.0], "ground-anchored, not lifted");
                assert_eq!(*segments, SEGMENTS);
                assert!(heights.iter().any(|h| *h > 0.0), "petals must be non-zero");
                assert_eq!(*texture, "ring_red.tga");
            }
            other => panic!("expected RadialRing, got {other:?}"),
        }
    }

    #[test]
    fn distance_pulses_via_bloom_then_reset_cycle() {
        // Sociable: PrimGI4's bloom loop must reset the distance once
        // the alpha fade-out completes. Starting at distance=2.6 we are
        // already in the fade-out branch; after ~`alpha_init / 0.0078`
        // frames the alpha hits 0 and distance snaps to ~2.14. Confirm
        // that across a long-enough window the distance both grows and
        // shrinks — i.e. it isn't monotonic.
        let mut e = BottomVolcanoEffect::new([0.0, 0.0, 0.0], VOLCANO_BLUE);
        let initial = e.distance();
        let mut saw_smaller = false;
        let mut saw_larger = false;
        for _ in 0..400 {
            step(&mut e, 1.0);
            let d = e.distance();
            if d < initial - 0.1 {
                saw_smaller = true;
            }
            if d > initial + 0.05 {
                saw_larger = true;
            }
        }
        assert!(
            saw_smaller && saw_larger,
            "distance must pulse (saw_smaller={saw_smaller}, saw_larger={saw_larger}) \
             across the bloom cycle",
        );
    }

    #[test]
    fn rise_angle_decreases_then_floors_or_resets() {
        // Sociable: rise_angle starts at 80° and ticks -1°/frame in
        // PrimGI4; it floors at 10° or resets to 74° at the alpha
        // restart. Either way, after enough frames it must be strictly
        // less than the starting 80°. Confirms the integrator is wired
        // (without it the petals would never lay flat / open up).
        let mut e = BottomVolcanoEffect::new([0.0, 0.0, 0.0], VOLCANO_GREEN);
        let initial = e.rise_angle_deg();
        assert_eq!(initial, BASE_RISE_ANGLE_DEG);
        step(&mut e, 30.0);
        assert!(
            e.rise_angle_deg() < initial,
            "rise_angle must decrease (got {} from {initial})",
            e.rise_angle_deg(),
        );
    }

    #[test]
    fn volcano_does_not_self_terminate() {
        // Holder enforces the 299990 ms duration; the effect itself must
        // keep returning Running even after many bloom cycles.
        let mut e = BottomVolcanoEffect::new([0.0, 0.0, 0.0], VOLCANO_GREEN);
        let mut status = EffectStatus::Running;
        for _ in 0..30 {
            status = step(&mut e, 20.0);
        }
        assert!(matches!(status, EffectStatus::Running));
    }
}
