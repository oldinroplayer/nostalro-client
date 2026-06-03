//! `EF_GUMGANG` family — Steel-Body / Fury electric-arc wreaths.
//!
//! Reference gifs: `200-250/203.gif` (Gumgang), `250-300/254.gif` (Steelbody),
//! `450-500/455.gif` (Gumgangnpc), `400-450/418.gif` (Doublegumgang),
//! `450-500/485.gif` / `486.gif` (Doublegumgang2/3).
//!
//! The original game's `GumGang(F1, F2)` launches `PP_GUMGANG` and
//! `DoubleGumGang(2, F2)` launches `PP_DOUBLEGUMGANG`. Both draw an orbiting
//! wreath of jagged lightning-arc billboards around the caster: `F1` emitters
//! (1 / 2 / 4), each owning four independent "ring" streaks. Each streak is a
//! **fixed-size** camera-facing square (the original game's `Tei_TowardScreen`
//! orients a square whose corners sit at radius `distance` in the diagonal
//! directions) textured with one of `super1.bmp`..`super5.bmp` (128×128 white
//! bolts on a magenta colour-key) and tinted by the colour variant. The quads
//! are large relative to their tight orbit, so the 4–16 of them **overlap into
//! one continuous lightning mass** rather than reading as separate sprites.
//!
//! Each streak runs the original `PrimGumGang` pulse, which drives the quad's
//! **alpha**, not its size: a counter increments each frame; while it is below
//! the grow window the pulse value ramps up (fade in), then it ramps down (fade
//! out), and once it drops below a floor the streak respawns at a fresh random
//! orbital position and texture. `RenderTeiRect` takes this pulse value as the
//! quad alpha. The four rings start at staggered pulse values (`70/130/190/250`,
//! or `40/80/120/160` for the Double family) so they desynchronise into a
//! continuous shimmer rather than blinking as one.
//!
//! `DoubleGumGang` differs in: a tighter orbit, smaller rings, and each streak
//! drawn as three concentric colour layers (`Rdist` 1.0 / 1.2 / 2.0) that
//! darken outward, giving the arcs a halo of depth.
//!
//! Colour variants are taken from the reference gifs (which outrank the source):
//! Gumgang/Steelbody blue, Gumgangnpc red; Doublegumgang red, Doublegumgang2
//! white, Doublegumgang3 blue.
//!
//! Lifetime: the buff auras are persistent (cleared by the server); only the
//! NPC cast (`Gumgangnpc`) is finite.

use crate::effect::draw::{BlendKind, EffectDrawList, EffectPrimitiveDraw, EffectStatus};
use crate::effect::effect_trait::{Effect, EffectRenderCtx, EffectUpdateCtx};

pub const TEXTURES: &[&str] = &[
    "super1.bmp",
    "super2.bmp",
    "super3.bmp",
    "super4.bmp",
    "super5.bmp",
];

const FRAMES_PER_SECOND: f32 = 60.0;

/// Persistent buffs use a large sentinel; the queue keeps them alive until the
/// server clears the status (matching the level-99 floor aura convention).
const PERSISTENT_DURATION_MS: u32 = 999_990;

const RINGS_PER_EMITTER: usize = 4;

/// Alpha pulse state machine (the original game's `PrimGumGang`). The pulsing
/// `height[1]` value is passed to `RenderTeiRect` as the quad **alpha** — the
/// quad itself is a fixed-size square, so this drives a flicker, not growth.
/// `counter` (`height[0]`) starts at 10 so a streak begins by fading *out*,
/// then ramps `0 → ~250` over `GROW_FRAMES`, then fades back down and respawns.
const GROW_FRAMES: f32 = 10.0;
const GROW_PER_FRAME: f32 = 25.0;
const SHRINK_PER_FRAME: f32 = 12.0;
const RESPAWN_BELOW: f32 = 12.0;
const COUNTER_INIT: f32 = 10.0;
/// Peak pulse value → fully opaque.
const ALPHA_DIVISOR: f32 = 255.0;
/// Skip a quad whose pulse alpha is negligible.
const MIN_ALPHA: f32 = 1.0 / ALPHA_DIVISOR;

const SQRT2: f32 = std::f32::consts::SQRT_2;

