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
    /// to original game's `PT_USEORGARGB`). Effects that zero a channel — e.g.
    /// DarkBreath setting `m_green = m_blue = 0` — populate this.
    pub tint: [f32; 4],
    /// Y-offset in world units (negative = upward). Mirrors `m_deltaPos2.y`.
    pub pos_y: f32,
    /// ACT action index to play. Mirrors the original game's
    /// `SetAction(F1, …)`: most SPR effects play action 0, but siblings that
    /// share one sprite (e.g. Vallentine vs Vallentine2) differ only by action.
    pub action: usize,
}

impl SprDef {
    const fn new(sprite: &'static str) -> Self {
        Self {
            sprite,
            size_scale: 1.0,
            anim_speed: 4.0,
            repeat: true,
            tint: [1.0, 1.0, 1.0, 1.0],
            pos_y: 0.0,
            action: 0,
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
    const fn with_pos_y(mut self, pos_y: f32) -> Self {
        self.pos_y = pos_y;
        self
    }
    const fn with_action(mut self, action: usize) -> Self {
        self.action = action;
        self
    }
}

pub fn spr_def(id: EffectId) -> Option<SprDef> {
    Some(match id {
        // Torch: original game reads animSpeed from m_param[1] (clamped to ≥1). The
        // ambient torch spawned by the client never sets m_param, so the
        // clamp picks 1.0.
        EffectId::Torch => SprDef::new("data/sprite/이팩트/torch_01").with_anim_speed(1.0),
        // Maple: original game uses PP_SAKURA (weather primitive); approximate with a
        // looping single Spr — animation cadence inherits the constructor
        // default since there's no direct equivalent.
        EffectId::Maple => SprDef::new("data/sprite/이팩트/단풍"),
        // Aqua: animSpeed=2, m_repeatAnim=false in original game `Aqua()`.
        EffectId::Aqua => SprDef::new("data/sprite/이팩트/성수뜨기")
            .with_anim_speed(2.0)
            .one_shot()
            .with_pos_y(-20.0),
        // Vallentine(0): animSpeed=2, m_repeatAnim=false.
        EffectId::Vallentine => SprDef::new("data/sprite/이팩트/vallentine")
            .with_anim_speed(2.0)
            .one_shot(),
        // Vallentine2 is Vallentine(1): same sprite, ACT action 1.
        EffectId::Vallentine2 => SprDef::new("data/sprite/이팩트/vallentine")
            .with_anim_speed(2.0)
            .one_shot()
            .with_action(1),
        // Itemfast is Vallentine(3): winged-boots sprite, animSpeed=4, one-shot.
        EffectId::Itemfast => SprDef::new("data/sprite/이팩트/fast")
            .with_anim_speed(4.0)
            .one_shot(),
        EffectId::Blessing => SprDef::new("data/sprite/이팩트/축복").one_shot(),
        // Demonstration: original game's `Demonstration()` calls
        // `SetAction(0, 16, MT_LOOP)` so the .act motion repeats over the
        // master duration. `m_size = 1.2`, `m_alpha = 200/255`, y-offset
        // `-1.0`. `m_animSpeed = 4` matches the constructor default.
        EffectId::Demonstration => SprDef::new("data/sprite/이팩트/데몬스트레이션")
            .with_size(1.2)
            .with_pos_y(-1.0),
        EffectId::NpcStop => SprDef::new("data/sprite/이팩트/스톱"),
        // Wink (`CHIMTO(1)`) is a Custom effect (`effects/wink.rs`), not a
        // `spr_def` — it picks one of wink.spr's four directional actions from
        // the camera angle, which the data-driven Spr path can't do.
        // Hamicastle: original game `Effect_SPR(0)` sets `m_animSpeed = 2`,
        // `m_repeatAnim = false`, `PT_USEORGARGB`. The sprite path is the
        // EUC-KR transliteration of `misc\kaeseulring.spr`.
        EffectId::Hamicastle => SprDef::new("data/sprite/이팩트/캐슬링")
            .with_anim_speed(2.0)
            .one_shot(),
        // Item status billboards — `Effect_SPR(2..6)`. Same setup as the other
        // `Effect_SPR` entries: `m_animSpeed = 2`, `m_repeatAnim = false`,
        // `PT_USEORGARGB` (default tint). One-shot one-frame-deep PP_3DPARTICLE
        // with no drift, so a held one-shot Spr reproduces them.
        EffectId::ItemThunder => SprDef::new("data/sprite/이팩트/item_thunder")
            .with_anim_speed(2.0)
            .one_shot(),
        EffectId::ItemCloud => SprDef::new("data/sprite/이팩트/item_cloud")
            .with_anim_speed(2.0)
            .one_shot(),
        EffectId::ItemCurse => SprDef::new("data/sprite/이팩트/item_curse")
            .with_anim_speed(2.0)
            .one_shot(),
        EffectId::ItemZzz => SprDef::new("data/sprite/이팩트/item_zzz")
            .with_anim_speed(2.0)
            .one_shot(),
        EffectId::ItemRain => SprDef::new("data/sprite/이팩트/item_rain")
            .with_anim_speed(2.0)
            .one_shot(),
        EffectId::Hamiblood => SprDef::new("data/sprite/이팩트/블러드러스트").one_shot(),
        EffectId::Kirikage => SprDef::new("data/sprite/이팩트/그림자베기").one_shot(),
        EffectId::Tatami => SprDef::new("data/sprite/이팩트/다다미 뒤집기").one_shot(),
        EffectId::Kasumikiri => SprDef::new("data/sprite/이팩트/안개베기").one_shot(),
        EffectId::Issen => SprDef::new("data/sprite/이팩트/일섬").one_shot(),
        EffectId::Kaen => SprDef::new("data/sprite/이팩트/화염진"),
        EffectId::Desperado => SprDef::new("data/sprite/이팩트/데스페라도").one_shot(),
        EffectId::LightningS => SprDef::new("data/sprite/아이템/라이트닝스피어").one_shot(),
        EffectId::BlindS => SprDef::new("data/sprite/아이템/블라인드스피어").one_shot(),
        EffectId::PoisonS => SprDef::new("data/sprite/아이템/포이즌스피어").one_shot(),
        EffectId::FreezingS => SprDef::new("data/sprite/아이템/프리징스피어").one_shot(),
        EffectId::FlareS => SprDef::new("data/sprite/아이템/플레어스피어").one_shot(),
        EffectId::Rapidshower => SprDef::new("data/sprite/이팩트/래피드샤워").one_shot(),
        EffectId::Magicalbullet => SprDef::new("data/sprite/이팩트/매지컬불릿").one_shot(),
        EffectId::Spreadattack => SprDef::new("data/sprite/이팩트/스프레드").one_shot(),
        EffectId::Tracking => SprDef::new("data/sprite/이팩트/트래킹").one_shot(),
        EffectId::Tripleaction => SprDef::new("data/sprite/이팩트/트리플액션").one_shot(),
        EffectId::NpcEarthquake => SprDef::new("data/sprite/이팩트/어스퀘이크").one_shot(),
        EffectId::PokLove => SprDef::new("data/sprite/이팩트/폭죽_러브").one_shot(),
        EffectId::PokBirth => SprDef::new("data/sprite/이팩트/폭죽_생일").one_shot(),
        EffectId::PokChristmas => SprDef::new("data/sprite/이팩트/폭죽_크리스마스").one_shot(),
        // Firework banners — `Effect_SPR(33/34)`, same setup as PokBirth above
        // (`animSpeed = 4`, `m_repeatAnim = false`). The original game's roman
        // sprite names (`white_day_fireworks` / `valentine_fireworks`) are the
        // Korean `폭죽_<word>` resources in the classic GRF.
        EffectId::PokWhite => SprDef::new("data/sprite/이팩트/폭죽_화이트데이").one_shot(),
        EffectId::PokValen => SprDef::new("data/sprite/이팩트/폭죽_발렌타인").one_shot(),
        // Dragonsmoke is routed through `spr_burst_params` because the
        // original game's `DragonSmoke()` launches a `PP_3DPARTICLE`
        // with upward drift + per-frame yaw spin + fade-out at 2/3 of
        // lifetime — none of which a static `SprDef` reproduces. See
        // `spr_burst.rs::spr_burst_params`.
        // PoisonHit: PT_USEORGARGB, m_size=1.5, animSpeed=2,
        // m_repeatAnim=false. Without repeat=false the .act loops and
        // re-renders the impact instead of holding the final smoke puffs.
        EffectId::Poisonhit => SprDef::new("data/sprite/이팩트/poisonhit")
            .with_size(1.5)
            .with_anim_speed(2.0)
            .one_shot(),
        // DarkBreath: original game zeroes m_green / m_blue so the sprite renders
        // pure red. m_size=0.8, animSpeed=1, m_duration=65 (overrides the
        // table value of 500). Fade-out from frame 60 isn't reproduced yet
        // — the renderer holds full alpha until the holder kills the
        // effect at duration.
        EffectId::Darkbreath => SprDef::new("data/sprite/이팩트/darkbreath")
            .with_size(0.8)
            .with_anim_speed(1.0)
            .with_tint([1.0, 0.0, 0.0, 1.0]),
        // Thunderstorm2: the original game's handler points at
        // `misc\thunder_storm.spr`, but that sprite is a renewal-era
        // addition not present in the classic GRF. The JS reference
        // client routes this id to the `setsudan` STR file instead, so
        // we follow that mapping (see `str_aliases.rs`) and leave SPR
        // routing out for this id.

        // Monster effects — `Effect_SPR(7..13)`. Each launches one
        // `PP_3DPARTICLE` billboard held until the duration. `animSpeed = 4`,
        // `m_repeatAnim = false` except M04. M02 (`Effect_SPR(8)`) is *not*
        // here — it's directional (see `effects/m_ef02.rs`).
        // M01: `m_pattern = 0` (plain alpha blend, not the default
        // PT_USEORGARGB), `m_alpha = 220`, `animSpeed = 3`. No blend field on
        // `SprDef`, so the alpha folds into the tint.
        EffectId::M01 => SprDef::new("data/sprite/이팩트/m_ef01")
            .with_anim_speed(3.0)
            .one_shot()
            .with_tint([1.0, 1.0, 1.0, 220.0 / 255.0]),
        EffectId::M03 => SprDef::new("data/sprite/이팩트/m_ef03").one_shot(),
        // M04: the one looping member — Somatology-lab mob aura, repeats over
        // its persistent duration (default `repeat = true` is correct).
        EffectId::M04 => SprDef::new("data/sprite/이팩트/m_ef04"),
        EffectId::M05 => SprDef::new("data/sprite/이팩트/m_ef05").one_shot(),
        EffectId::M06 => SprDef::new("data/sprite/이팩트/m_ef06").one_shot(),
        EffectId::M07 => SprDef::new("data/sprite/이팩트/m_ef07").one_shot(),
        _ => return None,
    })
}
