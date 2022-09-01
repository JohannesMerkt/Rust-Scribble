use rust_scribble_common::gamestate_common::Player;

pub trait RewardStrategy: Send + Sync {
    fn reward_points_to_player(&self, player: &mut Player, number_of_players: usize, position_finished: usize);
}

pub struct EqualRewardStrategy {
    pub full_reward: i64,
}

pub struct LinearlyDecreasingRewardStrategy {
    pub full_reward: i64,
}

pub struct ExponentiallyDecreasingRewardStrategy {
    pub full_reward: i64,
    pub decrease_per_position: f64,
}

impl RewardStrategy for EqualRewardStrategy {
    fn reward_points_to_player(&self, player: &mut Player, _number_of_players: usize, _position_finished: usize) {
        player.score += self.full_reward;
    }
}


impl RewardStrategy for LinearlyDecreasingRewardStrategy {
    fn reward_points_to_player(&self, player: &mut Player, number_of_players: usize, position_finished: usize) {
        let points_for_last_guesser = self.full_reward / number_of_players as i64;
        player.score += self.full_reward -
            points_for_last_guesser * (position_finished - 1) as i64;
    }
}

impl RewardStrategy for ExponentiallyDecreasingRewardStrategy {
    fn reward_points_to_player(&self, player: &mut Player, _number_of_players: usize, position_finished: usize) {
        player.score += (self.full_reward as f64 * ((1.0 - self.decrease_per_position).powi(
            position_finished as i32,
        )) as f64).round() as i64;
    }
}
