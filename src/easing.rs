use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Interp {
    Bezier,
    Linear,
    Constant,
    Cubic,
    Bounce,
    Elastic,
}

impl Interp {
    pub const ALL: [Interp; 6] = [Interp::Bezier, Interp::Linear, Interp::Constant, Interp::Cubic, Interp::Bounce, Interp::Elastic];

    pub fn name(self) -> &'static str {
        match self {
            Interp::Bezier => "Bezier",
            Interp::Linear => "Linear",
            Interp::Constant => "Constant",
            Interp::Cubic => "Cubic",
            Interp::Bounce => "Bounce",
            Interp::Elastic => "Elastic",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ease {
    In,
    Out,
    InOut,
}

impl Ease {
    pub const ALL: [Ease; 3] = [Ease::In, Ease::Out, Ease::InOut];

    pub fn name(self) -> &'static str {
        match self {
            Ease::In => "In",
            Ease::Out => "Out",
            Ease::InOut => "InOut",
        }
    }
}

pub fn eased(interp: Interp, ease: Ease, t: f32) -> f32 {
    let forward = match interp {
        Interp::Cubic => cubic_in,
        Interp::Bounce => bounce_in,
        Interp::Elastic => elastic_in,
        Interp::Constant => return 0.0,
        Interp::Linear | Interp::Bezier => return t,
    };
    match ease {
        Ease::In => forward(t),
        Ease::Out => 1.0 - forward(1.0 - t),
        Ease::InOut if t < 0.5 => forward(2.0 * t) / 2.0,
        Ease::InOut => 1.0 - forward(2.0 - 2.0 * t) / 2.0,
    }
}

fn cubic_in(t: f32) -> f32 {
    t * t * t
}

fn bounce_in(t: f32) -> f32 {
    1.0 - bounce_out(1.0 - t)
}

fn bounce_out(t: f32) -> f32 {
    let n = 7.5625;
    let d = 2.75;
    if t < 1.0 / d {
        n * t * t
    } else if t < 2.0 / d {
        let t = t - 1.5 / d;
        n * t * t + 0.75
    } else if t < 2.5 / d {
        let t = t - 2.25 / d;
        n * t * t + 0.9375
    } else {
        let t = t - 2.625 / d;
        n * t * t + 0.984375
    }
}

fn elastic_in(t: f32) -> f32 {
    if t <= 0.0 || t >= 1.0 {
        return t.clamp(0.0, 1.0);
    }
    let period = 0.3;
    -(2f32.powf(10.0 * (t - 1.0))) * ((t - 1.0 - period / 4.0) * (2.0 * PI) / period).sin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_starts_at_zero_and_ends_at_one() {
        for interp in [Interp::Linear, Interp::Cubic, Interp::Bounce, Interp::Elastic] {
            for ease in Ease::ALL {
                assert!(eased(interp, ease, 0.0).abs() < 1e-4, "{interp:?} {ease:?}");
                assert!((eased(interp, ease, 1.0) - 1.0).abs() < 1e-4, "{interp:?} {ease:?}");
            }
        }
    }

    #[test]
    fn cubic_in_starts_slow() {
        assert!(eased(Interp::Cubic, Ease::In, 0.25) < 0.25);
        assert!(eased(Interp::Cubic, Ease::Out, 0.25) > 0.25);
    }
}
