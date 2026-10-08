//! The droplet spec: the shape and material knobs of the DE's droplet — the status bar's
//! module drops and the compositor's scenefx droplet node read the same string, so the two
//! sides can never disagree on a field. Moved here from `cce_ui::scene::paint`, which
//! re-exports it, so the compositor can parse a spec without linking the toolkit.

/// Shape and material knobs for `cce_ui::scene::paint::Prim::Droplet`. Fractions are of the
/// droplet rect's height unless said otherwise, so a spec is resolution- and
/// module-size-independent; the tessellator resolves and clamps them against
/// the concrete rect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropletSpec {
    /// How far the sheet's bottom lifts above the rect bottom (the waist the
    /// sides pull up into), fraction of height. 0 = no waist (a capsule).
    pub sag: f32,
    /// Belly capsule radius, fraction of height. **≤ 0 disables the belly**:
    /// the drop is the sheet alone — with `attach` and `sheet_r` rounding its
    /// top and bottom this is the oval dewdrop, and the default.
    pub belly: f32,
    /// Belly half-width, fraction of the half-width left after the belly
    /// radius (1 = the belly spans the whole bottom).
    pub belly_w: f32,
    /// Smooth-union blend distance, fraction of height — bigger = softer neck
    /// between sheet and belly.
    pub blend: f32,
    /// Sheet bottom-corner radius, fraction of height.
    pub sheet_r: f32,
    /// Sheet TOP-corner radius (the meniscus taper at the attach line),
    /// fraction of height. 0 = the sides meet the attach edge square (the
    /// clinging-pool look); larger values narrow the contact span so the
    /// silhouette curves into the edge like a dewdrop. When `attach + sheet_r`
    /// exceeds the sheet height the pair scales down proportionally, so 0.5 +
    /// 0.5 is the fully continuous egg curve with no straight side segment.
    pub attach: f32,
    /// Tint opacity at the deep interior relative to the color's own alpha;
    /// the rim falls toward `clarity` × that (thin water is clearer). 1 = flat.
    pub clarity: f32,
    /// Dome slope amplitude: scales the surface tilt the shading sees.
    pub dome: f32,
    /// Shaded band width (the dome's curved skirt), fraction of height.
    pub band: f32,
    /// Specular (gleam) strength — replaces the DE material's slot.
    pub gleam: f32,
    /// Wet-surface shininess exponent.
    pub shine: f32,
    /// Fresnel rim crest amplitude (the glass-edge brightening).
    pub rim: f32,
    /// Bottom bow: the drop's bottom boundary becomes ONE continuous circular
    /// arc — lowest at center, rising by `bow` (fraction of height) at the
    /// drop's side extents. The arc's radius is derived per drop from that
    /// fixed edge rise, so a wide drop gets a huge radius and the curvature
    /// stays subtle at the middle while a narrow drop curves visibly. 0
    /// disables it (flat bottom run between the corner arcs).
    pub bow: f32,
    /// Corner-curve exponent for the silhouette (and the dome profile riding
    /// it): 2 = circular arcs, above 2 = superellipse quadrants whose
    /// curvature ramps to ZERO at both ends of each arc — every junction
    /// (attach↔side, side↔bottom, curve↔flat top) becomes curvature-
    /// continuous, so unequal attach/sheet_r radii read as ONE flowing curve
    /// instead of two arcs meeting, and the contact eases out of the flat
    /// top like a meniscus. Clamped to [2, 6].
    pub curve: f32,
    /// Extra tint density at the drop's deep interior: the body opacity ramps
    /// from `clarity` at the rim up to `1 + core` (× the color's own alpha,
    /// clamped to opaque) inside — the water reads thickest in the middle,
    /// which is also where a module's text sits, so glyphs get a calmer
    /// field without giving up the watery rim. 0 = the original flat
    /// interior falloff.
    pub core: f32,
    /// Refraction strength in logical px — how far the COMPOSITOR's droplet
    /// backdrop pass bends the image behind the drop at the rim. Client-side
    /// rendering ignores it (a Wayland client cannot see behind its own
    /// window); the compositor reads the same spec and drives its scenefx
    /// droplet node with it. 0 disables the backdrop pass.
    pub refr: f32,
    /// Strength (0-1) of the compositor pass's inverted lens ghost — the
    /// faint upside-down image of the scene a real hanging drop shows in its
    /// belly. Client-side ignored, like `refr`.
    pub ghost: f32,
    /// Contact-shadow strength (0-1): a soft dark falloff cast below the
    /// drop's lower arc, outside the silhouette — the volume cue of a bead
    /// sitting proud of the surface. The host must leave room beneath the
    /// drop box for it (the status bar insets the box by
    /// [`DropletSpec::shadow_gap`]). 0 disables it.
    pub shadow: f32,
}

