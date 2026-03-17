use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Default,
    Dracula,
    Nord,
}

impl Theme {
    pub fn label(&self) -> &'static str {
        match self {
            Theme::Default => "Default",
            Theme::Dracula => "Dracula",
            Theme::Nord => "Nord",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Theme::Default => Theme::Dracula,
            Theme::Dracula => Theme::Nord,
            Theme::Nord => Theme::Default,
        }
    }

    pub fn colors(&self) -> ThemeColors {
        match self {
            Theme::Default => default_colors(),
            Theme::Dracula => dracula_colors(),
            Theme::Nord => nord_colors(),
        }
    }
}

pub struct ThemeColors {
    pub bg: Color,
    pub fg: Color,
    pub muted: Color,
    pub border: Color,
    pub highlight_bg: Color,
    pub accent: Color,
    pub good: Color,
    pub warning: Color,
    pub error: Color,
    pub status_bg: Color,
    pub status_fg: Color,
    pub header_fg: Color,
}

fn default_colors() -> ThemeColors {
    ThemeColors {
        bg: Color::Reset,
        fg: Color::Reset,
        muted: Color::DarkGray,
        border: Color::DarkGray,
        highlight_bg: Color::Rgb(60, 60, 60),
        accent: Color::Cyan,
        good: Color::Green,
        warning: Color::Yellow,
        error: Color::Red,
        status_bg: Color::Green,
        status_fg: Color::Black,
        header_fg: Color::Cyan,
    }
}

fn dracula_colors() -> ThemeColors {
    ThemeColors {
        bg: Color::Reset,
        fg: Color::Rgb(200, 200, 210),
        muted: Color::Rgb(120, 130, 160),
        border: Color::Rgb(100, 110, 140),
        highlight_bg: Color::Rgb(60, 62, 80),
        accent: Color::Rgb(150, 210, 240),
        good: Color::Rgb(130, 220, 150),
        warning: Color::Rgb(220, 220, 160),
        error: Color::Rgb(240, 120, 120),
        status_bg: Color::Rgb(170, 140, 220),
        status_fg: Color::Rgb(30, 30, 40),
        header_fg: Color::Rgb(150, 210, 240),
    }
}

fn nord_colors() -> ThemeColors {
    ThemeColors {
        bg: Color::Reset,
        fg: Color::Rgb(200, 210, 220),
        muted: Color::Rgb(110, 120, 140),
        border: Color::Rgb(100, 110, 130),
        highlight_bg: Color::Rgb(55, 62, 78),
        accent: Color::Rgb(140, 190, 210),
        good: Color::Rgb(160, 190, 150),
        warning: Color::Rgb(220, 200, 150),
        error: Color::Rgb(190, 110, 120),
        status_bg: Color::Rgb(130, 165, 195),
        status_fg: Color::Rgb(40, 45, 55),
        header_fg: Color::Rgb(140, 190, 210),
    }
}
