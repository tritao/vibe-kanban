use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{LogMode, LogRenderMode, LogViewMode};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct TuiPrefs {
    pub(crate) selected_project_id: Option<Uuid>,
    pub(crate) show_cancelled: bool,
    pub(crate) log_mode: LogMode,
    pub(crate) log_render_mode: LogRenderMode,
    pub(crate) log_view_mode: LogViewMode,
    pub(crate) diff_theme: DiffTheme,
    pub(crate) diff_wrap: bool,
}

impl Default for TuiPrefs {
    fn default() -> Self {
        Self {
            selected_project_id: None,
            show_cancelled: false,
            log_mode: LogMode::Normalized,
            log_render_mode: LogRenderMode::Markdown,
            log_view_mode: LogViewMode::Timeline,
            diff_theme: DiffTheme::default(),
            diff_wrap: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiffTheme {
    Base16OceanDark,
    Base16EightiesDark,
    Base16MochaDark,
    Base16OceanLight,
    InspiredGithub,
    SolarizedDark,
    SolarizedLight,
}

impl Default for DiffTheme {
    fn default() -> Self {
        Self::InspiredGithub
    }
}

impl DiffTheme {
    pub(crate) fn is_light(self) -> bool {
        matches!(
            self,
            Self::Base16OceanLight | Self::InspiredGithub | Self::SolarizedLight
        )
    }

    pub(crate) fn syntect_key(self) -> &'static str {
        match self {
            Self::Base16OceanDark => "base16-ocean.dark",
            Self::Base16EightiesDark => "base16-eighties.dark",
            Self::Base16MochaDark => "base16-mocha.dark",
            Self::Base16OceanLight => "base16-ocean.light",
            Self::InspiredGithub => "InspiredGitHub",
            Self::SolarizedDark => "Solarized (dark)",
            Self::SolarizedLight => "Solarized (light)",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Base16OceanDark => "ocean.dark",
            Self::Base16EightiesDark => "eighties.dark",
            Self::Base16MochaDark => "mocha.dark",
            Self::Base16OceanLight => "ocean.light",
            Self::InspiredGithub => "github",
            Self::SolarizedDark => "solarized.dark",
            Self::SolarizedLight => "solarized.light",
        }
    }

    pub(crate) fn cycle_next(self) -> Self {
        match self {
            Self::InspiredGithub => Self::Base16OceanDark,
            Self::Base16OceanDark => Self::Base16EightiesDark,
            Self::Base16EightiesDark => Self::Base16MochaDark,
            Self::Base16MochaDark => Self::Base16OceanLight,
            Self::Base16OceanLight => Self::SolarizedDark,
            Self::SolarizedDark => Self::SolarizedLight,
            Self::SolarizedLight => Self::InspiredGithub,
        }
    }
}
