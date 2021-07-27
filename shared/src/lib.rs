pub mod game;

pub use bevy;
use crystalorb::Config;
pub use crystalorb_bevy_networking_turbulence;
pub use game::PlayerId;

pub const SERVER_PORT: u16 = 1212;
pub const TIMESTEP: f64 = 1.0 / 60.0;

pub fn crystal_orb_config() -> Config {
    Config {
        //lag_compensation_latency: (),
        //blend_latency: 0.001,
        timestep_seconds: TIMESTEP,
        //clock_sync_needed_sample_count: (),
        //clock_sync_assumed_outlier_rate: (),
        //clock_sync_request_period: (),
        //max_tolerable_clock_deviation: (),
        //snapshot_send_period: (),
        //update_delta_seconds_max: (),
        //timestamp_skip_threshold_seconds: (),
        //fastforward_max_per_step: (),
        //tweening_method: (),
        ..Default::default()
    }
}
