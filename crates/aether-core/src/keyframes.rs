use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier { x1: f32, y1: f32, x2: f32, y2: f32 },
}

impl EasingFunction {
    pub fn interpolate(&self, t: f32) -> f32 {
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }
        match self {
            EasingFunction::Linear => t,
            EasingFunction::EaseIn => t * t,
            EasingFunction::EaseOut => t * (2.0 - t),
            EasingFunction::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
            EasingFunction::CubicBezier { x1, y1, x2, y2 } => {
                solve_cubic_bezier(*x1, *y1, *x2, *y2, t)
            }
        }
    }
}

// Cubic bezier solver using Newton-Raphson with binary search fallback
fn solve_cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    let mut tau = t;
    for _ in 0..8 {
        let x = sample_curve_x(x1, x2, tau);
        let dx = sample_curve_derivative_x(x1, x2, tau);
        if dx.abs() < 1e-6 {
            break;
        }
        let diff = x - t;
        tau -= diff / dx;
    }

    // Binary search fallback if Newton-Raphson didn't converge perfectly
    if (sample_curve_x(x1, x2, tau) - t).abs() > 1e-4 {
        let mut low = 0.0;
        let mut high = 1.0;
        tau = t;
        while high - low > 1e-5 {
            let x = sample_curve_x(x1, x2, tau);
            if (x - t).abs() < 1e-5 {
                break;
            }
            if x < t {
                low = tau;
            } else {
                high = tau;
            }
            tau = (low + high) * 0.5;
        }
    }

    sample_curve_y(y1, y2, tau)
}

fn sample_curve_x(x1: f32, x2: f32, tau: f32) -> f32 {
    3.0 * (1.0 - tau) * (1.0 - tau) * tau * x1 + 3.0 * (1.0 - tau) * tau * tau * x2 + tau * tau * tau
}

fn sample_curve_y(y1: f32, y2: f32, tau: f32) -> f32 {
    3.0 * (1.0 - tau) * (1.0 - tau) * tau * y1 + 3.0 * (1.0 - tau) * tau * tau * y2 + tau * tau * tau
}

fn sample_curve_derivative_x(x1: f32, x2: f32, tau: f32) -> f32 {
    let c = 3.0 * x1 - 3.0 * x2 + 1.0;
    let b = -6.0 * x1 + 3.0 * x2;
    let a = 3.0 * x1;
    3.0 * c * tau * tau + 2.0 * b * tau + a
}

