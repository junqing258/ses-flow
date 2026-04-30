//! Workstation 插件服务层入口。
//! 按职责拆分 AppState 的实现，并集中 re-export 控制器需要的类型和函数。

mod auth;
mod events;
mod runner;
mod state;
mod station;
mod task;
mod util;

pub(crate) use state::AppState;
pub(crate) use station::station_id_from_connect;
pub(crate) use task::PendingRobotDeparture;