/// The four base corners sit at radius `distance·Rdist` in the diagonal
/// directions, so the camera-facing square's side is `√2·distance·Rdist`.
/// `ORBIT_LENGTH` (`length`) is how far each quad is pushed from the caster,
/// and the base sits `Y_BASE..Y_BASE+Y_RAND` above the ground (native RO:
/// negative y = up). These are the original game's literal world units —
/// `gumgang2`/`volcano` from the same source render 1:1 at this viewer's scale.
const ORBIT_LENGTH: f32 = 4.0;
const Y_BASE: f32 = 3.0;
const Y_RAND: f32 = 8.0;

/// Uniform dhxj-unit → viewer-unit conversion. The original game's effect
/// coordinates are plain world units (its render matrix is just the view
/// matrix, no scale), but our world is smaller per character: the already
/// character-calibrated `gumgang2`/`volcano` effects port the original
/// distances at ~0.7× (e.g. dhxj `1/2/3/4` → `1.0/1.6/2.2/2.8`). We apply the
/// same factor so the wreath hugs the caster. All source *ratios* (orbit : quad
/// : lift) are preserved exactly; only the absolute size is converted.
const WORLD_SCALE: f32 = 0.7;

/// Alpha ramp-in so the wreath doesn't pop in.
const FADE_IN_FRAMES: f32 = 16.0;

