use ratatui::style::{Color, Style};
use std::{env, fs, path::PathBuf, str::FromStr};

pub fn color(s: &str) -> Result<Color, String> {
    if matches!(s, "reset" | "default" | "none" | "transparent") {
        return Ok(Color::Reset);
    }
    if let Some(rgb) = s.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let rgb: Vec<u8> = rgb
            .split(',')
            .map(|s| s.trim().parse::<u8>())
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        return match rgb.as_slice() {
            [r, g, b] => Ok(Color::Rgb(*r, *g, *b)),
            _ => Err("Expected three RGB channels".into()),
        };
    }
    Color::from_str(s).map_err(|e| e.to_string())
}

#[derive(Clone)]
pub struct Theme {
    pub name: String,
    pub bg: Color,
    pub fg: Color,
    pub accent: Color,
    pub selected: Color,
    pub muted: Color,
    pub surface: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
}

pub fn config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"))
}
pub fn state_home() -> PathBuf {
    env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".local/state")
        })
}
impl Theme {
    pub fn load(name: &str) -> Result<Self, String> {
        let raw = match name {
            "daylight" => include_str!("../themes/backstage-daylight/colors.toml").to_owned(),
            "midnight" => include_str!("../themes/backstage-midnight/colors.toml").to_owned(),
            "omarchy" => fs::read_to_string(state_home().join("omarchy/current/theme/colors.toml"))
                .map_err(|e| e.to_string())?,
            "herdr" => return Self::herdr(),
            path => fs::read_to_string(path).map_err(|e| format!("Theme {path}: {e}"))?,
        };
        Self::parse(name, &raw)
    }
    pub fn parse(name: &str, raw: &str) -> Result<Self, String> {
        let v: toml::Value = toml::from_str(raw).map_err(|e| e.to_string())?;
        let get = |k: &str| -> Result<Color, String> {
            color(
                v.get(k)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("Theme needs {k}"))?,
            )
        };
        Ok(Self {
            name: name.to_owned(),
            bg: get("background")?,
            fg: get("foreground")?,
            accent: get("accent")?,
            selected: get("selection")?,
            muted: get("muted")?,
            surface: get("dark_background")?,
            red: get("red")?,
            green: get("green")?,
            yellow: get("yellow")?,
            blue: get("blue")?,
            magenta: get("magenta")?,
        })
    }
    fn herdr() -> Result<Self, String> {
        let raw = fs::read_to_string(config_home().join("herdr/config.toml"))
            .map_err(|e| e.to_string())?;
        let v: toml::Value = toml::from_str(&raw).map_err(|e| e.to_string())?;
        let name = v
            .get("theme")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("terminal");
        // Herdr's terminal theme follows the active Omarchy terminal palette.
        let mut t = if name == "terminal" {
            Self::load("omarchy")?
        } else {
            let user = config_home()
                .join("omarchy/themes")
                .join(name)
                .join("colors.toml");
            let path = if user.is_file() {
                user
            } else {
                PathBuf::from("/usr/share/omarchy/themes")
                    .join(name)
                    .join("colors.toml")
            };
            Self::load(path.to_str().ok_or("Invalid theme path")?)?
        };
        if let Some(c) = v.get("theme").and_then(|v| v.get("custom")) {
            for (key, dest) in [
                ("panel_bg", &mut t.bg),
                ("text", &mut t.fg),
                ("accent", &mut t.accent),
                ("selection_bg", &mut t.selected),
                ("surface_dim", &mut t.surface),
                ("text_muted", &mut t.muted),
                ("red", &mut t.red),
                ("green", &mut t.green),
                ("yellow", &mut t.yellow),
                ("blue", &mut t.blue),
                ("magenta", &mut t.magenta),
            ] {
                if let Some(s) = c.get(key).and_then(|v| v.as_str()) {
                    *dest = color(s)?;
                }
            }
        }
        t.name = format!("herdr/{name}");
        Ok(t)
    }
    pub fn herdr_config(&self) -> String {
        let colors = [
            ("panel_bg", self.bg),
            ("text", self.fg),
            ("accent", self.accent),
            ("active_row_bg", self.selected),
            ("selection_bg", self.selected),
            ("surface_dim", self.surface),
            ("red", self.red),
            ("green", self.green),
            ("yellow", self.yellow),
            ("blue", self.blue),
            ("magenta", self.magenta),
        ];
        let mut out = String::from(
            "# Generated from Backstage's Omarchy colours; merge only these theme sections.\n[theme]\nname = \"terminal\"\nauto_switch = false\n\n[theme.custom]\n",
        );
        for (key, c) in colors {
            out.push_str(&format!("{key} = \"{c}\"\n"));
        }
        out
    }
    pub fn base(&self) -> Style {
        Style::default().fg(self.fg).bg(self.bg)
    }
    pub fn accent(&self) -> Style {
        self.base().fg(self.accent).bold()
    }
}
