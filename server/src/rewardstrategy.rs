use rust_scribble_common::gamestate_common::Player;

pub trait RewardStrategy {
    fn reward_points_to_player(&self, player: &mut Player, all_players: &mut Vec<Player>);
}

pub struct EqualRewardStrategy {
    full_reward: i64,
}

pub struct LinearlyDecreasingRewardStrategy {
    full_reward: i64,
}

pub struct ExponentiallyDecreasingRewardStrategy {
    full_reward: i64,
    decrease_per_position: f64,
}

impl RewardStrategy for EqualRewardStrategy {
    fn reward_points_to_player(&self, player: &mut Player, _all_players: &mut Vec<Player>) {
        player.score += self.full_reward;
    }
}

fn calc_position_finished(all_players: &[Player]) -> usize {
    all_players.iter().filter(|p| p.guessed_word).count()
}

impl RewardStrategy for LinearlyDecreasingRewardStrategy {
    fn reward_points_to_player(&self, player: &mut Player, all_players: &mut Vec<Player>) {
        let number_of_players = all_players.len();
        let points_for_last_guesser = self.full_reward / number_of_players as i64;
        player.score += self.full_reward -
            points_for_last_guesser * (calc_position_finished(all_players) - 1) as i64;
    }
}

impl RewardStrategy for ExponentiallyDecreasingRewardStrategy {
    fn reward_points_to_player(&self, player: &mut Player, all_players: &mut Vec<Player>) {
        player.score += self.full_reward * ((1.0 - self.decrease_per_position).powi(
            calc_position_finished(all_players) as i32,
        )) as i64;
    }
}
