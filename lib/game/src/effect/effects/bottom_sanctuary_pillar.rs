//! EF_BOTTOM_SANC — sustained Sanctuary pillar at the caster's feet.
//! Visible reference: `ro-effects/effects/imgs/300-350/317.gif`.
//!
//! original game launches a single `PP_RECT_UP` primitive whose `m_GI[0]` carries a
//! 4-sided pillar:
//!   * `height[0] = height[1] = 2.5` — square base width
//!   * `max_height = 16` — vertical extent
//!   * `alphaB = 120, alphaT = 1880` — start alpha + fade timing
//!   * `RotStart = 0`, `full_display_angle = random(360)` — initial Y rotation
//!     randomised per spawn
//!   * parent `m_duration` is the master's lifetime (table value is
//!     `99990 ms`), so the pillar is effectively permanent until the
//!     Sanctuary cell dies.
//!
//! We render it as a 4-sided `Frustum` rotating slowly around the vertical
//! axis. The pillar's texture (`alpha_down.tga`) is horizontally banded; the
//! gif's apparent concentric rings come from that banding rotating with the
//! frustum, not from a second primitive.

use crate::effect::draw::{BlendKind, EffectDrawList, EffectPrimitiveDraw, EffectStatus};
use crate::effect::effect_trait::{Effect, EffectRenderCtx, EffectUpdateCtx};

pub const TEXTURE: &str = "alpha_down.tga";
pub const TEXTURES: &[&str] = &[TEXTURE];

const FRAMES_PER_SECOND: f32 = 60.0;
/// Lifetime kept in lockstep with the spec's `duration_ms`; the effect is a
/// "permanent" sustained skill effect on the original client.
pub const TOTAL_DURATION_MS: u32 = 99_990;

/// Square pillar — `sides == 4`.
const SIDES: u32 = 4;
/// Pillar half-extent on the X / Z plane (original game `height[0..2] = 2.5`).
const BASE_RADIUS: f32 = 2.5;
/// original game `max_height` for F1 == 1.
const PILLAR_HEIGHT: f32 = 16.0;
/// `alphaB = 120 / 255` baseline alpha — the pillar holds at this level.
const BASE_ALPHA: f32 = 120.0 / 255.0;
/// Frames to ramp from 0 to BASE_ALPHA at spawn — matches the gif fade-in.
const FADE_IN_FRAMES: f32 = 15.0;
/// Rotation rate, degrees per frame. Picked from the gif (one full revolution
/// every ~120 frames ≈ 2 s of wall-clock spin).
const ROT_DEG_PER_FRAME: f32 = 3.0;

pub struct BottomSanctuaryPillarEffect {
    world_pos: [f32; 3],
    age: f32,
    /// Random initial Y rotation, in radians. Stays constant per instance.
    initial_rotation: f32,
}

impl BottomSanctuaryPillarEffect {
    pub fn new(world_pos: [f32; 3]) -> Self {
        // Cheap deterministic-ish hash of the spawn position so successive
        // spawns at different cells get distinct rotations without pulling in
        // a real RNG dependency.
        let key = (world_pos[0].to_bits() ^ world_pos[2].to_bits()) as f32 * 1.6180339;
        let initial_rotation = key.rem_euclid(std::f32::consts::TAU);
        Self {
            world_pos,
            age: 0.0,
            initial_rotation,
        }
    }
}

impl Effect for BottomSanctuaryPillarEffect {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        self.age += ctx.delta;
        let total_s = TOTAL_DURATION_MS as f32 / 1000.0;
        if self.age >= total_s {
            EffectStatus::Dead
        } else {
            EffectStatus::Running
        }
    }

    fn collect_draws(&self, out: &mut EffectDrawList, _ctx: &EffectRenderCtx) {
        let frame = self.age * FRAMES_PER_SECOND;
        let alpha = BASE_ALPHA * (frame / FADE_IN_FRAMES).clamp(0.0, 1.0);
        let rotation = self.initial_rotation
            + (frame * ROT_DEG_PER_FRAME).to_radians();

        out.push(EffectPrimitiveDraw::Frustum {
            base: self.world_pos,
            bottom_size: BASE_RADIUS,
            top_size: BASE_RADIUS,
            height: PILLAR_HEIGHT,
            sides: SIDES,
            rotation,
            uv_repeat: 1.0,
            uv_scroll: [0.0, 0.0],
            wave_amplitude: 0.0,
            wave_frequency: 0.0,
            wave_phase: 0.0,
            tilt_x_rad: 0.0,
            rotation_y_rad: 0.0,
            cull_back: false,
            texture: TEXTURE,
            color: [1.0, 1.0, 1.0, alpha],
            blend: BlendKind::Alpha,
        });
    }
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

    fn draws(effect: &BottomSanctuaryPillarEffect) -> Vec<EffectPrimitiveDraw> {
        let mut list = EffectDrawList::new();
        effect.collect_draws(&mut list, &render_ctx());
        list.primitives
    }

    fn step(effect: &mut BottomSanctuaryPillarEffect, dt: f32) {
        effect.update(&EffectUpdateCtx { delta: dt, camera_target: None });
    }

    #[test]
    fn emits_a_square_frustum() {
        let mut bs = BottomSanctuaryPillarEffect::new([0.0; 3]);
        step(&mut bs, 0.0);
        match &draws(&bs)[0] {
            EffectPrimitiveDraw::Frustum {
                sides,
                bottom_size,
                top_size,
                height,
                ..
            } => {
                assert_eq!(*sides, 4);
                assert!((bottom_size - top_size).abs() < f32::EPSILON, "cylinder, not cone");
                assert!(*height > 0.0);
            }
            other => panic!("expected Frustum, got {other:?}"),
        }
    }

    #[test]
    fn rotation_advances_over_time() {
        let mut bs = BottomSanctuaryPillarEffect::new([0.0; 3]);
        step(&mut bs, 0.0);
        let r0 = match &draws(&bs)[0] {
            EffectPrimitiveDraw::Frustum { rotation, .. } => *rotation,
            _ => unreachable!(),
        };
        step(&mut bs, 1.0);
        let r1 = match &draws(&bs)[0] {
            EffectPrimitiveDraw::Frustum { rotation, .. } => *rotation,
            _ => unreachable!(),
        };
        assert!(r1 > r0, "rotation advances ({r0} -> {r1})");
    }

    #[test]
    fn alpha_ramps_in_then_holds() {
        let mut bs = BottomSanctuaryPillarEffect::new([0.0; 3]);
        step(&mut bs, 0.0);
        let a0 = match &draws(&bs)[0] {
            EffectPrimitiveDraw::Frustum { color, .. } => color[3],
            _ => unreachable!(),
        };
        step(&mut bs, FADE_IN_FRAMES / FRAMES_PER_SECOND + 0.01);
        let a_peak = match &draws(&bs)[0] {
            EffectPrimitiveDraw::Frustum { color, .. } => color[3],
            _ => unreachable!(),
        };
        assert!(a_peak > a0);
        assert!((a_peak - BASE_ALPHA).abs() < 1e-4, "holds at BASE_ALPHA");
    }

    #[test]
    fn runs_for_full_duration() {
        let mut bs = BottomSanctuaryPillarEffect::new([0.0; 3]);
        let s = bs.update(&EffectUpdateCtx { delta: 1.0, camera_target: None });
        assert!(matches!(s, EffectStatus::Running));
    }
}
