use serde::Serialize;

use super::hotkey::Hotkey;
use crate::world::Tab;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TabPhase {
    #[default]
    Closed,
    Connecting,
    Lobby,
    Joining,
    Alive,
    Spectating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabView {
    pub phase: TabPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TabFacts {
    pub dialing: bool,
    pub connected: bool,
    pub present: bool,
    pub joining: bool,
    pub dead: bool,
    pub spectating: bool,
}

impl TabView {
    pub fn open(self) -> bool {
        !matches!(self.phase, TabPhase::Closed | TabPhase::Connecting)
    }

    pub fn derive(facts: TabFacts) -> TabView {
        use TabPhase::*;
        let view = |phase: TabPhase| TabView { phase };
        match facts {
            TabFacts {
                connected: false,
                dialing,
                ..
            } => view(if dialing { Connecting } else { Closed }),
            TabFacts { present: true, .. } => view(Alive),
            TabFacts { joining: true, .. } => view(Joining),
            TabFacts { dead: true, .. }
            | TabFacts {
                spectating: true, ..
            } => view(Spectating),
            _ => view(Lobby),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnitMode {
    #[default]
    ServerUnits,
    TabsAsUnits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ControlMode {
    Single,
    Dual,
    TabsAsUnits,
}

pub fn control_mode(views: [TabView; 2], unit_mode: UnitMode) -> ControlMode {
    match (views[1].open(), unit_mode) {
        (false, _) => ControlMode::Single,
        (true, UnitMode::ServerUnits) => ControlMode::Dual,
        (true, UnitMode::TabsAsUnits) => ControlMode::TabsAsUnits,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Intent {
    #[default]
    Idle,
    Play,
    Spectate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    Ignore,
    SwitchTab,
    ChangeUnit,
    Join,
    TargetLargest,
    SplitAcross(u8, u8),
    OverlapSplit,
    OverlapSplitOnce,
    Parallel,
    Active,
}

pub(crate) fn route(hotkey: Hotkey, down: bool, unit_mode: UnitMode, second_open: bool) -> Route {
    let units = unit_mode == UnitMode::TabsAsUnits;
    let units_dual = units && second_open;
    let switch_key = if units {
        Hotkey::ChangeUnit
    } else {
        Hotkey::Tab
    };
    let on_down = |route: Route| if down { route } else { Route::Ignore };
    match hotkey {
        h if h == switch_key => on_down(Route::SwitchTab),
        Hotkey::Tab | Hotkey::ChangeUnit => on_down(Route::ChangeUnit),
        Hotkey::StartNewGame => on_down(Route::Join),
        Hotkey::ToggleSpectateTarget => on_down(Route::TargetLargest),
        Hotkey::TestSplit if down && units_dual => Route::SplitAcross(4, 4),
        Hotkey::InfernoSplit if down && units_dual => Route::SplitAcross(2, 4),
        Hotkey::OverlapSplit if down && units_dual => Route::OverlapSplit,
        Hotkey::OverlapSplitOnce if down && units_dual => Route::OverlapSplitOnce,
        Hotkey::Parallel => Route::Parallel,
        _ => Route::Active,
    }
}

pub fn other(tab: Tab) -> Tab {
    match tab {
        Tab::Main => Tab::Second,
        Tab::Second => Tab::Main,
    }
}