#[derive(Clone, Copy, Debug)]
pub struct GumGangLayer {
    /// Orbit-radius multiplier (the original game's `Rdist`).
    pub rdist: f32,
    /// Additive tint, normalised (the original game's per-layer `r,g,b`).
    pub color_rgb: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct GumGangParams {
    pub emitters: u8,
    /// Quad base half-diagonal, world units (the original game's `distance`).
    /// The camera-facing square's side is `√2·distance·Rdist`.
    pub distance: f32,
    /// Staggered initial pulse (alpha) value of the four rings.
    pub ring_init: [f32; RINGS_PER_EMITTER],
    /// One layer for `PP_GUMGANG`, three concentric layers for `PP_DOUBLEGUMGANG`.
    pub layers: &'static [GumGangLayer],
    /// `None` = persistent buff; `Some(ms)` = finite (NPC cast).
    pub lifetime_ms: Option<u32>,
}

impl GumGangParams {
    pub const fn total_duration_ms(&self) -> u32 {
        match self.lifetime_ms {
            Some(ms) => ms,
            None => PERSISTENT_DURATION_MS,
        }
    }
}

const fn norm(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

const GUMGANG_RINGS: [f32; RINGS_PER_EMITTER] = [70.0, 130.0, 190.0, 250.0];
const DOUBLE_RINGS: [f32; RINGS_PER_EMITTER] = [40.0, 80.0, 120.0, 160.0];

const BLUE_SINGLE: &[GumGangLayer] = &[GumGangLayer {
    rdist: 1.0,
    color_rgb: norm(100, 100, 255),
}];
const RED_SINGLE: &[GumGangLayer] = &[GumGangLayer {
    rdist: 1.0,
    color_rgb: norm(255, 20, 20),
}];

const RED_TRIPLE: &[GumGangLayer] = &[
    GumGangLayer { rdist: 1.0, color_rgb: norm(255, 100, 100) },
    GumGangLayer { rdist: 1.2, color_rgb: norm(120, 0, 0) },
    GumGangLayer { rdist: 2.0, color_rgb: norm(120, 0, 0) },
];
const WHITE_TRIPLE: &[GumGangLayer] = &[
    GumGangLayer { rdist: 1.0, color_rgb: norm(90, 90, 80) },
    GumGangLayer { rdist: 1.2, color_rgb: norm(60, 60, 50) },
    GumGangLayer { rdist: 2.0, color_rgb: norm(30, 30, 20) },
];
const BLUE_TRIPLE: &[GumGangLayer] = &[
    GumGangLayer { rdist: 1.0, color_rgb: norm(100, 100, 255) },
    GumGangLayer { rdist: 1.2, color_rgb: norm(0, 0, 120) },
    GumGangLayer { rdist: 2.0, color_rgb: norm(0, 0, 120) },
];

/// #203 `GumGang(1)` — blue, single emitter.
pub const GUMGANG: GumGangParams = GumGangParams {
    emitters: 1,
    distance: 3.5,
    ring_init: GUMGANG_RINGS,
    layers: BLUE_SINGLE,
    lifetime_ms: None,
};
/// #254 `GumGang(4)` — blue, dense four-emitter corona.
pub const STEELBODY: GumGangParams = GumGangParams {
    emitters: 4,
    distance: 3.5,
    ring_init: GUMGANG_RINGS,
    layers: BLUE_SINGLE,
    lifetime_ms: None,
};
/// #455 `GumGang(2, 1)` — red, two emitters, finite NPC cast (90 frames).
pub const GUMGANGNPC: GumGangParams = GumGangParams {
    emitters: 2,
    distance: 3.5,
    ring_init: GUMGANG_RINGS,
    layers: RED_SINGLE,
    lifetime_ms: Some(1500),
};
/// #418 `DoubleGumGang(2, 2)` — red triple-layer.
pub const DOUBLE_RED: GumGangParams = GumGangParams {
    emitters: 2,
    distance: 2.0,
    ring_init: DOUBLE_RINGS,
    layers: RED_TRIPLE,
    lifetime_ms: None,
};
/// #485 `DoubleGumGang(2, 1)` — white triple-layer.
pub const DOUBLE_WHITE: GumGangParams = GumGangParams {
    emitters: 2,
    distance: 2.0,
    ring_init: DOUBLE_RINGS,
    layers: WHITE_TRIPLE,
    lifetime_ms: None,
};
/// #486 `DoubleGumGang(2, 0)` — blue triple-layer.
pub const DOUBLE_BLUE: GumGangParams = GumGangParams {
    emitters: 2,
    distance: 2.0,
    ring_init: DOUBLE_RINGS,
    layers: BLUE_TRIPLE,
    lifetime_ms: None,
};

/// Small deterministic LCG so respawns scatter without a `rand` dependency.
fn lcg(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *state
}

/// Uniform `[0, max)` from the streak's own generator.
fn rand_range(state: &mut u32, max: f32) -> f32 {
    ((lcg(state) >> 8) as f32 / (1u32 << 24) as f32) * max
}

struct Streak {
    rng: u32,
    /// Pulse counter, in frames (the original game's `height[i]`).
    counter: f32,
    /// Pulse value driving the quad alpha (the original game's `height[i+1]`).
    pulse: f32,
    /// Orbital position angle, radians (the original game's `height[i+3]`).
    angle: f32,
    /// Vertical offset (world units, negative = up; `vec*.y`).
    y_off: f32,
    /// Index into [`TEXTURES`] (the original game's `height[i+2]`).
    tex: usize,
}

impl Streak {
    fn new(seed: u32, init_pulse: f32) -> Self {
        let mut s = Self {
            rng: seed,
            counter: COUNTER_INIT,
            pulse: init_pulse,
            angle: 0.0,
            y_off: 0.0,
            tex: 0,
        };
        s.scatter();
        s
    }

    /// Pick a fresh random orbit angle, texture and vertical offset.
    fn scatter(&mut self) {
        self.angle = rand_range(&mut self.rng, std::f32::consts::TAU);
        self.y_off = -(Y_BASE + rand_range(&mut self.rng, Y_RAND));
        self.tex = (rand_range(&mut self.rng, TEXTURES.len() as f32) as usize).min(TEXTURES.len() - 1);
    }

    fn respawn(&mut self) {
        self.counter = 0.0;
        self.pulse = 0.0;
        self.scatter();
    }

    fn tick(&mut self, delta_frames: f32) {
        self.counter += delta_frames;
        if self.counter < GROW_FRAMES {
            self.pulse += GROW_PER_FRAME * delta_frames;
        } else {
            self.pulse -= SHRINK_PER_FRAME * delta_frames;
            if self.pulse < RESPAWN_BELOW {
                self.respawn();
            }
        }
    }
}

pub struct GumGangEffect {
    params: GumGangParams,
    world_pos: [f32; 3],
    age_frames: f32,
    streaks: Vec<Streak>,
}

impl GumGangEffect {
    pub fn new(world_pos: [f32; 3], params: GumGangParams) -> Self {
        let mut streaks = Vec::with_capacity(params.emitters as usize * RINGS_PER_EMITTER);
        for ec in 0..params.emitters as usize {
            for ring in 0..RINGS_PER_EMITTER {
                let idx = ec * RINGS_PER_EMITTER + ring;
                // Distinct, deterministic seed per streak.
                let seed = (idx as u32).wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9);
                streaks.push(Streak::new(seed, params.ring_init[ring]));
            }
        }
        Self {
            params,
            world_pos,
            age_frames: 0.0,
            streaks,
        }
    }
}

fn alpha_at(frame: f32) -> f32 {
    (frame / FADE_IN_FRAMES).clamp(0.0, 1.0)
}

/// Whether a streak's texture should be flipped horizontally so its bolt always
/// curves *inward*: a quad on the screen-left has its spikes facing right, one
/// on the screen-right has them facing left (the reference's symmetric wreath).
///
/// `on_screen_right` is the sign of the orbit offset projected onto the camera's
/// right vector. The five `super*.bmp` bolts split into two handedness groups
/// (the original game's texture-index parity 0/2 vs 1/3/4): textures whose
/// spikes already point right when unflipped need flipping only on the right
/// side, and vice-versa. `true` = flip the quad's U.
fn mirror_texture(on_screen_right: bool, tex: usize) -> bool {
    let spikes_right_unflipped = tex == 0 || tex == 2;
    // Want spikes pointing inward: left side → right, right side → left.
    let want_spikes_right = !on_screen_right;
    want_spikes_right != spikes_right_unflipped
}

impl Effect for GumGangEffect {
    fn update(&mut self, ctx: &EffectUpdateCtx) -> EffectStatus {
        let delta_frames = ctx.delta * FRAMES_PER_SECOND;
        self.age_frames += delta_frames;
        for streak in &mut self.streaks {
            streak.tick(delta_frames);
        }

        if let Some(ms) = self.params.lifetime_ms {
            let total_frames = ms as f32 / 1000.0 * FRAMES_PER_SECOND;
            if self.age_frames >= total_frames {
                return EffectStatus::Dead;
            }
        }
        EffectStatus::Running
    }

