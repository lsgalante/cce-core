//! The DE's ramp spec — `"smooth;0.000:0.500,0.200:1.000,…"` — and the curve it draws:
//! what cce-ui's Ramp widget writes, its relief profiles read, and the window manager's
//! camera transitions evaluate. `cce_ui::widget` and `cce_ui::layout` re-export these.

/// Serialize ramp keys + line type as the DE's ramp spec string:
/// `"smooth;0.000:0.500,0.200:1.000,…"` (`"linear;…"` for straight segments) —
/// the format ramp-valued params travel in (`ParametersBg` "ramp" rows,
/// project files, `cce_ui::layout::set_bevel_profile_keys` consumers).
pub fn format_ramp_spec(keys: &[(f32, f32)], smooth: bool) -> String {
    let body: Vec<String> =
        keys.iter().map(|(p, v)| format!("{:.3}:{:.3}", p, v)).collect();
    format!("{};{}", if smooth { "smooth" } else { "linear" }, body.join(","))
}

/// Parse a ramp spec string ([`format_ramp_spec`]) into `(keys, smooth)`.
/// `None` for anything that doesn't yield at least two keys.
pub fn parse_ramp_spec(spec: &str) -> Option<(Vec<(f32, f32)>, bool)> {
    let (head, body) = spec.split_once(';')?;
    let smooth = head.trim() == "smooth";
    let mut keys = Vec::new();
    for part in body.split(',') {
        let (p, v) = part.split_once(':')?;
        keys.push((
            p.trim().parse::<f32>().ok()?.clamp(0.0, 1.0),
            v.trim().parse::<f32>().ok()?.clamp(0.0, 1.0),
        ));
    }
    if keys.len() < 2 {
        return None;
    }
    keys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    Some((keys, smooth))
}

/// Evaluate a ramp key list at `t` — THE ramp interpolation of the DE.
/// `cce_ui::widget::Ramp` draws it, `RampPreview` previews it, the relief
/// profile LUTs sample it, and cce-window-manager's camera speed ramp mirrors
/// it verbatim (that crate stays dependency-minimal), so a curve sculpted in
/// the widget is exactly the curve every consumer evaluates. Keys are
/// `(pos, value)` sorted by pos; outside the key range the end values hold.
///
/// `smooth` is the widget's curved line type: a **monotone cubic** through
/// the keys (Fritsch–Butland tangents, cubic Hermite segments) — C1, passes
/// through every key, never overshoots a key, and flattens only at the ends
/// and at genuine local extrema. It used to be a smoothstep blend PER
/// SEGMENT, which forces zero slope at every key: a curve with more than two
/// keys came out as a chain of little bumps, and a wall profile built from
/// it read as jagged and uneven where a smooth slope was drawn. A two-key
/// ramp is unchanged — zero tangents at both ends make the single Hermite
/// segment exactly the old smoothstep — so the identity sentinel and every
/// simple ease keep their look. `false` is straight segments.
pub fn sample_ramp_keys(keys: &[(f32, f32)], smooth: bool, t: f32) -> f32 {
    let Some(first) = keys.first() else { return 0.0 };
    let last = keys.last().unwrap();
    if t <= first.0 {
        return first.1;
    }
    if t >= last.0 {
        return last.1;
    }
    for i in 0..keys.len() - 1 {
        let ((x0, y0), (x1, y1)) = (keys[i], keys[i + 1]);
        if t < x0 || t > x1 {
            continue;
        }
        let h = x1 - x0;
        if h.abs() < 0.0001 {
            return y0;
        }
        let s = (t - x0) / h;
        if !smooth {
            return y0 + (y1 - y0) * s;
        }
        let (m0, m1) = (ramp_key_tangent(keys, i), ramp_key_tangent(keys, i + 1));
        let (s2, s3) = (s * s, s * s * s);
        let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
        let h10 = s3 - 2.0 * s2 + s;
        let h01 = -2.0 * s3 + 3.0 * s2;
        let h11 = s3 - s2;
        return h00 * y0 + h10 * h * m0 + h01 * y1 + h11 * h * m1;
    }
    first.1
}

/// The monotone cubic's tangent (dy/dpos) at key `i`: zero at either end and
/// at any local extremum (so the curve never overshoots a key), otherwise the
/// Fritsch–Butland weighted harmonic mean of the two neighbouring secants —
/// the shape-preserving choice, which keeps every segment monotone whenever
/// its keys are.
fn ramp_key_tangent(keys: &[(f32, f32)], i: usize) -> f32 {
    if i == 0 || i + 1 >= keys.len() {
        return 0.0;
    }
    let ((xp, yp), (x, y), (xn, yn)) = (keys[i - 1], keys[i], keys[i + 1]);
    let (h0, h1) = (x - xp, xn - x);
    if h0 <= 0.0001 || h1 <= 0.0001 {
        return 0.0;
    }
    let (d0, d1) = ((y - yp) / h0, (yn - y) / h1);
    if d0 * d1 <= 0.0 {
        return 0.0;
    }
    let (w0, w1) = (2.0 * h1 + h0, h1 + 2.0 * h0);
    (w0 + w1) / (w0 / d0 + w1 / d1)
}