impl std::str::FromStr for EasingFunction {
    type Err = crate::AetherError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_lowercase();
        if s == "linear" {
            Ok(EasingFunction::Linear)
        } else if s == "ease_in" || s == "ease-in" {
            Ok(EasingFunction::EaseIn)
        } else if s == "ease_out" || s == "ease-out" {
            Ok(EasingFunction::EaseOut)
        } else if s == "ease_in_out" || s == "ease-in-out" {
            Ok(EasingFunction::EaseInOut)
        } else if s.starts_with("cubic_bezier") || s.starts_with("cubic-bezier") {
            let trimmed = s
                .replace("cubic_bezier", "")
                .replace("cubic-bezier", "")
                .replace(['(', ')'], "");
            let parts: Vec<&str> = trimmed.split(',').map(|p| p.trim()).collect();
            if parts.len() == 4 {
                let x1 = parts[0].parse::<f32>().map_err(|e| crate::AetherError::InvalidCommand(e.to_string()))?;
                let y1 = parts[1].parse::<f32>().map_err(|e| crate::AetherError::InvalidCommand(e.to_string()))?;
                let x2 = parts[2].parse::<f32>().map_err(|e| crate::AetherError::InvalidCommand(e.to_string()))?;
                let y2 = parts[3].parse::<f32>().map_err(|e| crate::AetherError::InvalidCommand(e.to_string()))?;
                Ok(EasingFunction::CubicBezier { x1, y1, x2, y2 })
            } else {
                Err(crate::AetherError::InvalidCommand(format!("Invalid cubic_bezier format: {}", s)))
            }
        } else {
            Ok(EasingFunction::Linear)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyframe<T> {
    pub time_ms: u64,
    pub value: T,
    pub easing: EasingFunction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct KeyframeTrack<T> {
    pub keyframes: Vec<Keyframe<T>>,
}

impl<T: Copy + PartialOrd> KeyframeTrack<T> {
    pub fn new() -> Self {
        KeyframeTrack { keyframes: Vec::new() }
    }

    pub fn insert_keyframe(&mut self, kf: Keyframe<T>) {
        if let Some(pos) = self.keyframes.iter().position(|k| k.time_ms == kf.time_ms) {
            self.keyframes[pos] = kf;
        } else {
            self.keyframes.push(kf);
            self.keyframes.sort_by_key(|k| k.time_ms);
        }
    }

    pub fn remove_keyframe(&mut self, time_ms: u64) -> bool {
        let len_before = self.keyframes.len();
        self.keyframes.retain(|k| k.time_ms != time_ms);
        self.keyframes.len() < len_before
    }
}

impl KeyframeTrack<f32> {
    pub fn interpolate(&self, time_ms: u64) -> f32 {
        if self.keyframes.is_empty() {
            return 0.0;
        }
        if self.keyframes.len() == 1 {
            return self.keyframes[0].value;
        }

        if time_ms <= self.keyframes[0].time_ms {
            return self.keyframes[0].value;
        }
        let last_idx = self.keyframes.len() - 1;
        if time_ms >= self.keyframes[last_idx].time_ms {
            return self.keyframes[last_idx].value;
        }

        let mut idx = 0;
        for i in 0..last_idx {
            if time_ms >= self.keyframes[i].time_ms && time_ms <= self.keyframes[i+1].time_ms {
                idx = i;
                break;
            }
        }

        let kf0 = &self.keyframes[idx];
        let kf1 = &self.keyframes[idx + 1];

        let total_duration = (kf1.time_ms - kf0.time_ms) as f32;
        if total_duration == 0.0 {
            return kf1.value;
        }

        let elapsed = (time_ms - kf0.time_ms) as f32;
        let t = elapsed / total_duration;

        let t_eased = kf0.easing.interpolate(t);

        kf0.value + (kf1.value - kf0.value) * t_eased
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_easing_parsing() {
        let linear = EasingFunction::from_str("linear").unwrap();
        assert_eq!(linear, EasingFunction::Linear);

        let ease_in = EasingFunction::from_str("ease_in").unwrap();
        assert_eq!(ease_in, EasingFunction::EaseIn);

        let cb = EasingFunction::from_str("cubic-bezier(0.25, 0.1, 0.25, 1.0)").unwrap();
        assert_eq!(cb, EasingFunction::CubicBezier { x1: 0.25, y1: 0.1, x2: 0.25, y2: 1.0 });
    }

    #[test]
    fn test_easing_interpolation() {
        let linear = EasingFunction::Linear;
        assert_eq!(linear.interpolate(0.0), 0.0);
        assert_eq!(linear.interpolate(0.5), 0.5);
        assert_eq!(linear.interpolate(1.0), 1.0);

        let ease_in = EasingFunction::EaseIn;
        assert_eq!(ease_in.interpolate(0.5), 0.25);

        let cubic = EasingFunction::CubicBezier { x1: 0.25, y1: 0.1, x2: 0.25, y2: 1.0 };
        let mid = cubic.interpolate(0.5);
        assert!(mid > 0.0 && mid < 1.0);
    }

    #[test]
    fn test_keyframe_track() {
        let mut track = KeyframeTrack::new();
        track.insert_keyframe(Keyframe {
            time_ms: 0,
            value: 0.0,
            easing: EasingFunction::Linear,
        });
        track.insert_keyframe(Keyframe {
            time_ms: 1000,
            value: 10.0,
            easing: EasingFunction::Linear,
        });

        assert_eq!(track.interpolate(0), 0.0);
        assert_eq!(track.interpolate(500), 5.0);
        assert_eq!(track.interpolate(1000), 10.0);
        assert_eq!(track.interpolate(1500), 10.0);
    }
}