    fn collect_draws(&self, out: &mut EffectDrawList, ctx: &EffectRenderCtx) {
        let fade = alpha_at(self.age_frames);
        if fade <= 0.0 {
            return;
        }
        // Camera right vector (horizontal), used to decide whether each quad
        // sits on the screen-left or screen-right of the caster so its bolt can
        // be flipped to curve inward (see `mirror_texture`).
        let cam = &ctx.camera;
        let fwd = [
            cam.target[0] - cam.eye[0],
            cam.target[1] - cam.eye[1],
            cam.target[2] - cam.eye[2],
        ];
        let right = [
            fwd[1] * cam.up[2] - fwd[2] * cam.up[1],
            fwd[2] * cam.up[0] - fwd[0] * cam.up[2],
            fwd[0] * cam.up[1] - fwd[1] * cam.up[0],
        ];

        let [wx, wy, wz] = self.world_pos;
        for streak in &self.streaks {
            let alpha = (streak.pulse / ALPHA_DIVISOR).clamp(0.0, 1.0) * fade;
            if alpha < MIN_ALPHA {
                continue;
            }
            // Quad centre: the orbit offset (`vec*`), shared by every layer —
            // only the quad size scales with `Rdist`.
            let (sin_a, cos_a) = streak.angle.sin_cos();
            let pos = [
                wx + cos_a * ORBIT_LENGTH * WORLD_SCALE,
                wy + streak.y_off * WORLD_SCALE,
                wz + sin_a * ORBIT_LENGTH * WORLD_SCALE,
            ];
            // Project the orbit offset onto the camera right vector (y = 0).
            let on_screen_right = cos_a * right[0] + sin_a * right[2] > 0.0;
            let uv = if mirror_texture(on_screen_right, streak.tex) {
                // Horizontal flip (the original game's vec1↔vec2 / vec3↔vec4 swap).
                [[1.0, 0.0], [0.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
            } else {
                [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]
            };
            for layer in self.params.layers {
                let side = SQRT2 * self.params.distance * layer.rdist * WORLD_SCALE;
                let [cr, cg, cb] = layer.color_rgb;
                out.push(EffectPrimitiveDraw::Billboard {
                    pos,
                    size: [side, side],
                    uv,
                    rotation: 0.0,
                    texture: TEXTURES[streak.tex],
                    color: [cr, cg, cb, alpha],
                    blend: BlendKind::Additive,
                });
            }
        }
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

    fn step(e: &mut GumGangEffect, frames: f32) -> EffectStatus {
        e.update(&EffectUpdateCtx {
            delta: frames / FRAMES_PER_SECOND,
            camera_target: None,
        })
    }

    fn draws(e: &GumGangEffect) -> Vec<EffectPrimitiveDraw> {
        let mut list = EffectDrawList::new();
        e.collect_draws(&mut list, &render_ctx());
        list.primitives
    }

    fn billboard(p: &EffectPrimitiveDraw) -> ([f32; 3], [f32; 2], [f32; 4], &'static str, BlendKind) {
        match p {
            EffectPrimitiveDraw::Billboard { pos, size, color, texture, blend, .. } => {
                (*pos, *size, *color, *texture, *blend)
            }
            _ => panic!("expected Billboard, got {:?}", p),
        }
    }

    #[test]
    fn emitter_count_scales_billboard_count() {
        // Sociable: one frame in, every emitter×ring streak is still faded in
        // and visible — an additive `super*.bmp` quad. One emitter → 4, four → 16.
        let mut single = GumGangEffect::new([5.0, 0.0, -3.0], GUMGANG);
        let mut steel = GumGangEffect::new([0.0; 3], STEELBODY);
        step(&mut single, 1.0);
        step(&mut steel, 1.0);
        assert_eq!(draws(&single).len(), RINGS_PER_EMITTER);
        assert_eq!(draws(&steel).len(), 4 * RINGS_PER_EMITTER);
        for p in draws(&single) {
            let (_, _, _, tex, blend) = billboard(&p);
            assert!(TEXTURES.contains(&tex));
            assert_eq!(blend, BlendKind::Additive);
        }
    }

    #[test]
    fn double_family_draws_three_concentric_darkening_layers() {
        // 2 emitters × 4 rings × 3 layers. The three layers of a streak share a
        // position but their square *side* scales 1.0 : 1.2 : 2.0, with tints
        // darkening outward.
        let center = [0.0; 3];
        let mut e = GumGangEffect::new(center, DOUBLE_BLUE);
        step(&mut e, 1.0);
        let prims = draws(&e);
        assert_eq!(prims.len(), 2 * RINGS_PER_EMITTER * 3);

        // First streak's three consecutive layers share a centre.
        let p0 = billboard(&prims[0]);
        let p1 = billboard(&prims[1]);
        let p2 = billboard(&prims[2]);
        assert_eq!(p0.0, p1.0, "layers share a position");
        assert_eq!(p1.0, p2.0, "layers share a position");

        let (s0, s1, s2) = (p0.1[0], p1.1[0], p2.1[0]);
        assert!((s1 / s0 - 1.2).abs() < 0.01, "mid layer ≈ 1.2× ({s0} -> {s1})");
        assert!((s2 / s0 - 2.0).abs() < 0.01, "outer layer ≈ 2.0× ({s0} -> {s2})");

        let lum = |c: [f32; 4]| c[0] + c[1] + c[2];
        assert!(lum(p0.2) > lum(p2.2), "outer layer must be darker");
    }

    #[test]
    fn quad_is_fixed_size_while_alpha_pulses() {
        // The pulse drives alpha, not size: the square side stays constant while
        // the streak's alpha changes frame to frame (a flicker, not growth).
        let mut e = GumGangEffect::new([0.0; 3], GUMGANG);
        step(&mut e, FADE_IN_FRAMES + 1.0); // past fade-in so `fade` is 1.0
        let expected_side = SQRT2 * GUMGANG.distance * WORLD_SCALE;

        let mut alphas = Vec::new();
        for _ in 0..12 {
            step(&mut e, 2.0);
            for p in draws(&e) {
                let (_, size, color, ..) = billboard(&p);
                assert!((size[0] - expected_side).abs() < 1e-3, "side must be fixed");
                assert_eq!(size[0], size[1], "quad must be square");
                alphas.push(color[3]);
            }
        }
        let min = alphas.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = alphas.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(max - min > 0.1, "alpha must pulse over time ({min}..{max})");
    }

    #[test]
    fn persistent_vs_finite_lifetime() {
        let mut buff = GumGangEffect::new([0.0; 3], GUMGANG);
        assert!(matches!(step(&mut buff, 600.0), EffectStatus::Running));

        let mut npc = GumGangEffect::new([0.0; 3], GUMGANGNPC);
        let finite = GUMGANGNPC.lifetime_ms.unwrap() as f32 / 1000.0 * FRAMES_PER_SECOND;
        assert!(matches!(step(&mut npc, finite + 1.0), EffectStatus::Dead));
    }

    #[test]
    fn variants_keep_distinct_tints() {
        assert_ne!(GUMGANG.layers[0].color_rgb, GUMGANGNPC.layers[0].color_rgb);
        assert_ne!(DOUBLE_RED.layers[0].color_rgb, DOUBLE_BLUE.layers[0].color_rgb);
        assert_ne!(DOUBLE_WHITE.layers[0].color_rgb, DOUBLE_BLUE.layers[0].color_rgb);
    }
}
