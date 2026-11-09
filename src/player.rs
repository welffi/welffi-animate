pub const SPEEDS: [f64; 6] = [0.1, 0.25, 0.5, 1.0, 1.5, 2.0];

pub struct Player {
    pub time: f64,
    pub playing: bool,
    pub speed: f64,
    pub looping: bool,
    pub loop_in: Option<f64>,
    pub loop_out: Option<f64>,
}

impl Default for Player {
    fn default() -> Player {
        Player { time: 0.0, playing: true, speed: 1.0, looping: true, loop_in: None, loop_out: None }
    }
}

impl Player {
    pub fn range(&self, duration: f64) -> (f64, f64) {
        let start = self.loop_in.unwrap_or(0.0).min(duration);
        let end = self.loop_out.unwrap_or(duration).clamp(start, duration);
        (start, end)
    }

    pub fn advance(&mut self, seconds: f64, duration: f64) {
        if !self.playing || duration <= 0.0 {
            return;
        }
        self.time += seconds * self.speed;
        let (start, end) = self.range(duration);
        if self.time < end {
            return;
        }
        if self.looping && end > start {
            self.time = start + (self.time - start) % (end - start);
        } else {
            self.time = end;
            self.playing = false;
        }
    }

    pub fn follow(&mut self, now: bool, before: bool, duration: f64) {
        if now != before && now != self.playing {
            self.toggle(duration);
        }
    }

    pub fn set_in(&mut self) {
        self.loop_in = Some(self.time);
        if self.loop_out.is_some_and(|out| out <= self.time) {
            self.loop_out = None;
        }
    }

    pub fn set_out(&mut self) {
        self.loop_out = Some(self.time);
        if self.loop_in.is_some_and(|start| start >= self.time) {
            self.loop_in = None;
        }
    }

    pub fn clear_loop(&mut self) {
        self.loop_in = None;
        self.loop_out = None;
    }

    pub fn toggle(&mut self, duration: f64) {
        let (start, end) = self.range(duration);
        if !self.playing && self.time >= end {
            self.time = start;
        }
        self.playing = !self.playing;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playing_loops_back_to_the_start() {
        let mut player = Player::default();
        player.advance(1.5, 1.0);
        assert!((player.time - 0.5).abs() < 1e-9);
        assert!(player.playing);
    }

    #[test]
    fn playing_without_loop_stops_at_the_end() {
        let mut player = Player { looping: false, ..Player::default() };
        player.advance(2.0, 1.0);
        assert_eq!(player.time, 1.0);
        assert!(!player.playing);
    }

    #[test]
    fn slow_motion_takes_longer_to_get_there() {
        let mut player = Player { speed: 0.25, ..Player::default() };
        player.advance(1.0, 10.0);
        assert!((player.time - 0.25).abs() < 1e-9);
    }

    #[test]
    fn the_loop_range_keeps_playback_between_its_points() {
        let mut player = Player { time: 2.9, loop_in: Some(2.0), loop_out: Some(3.0), ..Player::default() };
        player.advance(0.3, 10.0);
        assert!((player.time - 2.2).abs() < 1e-9);
    }

    #[test]
    fn a_loop_out_before_the_loop_in_is_pulled_up_to_it() {
        let player = Player { loop_in: Some(4.0), loop_out: Some(1.0), ..Player::default() };
        assert_eq!(player.range(10.0), (4.0, 4.0));
    }

    #[test]
    fn setting_one_loop_point_past_the_other_drops_the_other() {
        let mut player = Player { time: 3.0, loop_in: Some(1.0), loop_out: Some(2.0), ..Player::default() };
        player.set_in();
        assert_eq!((player.loop_in, player.loop_out), (Some(3.0), None));
        player.time = 2.0;
        player.set_out();
        assert_eq!((player.loop_in, player.loop_out), (None, Some(2.0)));
        player.clear_loop();
        assert_eq!((player.loop_in, player.loop_out), (None, None));
    }

    #[test]
    fn following_copies_a_change_of_the_other_player_and_nothing_else() {
        let mut player = Player { playing: false, ..Player::default() };
        player.follow(true, true, 5.0);
        assert!(!player.playing);
        player.follow(true, false, 5.0);
        assert!(player.playing);
        player.follow(false, true, 5.0);
        assert!(!player.playing);
        player.follow(false, true, 5.0);
        assert!(!player.playing);
    }

    #[test]
    fn pressing_play_at_the_end_starts_over() {
        let mut player = Player { time: 1.0, playing: false, looping: false, ..Player::default() };
        player.toggle(1.0);
        assert_eq!(player.time, 0.0);
        assert!(player.playing);
    }

    #[test]
    fn nothing_moves_while_paused_or_empty() {
        let mut paused = Player { playing: false, ..Player::default() };
        paused.advance(1.0, 5.0);
        assert_eq!(paused.time, 0.0);
        let mut empty = Player::default();
        empty.advance(1.0, 0.0);
        assert_eq!(empty.time, 0.0);
    }
}