impl DropletSpec {
    /// Parse the DE's droplet spec string — whitespace-separated `k=v` pairs
    /// onto the defaults (an empty string is all defaults). Unknown keys and
    /// non-numeric values `log::warn!` and are skipped, so a typo surfaces in
    /// the log instead of silently reverting one knob. Shared by the status
    /// bar (which draws the drop) and the compositor (whose scenefx droplet
    /// node refracts the backdrop behind it) so the two sides can never
    /// disagree about a spec's meaning.
    pub fn parse(raw: &str) -> Self {
        let mut spec = Self::default();
        for tok in raw.split_whitespace() {
            let Some((key, val)) = tok.split_once('=') else {
                log::warn!("droplet spec: token '{}' is not k=v — skipped", tok);
                continue;
            };
            let Ok(v) = val.parse::<f32>() else {
                log::warn!("droplet spec: '{}' has a non-numeric value — skipped", tok);
                continue;
            };
            match key {
                "sag" => spec.sag = v,
                "belly" => spec.belly = v,
                "belly_w" => spec.belly_w = v,
                "blend" => spec.blend = v,
                "sheet_r" => spec.sheet_r = v,
                "attach" => spec.attach = v,
                "clarity" => spec.clarity = v,
                "dome" => spec.dome = v,
                "band" => spec.band = v,
                "gleam" => spec.gleam = v,
                "shine" => spec.shine = v,
                "rim" => spec.rim = v,
                "bow" => spec.bow = v,
                "curve" => spec.curve = v,
                "core" => spec.core = v,
                "refr" => spec.refr = v,
                "ghost" => spec.ghost = v,
                "shadow" => spec.shadow = v,
                _ => log::warn!("droplet spec: unknown key '{}' — skipped", key),
            }
        }
        spec
    }

    /// Resolve the silhouette's height-fraction knobs against a concrete rect
    /// (logical px) with the SAME clamps the tessellator applies: returns
    /// `(sheet_r, attach_r, bow_rise)` in logical px, the attach/sheet pair
    /// proportionally scaled down when it overfills the height. The
    /// compositor's droplet backdrop node uses this so its refracting
    /// silhouette and the client-drawn drop are the same shape.
    pub fn resolve_silhouette(&self, w: f32, h: f32) -> (f32, f32, f32) {
        let hx = w * 0.5;
        let hy = h * 0.5;
        let mut sr = (self.sheet_r.clamp(0.0, 1.0) * h).min(hx);
        let mut ar = (self.attach.clamp(0.0, 1.0) * h).min(hx);
        let sheet_h = 2.0 * hy;
        if sr + ar > sheet_h && sr + ar > 0.0 {
            let f = sheet_h / (sr + ar);
            sr *= f;
            ar *= f;
        }
        let bow = (self.bow.clamp(0.0, 0.5) * h).min(hy * 0.9);
        (sr, ar, bow)
    }

    /// Vertical room (logical px) a host should leave BELOW the drop box for
    /// the contact shadow, given the full slot height. One place, so the
    /// bar's reserved gap and the shader's falloff reach stay proportioned.
    pub fn shadow_gap(&self, slot_h: f32) -> f32 {
        if self.shadow > 0.0 {
            (0.16 * slot_h).ceil()
        } else {
            0.0
        }
    }
}

impl Default for DropletSpec {
    fn default() -> Self {
        // The oval dewdrop: no belly, no sag — one continuous curve from a
        // tapered attach line to a fully round bottom. attach + sheet_r fill
        // the whole height (no straight side segment), biased bottom-heavy,
        // and the superellipse curve exponent keeps the unequal pair
        // curvature-continuous. The pendant-pool look is reachable by
        // setting `belly` > 0 (and usually some `sag`).
        Self {
            sag: 0.0,
            belly: 0.0,
            belly_w: 0.5,
            blend: 0.35,
            sheet_r: 0.58,
            attach: 0.42,
            clarity: 0.5,
            dome: 0.9,
            band: 0.9,
            gleam: 1.4,
            shine: 32.0,
            rim: 0.5,
            bow: 0.12,
            curve: 2.6,
            core: 0.35,
            refr: 0.0,
            ghost: 0.0,
            shadow: 0.35,
        }
    }
}
