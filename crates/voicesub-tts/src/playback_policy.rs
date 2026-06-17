pub const SPEECH_QUEUE_BOOST_THRESHOLD: usize = 2;
pub const QUEUE_BOOST_STEP: f32 = 0.12;
pub const QUEUE_BOOST_MAX: f32 = 0.7;
pub const PLAYBACK_RATE_MIN: f32 = 0.5;
pub const PLAYBACK_RATE_MAX: f32 = 2.0;

pub fn queue_depth_for_playback(waiting_count: usize) -> usize {
    waiting_count.saturating_add(1)
}

pub fn clamp_playback_rate(rate: f32) -> f32 {
    if !rate.is_finite() || rate <= 0.0 {
        return 1.0;
    }
    rate.clamp(PLAYBACK_RATE_MIN, PLAYBACK_RATE_MAX)
}

pub fn effective_playback_rate(base_rate: f32, queue_depth: usize, defer_boost: bool) -> f32 {
    let base = clamp_playback_rate(base_rate);
    let depth = if defer_boost {
        queue_depth
            .max(SPEECH_QUEUE_BOOST_THRESHOLD)
            .saturating_sub(1)
    } else {
        queue_depth
    };
    if depth <= SPEECH_QUEUE_BOOST_THRESHOLD {
        return base;
    }
    let excess = depth - SPEECH_QUEUE_BOOST_THRESHOLD;
    let boost = (excess as f32 * QUEUE_BOOST_STEP).min(QUEUE_BOOST_MAX);
    clamp_playback_rate(base + boost)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defers_boost_while_current_clip_is_playing() {
        let speaking = effective_playback_rate(1.0, queue_depth_for_playback(4), true);
        let idle = effective_playback_rate(1.0, queue_depth_for_playback(4), false);
        assert!(speaking <= idle);
        assert!(idle > 1.0);
    }

    #[test]
    fn leaves_small_queue_at_base_rate() {
        assert_eq!(
            effective_playback_rate(1.0, queue_depth_for_playback(1), false),
            1.0
        );
    }
}
