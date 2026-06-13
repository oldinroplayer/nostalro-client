//! `BlueCasting` / `DarkCasting` — begin-spell casting rings that differ from
//! the yellow `BeginSpell` only by ring texture and colour.
//!
//! Original game (`RagEffect2.cpp:36`):
//! ```cpp
//! BlueCasting:  SAINTCASTING(45, "effect\\ring_blue.tga",  5);  SAINTCASTING(25, …);
//! DarkCasting:  SAINTCASTING(45, "effect\\ring_black.tga", 6);  SAINTCASTING(25, …);
//! ```
//! Both `F1` values fall through `SAINTCASTING`'s default size table
//! (`max_height ∈ {20, 19, 18, 17}` per emitter — only `F1==2`/`F1==22` differ),
//! so the geometry is byte-for-byte the yellow `BeginSpell` path. The whole
//! 4-emitter cone seed + per-frame bell envelope lives in [`super::saint_casting`].
//!
//! Colour and blend come from the shared vertex-tint table keyed by `m_size`:
//! `F1=5` → size 4 → (100,100,255) **additive** blue glow; `F1=6` → size 12 →
//! (50,50,50) **alpha-blended** — a dark dome that genuinely darkens what's
//! behind it (additive dark would be invisible).

use crate::effect::draw::{BlendKind, EffectDrawList, EffectStatus};
use crate::effect::effect_trait::{Effect, EffectRenderCtx, EffectUpdateCtx};
use crate::effect::effects::saint_casting::{
    SaintCastingConfig, SaintCastingEffect, TOTAL_DURATION_MS as SAINT_TOTAL_DURATION_MS,
};

pub const TOTAL_DURATION_MS: u32 = SAINT_TOTAL_DURATION_MS;

/// Shared default-`F1` size table — every `SAINTCASTING` call except the
/// asura (`F1==2`) and aura-blade (`F1==22`) variants uses it.
const DEFAULT_HEIGHTS: [f32; 4] = [20.0, 19.0, 18.0, 17.0];

/// `EF_BLUECASTING` → `BlueCasting()` → `SAINTCASTING(_, "ring_blue.tga", 5)`.
pub const BLUE: SaintCastingConfig = SaintCastingConfig {
    texture: "ring_blue.tga",
    max_heights: DEFAULT_HEIGHTS,
    color_rgb: [100.0 / 255.0, 100.0 / 255.0, 1.0],
    blend: BlendKind::Additive,
};

/// `EF_DARKCASTING` → `DarkCasting()` → `SAINTCASTING(_, "ring_black.tga", 6)`.
pub const DARK: SaintCastingConfig = SaintCastingConfig {
    texture: "ring_black.tga",
    max_heights: DEFAULT_HEIGHTS,
    color_rgb: [50.0 / 255.0, 50.0 / 255.0, 50.0 / 255.0],
    blend: BlendKind::Alpha,
};

pub const TEXTURES: &[&str] = &["ring_blue.tga", "ring_black.tga"];

pub struct ColorCastingEffect(SaintCastingEffect);

impl ColorCastingEffect {
    pub fn new(world_pos: [f32; 3], cfg: SaintCastingConfig) -> Self {
        Self(SaintCastingEffect::new(world_pos, cfg))
    }
}

impl Effect for ColorCastingEffect {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        self.0.update(ctx)
    }

    fn collect_draws(&self, out: &mut EffectDrawList, ctx: &EffectRenderCtx) {
        self.0.collect_draws(out, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::draw::EffectPrimitiveDraw;

    fn frustums(cfg: SaintCastingConfig) -> Vec<(&'static str, [f32; 4], BlendKind)> {
        let e = ColorCastingEffect::new([0.0; 3], cfg);
        let mut list = EffectDrawList::new();
        e.collect_draws(
            &mut list,
            &EffectRenderCtx {
                camera: Default::default(),
                screen_w: 800.0,
                screen_h: 600.0,
                elapsed: 0.0,
            },
        );
        list.primitives
            .iter()
            .filter_map(|p| match p {
                EffectPrimitiveDraw::Frustum {
                    texture,
                    color,
                    blend,
                    ..
                } => Some((*texture, *color, *blend)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn blue_and_dark_emit_eight_tinted_cones_of_their_own_ring_texture() {
        // Sociable: through SaintCastingEffect — blue is an additive blue
        // glow, dark is an alpha-blended dark-gray dome.
        let blue = frustums(BLUE);
        assert_eq!(blue.len(), 8, "two SAINTCASTING passes × 4 emitters");
        for (tex, color, blend) in &blue {
            assert_eq!(*tex, "ring_blue.tga");
            assert!(color[2] > color[0] && color[2] > 0.9, "blue-dominant tint");
            assert_eq!(*blend, BlendKind::Additive);
        }

        let dark = frustums(DARK);
        assert_eq!(dark.len(), 8);
        for (tex, color, blend) in &dark {
            assert_eq!(*tex, "ring_black.tga");
            assert!(
                color[0] < 0.3 && color[1] < 0.3 && color[2] < 0.3,
                "dark-gray tint, got {color:?}"
            );
            assert_eq!(*blend, BlendKind::Alpha);
        }
    }
}
