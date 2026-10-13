use iced::{Color, Theme, theme::Mode};
use repostatus::storage::ThemePreference;

pub fn resolve(preference: ThemePreference, system: Mode) -> Theme {
    match preference {
        ThemePreference::Light => Theme::Light,
        ThemePreference::Dark => Theme::Dark,
        ThemePreference::System if system == Mode::Dark => Theme::Dark,
        ThemePreference::System => Theme::Light,
    }
}

#[derive(Clone, Copy)]
pub struct Colors {
    pub bg: Color,
    pub header: Color,
    pub stripe: Color,
    pub line: Color,
    pub ink: Color,
    pub muted: Color,
    pub dim: Color,
    pub green: Color,
    pub blue: Color,
    pub amber: Color,
    pub red: Color,
}

impl Colors {
    pub fn for_theme(theme: &Theme) -> Self {
        if theme.extended_palette().is_dark {
            Self {
                bg: Color::from_rgb(0.065, 0.078, 0.082),
                header: Color::from_rgb(0.085, 0.101, 0.106),
                stripe: Color::from_rgb(0.078, 0.092, 0.097),
                line: Color::from_rgb(0.20, 0.25, 0.26),
                ink: Color::from_rgb(0.82, 0.85, 0.81),
                muted: Color::from_rgb(0.48, 0.56, 0.55),
                dim: Color::from_rgb(0.30, 0.36, 0.36),
                green: Color::from_rgb(0.67, 0.81, 0.61),
                blue: Color::from_rgb(0.51, 0.74, 0.79),
                amber: Color::from_rgb(0.83, 0.71, 0.52),
                red: Color::from_rgb(0.86, 0.53, 0.53),
            }
        } else {
            Self {
                bg: Color::from_rgb8(246, 247, 242),
                header: Color::from_rgb8(238, 241, 235),
                stripe: Color::from_rgb8(233, 238, 231),
                line: Color::from_rgb8(193, 204, 196),
                ink: Color::from_rgb8(37, 54, 47),
                muted: Color::from_rgb8(82, 99, 91),
                dim: Color::from_rgb8(146, 158, 148),
                green: Color::from_rgb8(50, 96, 46),
                blue: Color::from_rgb8(36, 100, 121),
                amber: Color::from_rgb8(128, 91, 29),
                red: Color::from_rgb8(170, 54, 54),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_preferences_ignore_system_and_system_follows_changes() {
        for mode in [Mode::None, Mode::Light, Mode::Dark] {
            assert_eq!(resolve(ThemePreference::Light, mode), Theme::Light);
            assert_eq!(resolve(ThemePreference::Dark, mode), Theme::Dark);
        }
        assert_eq!(resolve(ThemePreference::System, Mode::Light), Theme::Light);
        assert_eq!(resolve(ThemePreference::System, Mode::Dark), Theme::Dark);
        assert_eq!(resolve(ThemePreference::System, Mode::None), Theme::Light);
    }
}
