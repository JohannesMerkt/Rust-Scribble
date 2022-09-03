use rust_scribble_common::gamestate_common::Player;

pub trait RewardStrategyGuesser: Send + Sync {
    fn reward_points_to_guesser(
        &self,
        player: &mut Player,
        number_of_guessers: usize,
        players_already_guessed: usize,
        time_left: i64,
    );
}

pub trait RewardStrategyDrawer: Send + Sync {
    fn reward_points_to_drawer(
        &self,
        player: &mut Player,
        number_of_guessers: usize,
        players_already_guessed: usize,
        time_left: i64,
    );
}

pub struct EqualRewardStrategy {
    pub full_reward: i64,
}

pub struct TimeBasedRewardStrategy {
    pub full_reward: i64,
    pub initial_time: i64,
}

pub struct LinearlyDecreasingRewardStrategy {
    pub full_reward: i64,
}

pub struct ExponentiallyDecreasingRewardStrategy {
    pub full_reward: i64,
    pub decrease_per_position: f64,
}

/// Warning: this strategy does not award at 'last_award' points in total
pub struct ExponentiallyIncreasingRewardStrategy {
    pub last_reward: i64,
    pub increase_per_position: f64,
}

impl RewardStrategyGuesser for EqualRewardStrategy {
    fn reward_points_to_guesser(
        &self,
        player: &mut Player,
        _number_of_guessers: usize,
        _players_already_guessed: usize,
        _time_left: i64,
    ) {
        player.score += self.full_reward;
    }
}

impl RewardStrategyDrawer for EqualRewardStrategy {
    fn reward_points_to_drawer(
        &self,
        player: &mut Player,
        number_of_guessers: usize,
        _players_already_guessed: usize,
        _time_left: i64,
    ) {
        let points_per_correct_guess = self.full_reward / number_of_guessers as i64;
        player.score += points_per_correct_guess
    }
}

impl RewardStrategyGuesser for TimeBasedRewardStrategy {
    fn reward_points_to_guesser(
        &self,
        player: &mut Player,
        _number_of_guessers: usize,
        _players_already_guessed: usize,
        time_left: i64,
    ) {
        player.score +=
            ((time_left as f64 / self.initial_time as f64) * self.full_reward as f64) as i64;
    }
}

impl RewardStrategyDrawer for TimeBasedRewardStrategy {
    fn reward_points_to_drawer(
        &self,
        player: &mut Player,
        number_of_guessers: usize,
        _players_already_guessed: usize,
        time_left: i64,
    ) {
        player.score += ((time_left as f64 / self.initial_time as f64)
            * (self.full_reward as f64 / number_of_guessers as f64)) as i64;
    }
}

impl RewardStrategyGuesser for LinearlyDecreasingRewardStrategy {
    fn reward_points_to_guesser(
        &self,
        player: &mut Player,
        number_of_guessers: usize,
        players_already_guessed: usize,
        _time_left: i64,
    ) {
        let points_for_last_guesser = self.full_reward / number_of_guessers as i64;
        player.score +=
            self.full_reward - points_for_last_guesser * (players_already_guessed - 1) as i64;
    }
}

impl RewardStrategyGuesser for ExponentiallyDecreasingRewardStrategy {
    fn reward_points_to_guesser(
        &self,
        player: &mut Player,
        _number_of_guessers: usize,
        players_already_guessed: usize,
        _time_left: i64,
    ) {
        player.score += (self.full_reward as f64
            * ((1.0 - self.decrease_per_position).powi(players_already_guessed as i32)) as f64)
            .round() as i64;
    }
}

impl RewardStrategyDrawer for ExponentiallyIncreasingRewardStrategy {
    fn reward_points_to_drawer(
        &self,
        player: &mut Player,
        number_of_guessers: usize,
        players_already_guessed: usize,
        _time_left: i64,
    ) {
        player.score += (self.last_reward as f64
            * ((1.0 - self.increase_per_position)
                .powi((number_of_guessers - players_already_guessed) as i32)) as f64)
            .round() as i64;
    }
}
