//! GRF SPR-sprite descriptors per `EffectId`.
//!
//! Parallel to [`super::str_aliases`]: each implemented effect returns a
//! [`SprDef`] carrying the canonical GRF path (without the `.spr`/`.act`
//! extension), plus its original-game `m_size`, `m_animSpeed` and
//! `m_repeatAnim`.
//! Returning `None` means "this id is not an SPR-billboard effect" — the
//! caller then routes the id through the regular Custom/STR/Noop
//! fall-through.

use models::enums::effect_id::EffectId;

/// Per-id SPR billboard parameters lifted from the original game.
///
/// The original game's `RagEffectPrim` constructor sets `m_animSpeed = 4`,
/// `m_repeatAnim = true`, and `m_red = m_green = m_blue = 255` by default.
/// Effects that never call the corresponding setter inherit those defaults,
/// so [`SprDef::new`] mirrors them. Callers override only what the effect's
/// recipe actually changes.
#[derive(Clone, Copy, Debug)]
pub struct SprDef {
    pub sprite: &'static str,
    pub size_scale: f32,
    pub anim_speed: f32,
    /// `true` = loop the .act motions (original game's `m_repeatAnim`).
    /// `false` = play once and hold the final motion until the effect's
    /// duration_ms expires.
    pub repeat: bool,
    /// RGBA multiplier applied per-pixel. `[1.0; 4]` = no tint (equivalent
    /// to dhxj's `PT_USEORGARGB`). Effects that zero a channel — e.g.
    /// DarkBreath setting `m_green = m_blue = 0` — populate this.
    pub tint: [f32; 4],
}

impl SprDef {
    const fn new(sprite: &'static str) -> Self {
        Self {
            sprite,
            size_scale: 1.0,
            anim_speed: 4.0,
            repeat: true,
            tint: [1.0, 1.0, 1.0, 1.0],
        }
    }
    const fn with_size(mut self, size_scale: f32) -> Self {
        self.size_scale = size_scale;
        self
    }
    const fn with_anim_speed(mut self, anim_speed: f32) -> Self {
        self.anim_speed = anim_speed;
        self
    }
    const fn one_shot(mut self) -> Self {
        self.repeat = false;
        self
    }
    const fn with_tint(mut self, tint: [f32; 4]) -> Self {
        self.tint = tint;
        self
    }
}

pub fn spr_def(id: EffectId) -> Option<SprDef> {
    Some(match id {
        // Torch: dhxj reads animSpeed from m_param[1] (clamped to ≥1). The
        // ambient torch spawned by the client never sets m_param, so the
        // clamp picks 1.0.
        EffectId::Torch => SprDef::new("data/sprite/이팩트/torch_01").with_anim_speed(1.0),
        // Maple: dhxj uses PP_SAKURA (weather primitive); approximate with a
        // looping single Spr — animation cadence inherits the constructor
        // default since there's no direct equivalent.
        EffectId::Maple => SprDef::new("data/sprite/이팩트/단풍"),
        // Aqua: animSpeed=2, m_repeatAnim=false in dhxj `Aqua()`.
        EffectId::Aqua => SprDef::new("data/sprite/이팩트/아쿠아플레이")
            .with_anim_speed(2.0)
            .one_shot(),
        // Vallentine(0): animSpeed=2, m_repeatAnim=false.
        EffectId::Vallentine => SprDef::new("data/sprite/이팩트/vallentine")
            .with_anim_speed(2.0)
            .one_shot(),
        // Dragonsmoke: dhxj `DragonSmoke()` never calls SetAnimSpeed so the
        // RagEffectPrim constructor default (4) sticks. m_size=1.5.
        // Tilt/drift aren't reproduced (renderer is axis-aligned), so the
        // puff appears static — acceptable.
        EffectId::Dragonsmoke => SprDef::new("data/sprite/이팩트/굴뚝연기").with_size(1.5),
        // PoisonHit: PT_USEORGARGB, m_size=1.5, animSpeed=2,
        // m_repeatAnim=false. Without repeat=false the .act loops and
        // re-renders the impact instead of holding the final smoke puffs.
        EffectId::Poisonhit => SprDef::new("data/sprite/이팩트/poisonhit")
            .with_size(1.5)
            .with_anim_speed(2.0)
            .one_shot(),
        // DarkBreath: dhxj zeroes m_green / m_blue so the sprite renders
        // pure red. m_size=0.8, animSpeed=1, m_duration=65 (overrides the
        // table value of 500). Fade-out from frame 60 isn't reproduced yet
        // — the renderer holds full alpha until the holder kills the
        // effect at duration.
        EffectId::Darkbreath => SprDef::new("data/sprite/이팩트/darkbreath")
            .with_size(0.8)
            .with_anim_speed(1.0)
            .with_tint([1.0, 0.0, 0.0, 1.0]),
        // Thunderstorm2: PT_USEORGARGB, m_size=2.5, m_animSpeed=2,
        // looping for the full master duration. The original game
        // overlays this on the standard thunder_storm STR, but the
        // master switch routes by id so the SPR-only branch covers the
        // gun-skill variant the server emits.
        EffectId::Thunderstorm2 => SprDef::new("data/sprite/이팩트/thunder_storm")
            .with_size(2.5)
            .with_anim_speed(2.0),
        _ => return None,
    })
}
