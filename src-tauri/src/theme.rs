use crate::config::Theme;

pub struct Palette {
    pub primary: &'static str,
    pub secondary: &'static str,
    pub secondary_alt: &'static str,
    pub tertiary: &'static str,
    pub floating: &'static str,
    pub text: &'static str,
    pub text_muted: &'static str,
    pub header: &'static str,
    pub accent: &'static str,
    pub hover: &'static str,
    pub active: &'static str,
    pub border: &'static str,
    pub dark: bool,
}

impl Theme {
    pub fn slug(self) -> &'static str {
        match self {
            Theme::Palladium => "palladium",
            Theme::Oxygen => "oxygen",
            Theme::Hydrogen => "hydrogen",
            Theme::Plutonium => "plutonium",
            Theme::Uranium => "uranium",
            Theme::Iron => "iron",
            Theme::Gallium => "gallium",
        }
    }

    pub fn palette(self) -> Palette {
        match self {
            Theme::Palladium => Palette {
                primary: "#12071f",
                secondary: "#180c28",
                secondary_alt: "#1d0f31",
                tertiary: "#0a0413",
                floating: "#070210",
                text: "#e2d4f0",
                text_muted: "#9b7cc0",
                header: "#f3e9ff",
                accent: "#a855f7",
                hover: "rgba(216,180,254,0.07)",
                active: "rgba(216,180,254,0.12)",
                border: "rgba(140,60,200,0.35)",
                dark: true,
            },
            Theme::Oxygen => Palette {
                primary: "#ffffff",
                secondary: "#f4f4f6",
                secondary_alt: "#eaeaee",
                tertiary: "#f7f7f9",
                floating: "#ffffff",
                text: "#18181b",
                text_muted: "#6b7280",
                header: "#09090b",
                accent: "#3f3f46",
                hover: "rgba(0,0,0,0.05)",
                active: "rgba(0,0,0,0.09)",
                border: "rgba(0,0,0,0.12)",
                dark: false,
            },
            Theme::Hydrogen => Palette {
                primary: "#0a0a0b",
                secondary: "#111113",
                secondary_alt: "#18181b",
                tertiary: "#000000",
                floating: "#000000",
                text: "#fafafa",
                text_muted: "#a1a1aa",
                header: "#ffffff",
                accent: "#71717a",
                hover: "rgba(255,255,255,0.06)",
                active: "rgba(255,255,255,0.1)",
                border: "rgba(255,255,255,0.12)",
                dark: true,
            },
            Theme::Plutonium => Palette {
                primary: "#0a1230",
                secondary: "#0d1838",
                secondary_alt: "#122045",
                tertiary: "#060c20",
                floating: "#04091a",
                text: "#eaf1ff",
                text_muted: "#8fa9d8",
                header: "#ffffff",
                accent: "#3b7de0",
                hover: "rgba(107,163,255,0.08)",
                active: "rgba(107,163,255,0.14)",
                border: "rgba(80,130,220,0.35)",
                dark: true,
            },
            Theme::Uranium => Palette {
                primary: "#062617",
                secondary: "#08301d",
                secondary_alt: "#0b3d26",
                tertiary: "#031710",
                floating: "#02120b",
                text: "#e8fff2",
                text_muted: "#79c99a",
                header: "#ffffff",
                accent: "#1f9d55",
                hover: "rgba(95,255,160,0.08)",
                active: "rgba(95,255,160,0.14)",
                border: "rgba(60,200,120,0.32)",
                dark: true,
            },
            Theme::Iron => Palette {
                primary: "#141417",
                secondary: "#1a1a1e",
                secondary_alt: "#232328",
                tertiary: "#0b0b0d",
                floating: "#08080a",
                text: "#f4f4f5",
                text_muted: "#8b8f98",
                header: "#ffffff",
                accent: "#6b7280",
                hover: "rgba(255,255,255,0.05)",
                active: "rgba(255,255,255,0.09)",
                border: "rgba(255,255,255,0.1)",
                dark: true,
            },
            Theme::Gallium => Palette {
                primary: "#000000",
                secondary: "#000000",
                secondary_alt: "#000000",
                tertiary: "#000000",
                floating: "#000000",
                text: "#ffffff",
                text_muted: "rgba(255,255,255,0.62)",
                header: "#ffffff",
                accent: "#ffffff",
                hover: "rgba(255,255,255,0.08)",
                active: "rgba(255,255,255,0.14)",
                border: "rgba(255,255,255,0.5)",
                dark: true,
            },
        }
    }
}

pub fn discord_css(theme: Theme) -> String {
    let p = theme.palette();
    let scheme = if p.dark { "dark" } else { "light" };

    let outlines = if matches!(theme, Theme::Gallium) {
        format!(
            "[class*=\"scroller\"] > [class*=\"container\"],\
             [class*=\"chat_\"],[class*=\"sidebar\"],[class*=\"guilds_\"],\
             [class*=\"panels_\"],[class*=\"membersWrap\"],[class*=\"channelTextArea\"],\
             [class*=\"searchBar\"],[class*=\"card_\"],[class*=\"modal\"]\
             {{border:1px solid {border} !important}}",
            border = p.border
        )
    } else {
        String::new()
    };

    format!(
        ":root,.theme-dark,.theme-light,.visual-refresh{{\
color-scheme:{scheme};\
--background-primary:{primary};--background-secondary:{secondary};\
--background-secondary-alt:{secondary_alt};--background-tertiary:{tertiary};\
--background-floating:{floating};--background-nested-floating:{floating};\
--background-accent:{accent};--background-mentioned:{active};\
--background-mentioned-hover:{hover};\
--background-modifier-hover:{hover};--background-modifier-active:{active};\
--background-modifier-selected:{active};--background-modifier-accent:{border};\
--bg-base-primary:{primary};--bg-base-secondary:{secondary};\
--bg-base-tertiary:{tertiary};--bg-surface-overlay:{floating};\
--bg-overlay-chat:{primary};--bg-mod-faint:{hover};--bg-mod-subtle:{hover};\
--bg-mod-strong:{active};--bg-mod-chat:{secondary};\
--text-normal:{text};--text-default:{text};--text-muted:{text_muted};\
--text-secondary:{text_muted};--text-feedback-muted:{text_muted};\
--header-primary:{header};--header-secondary:{text_muted};\
--interactive-normal:{text};--interactive-hover:{header};\
--interactive-active:{header};--interactive-muted:{text_muted};\
--channels-default:{text_muted};--channel-icon:{text_muted};\
--channeltextarea-background:{secondary_alt};\
--chat-background:{primary};--chat-input-container-background:{primary};\
--brand-experiment:{accent};--brand-500:{accent};--brand-experiment-500:{accent};\
--button-filled-brand-background:{accent};\
--border-subtle:{border};--border-faint:{border};--border-strong:{border};\
--scrollbar-thin-thumb:{border};--scrollbar-auto-thumb:{border};\
--scrollbar-thin-track:transparent;--scrollbar-auto-track:transparent;\
--elevation-low:none;--elevation-medium:none;--elevation-high:none\
}}{outlines}",
        scheme = scheme,
        primary = p.primary,
        secondary = p.secondary,
        secondary_alt = p.secondary_alt,
        tertiary = p.tertiary,
        floating = p.floating,
        accent = p.accent,
        text = p.text,
        text_muted = p.text_muted,
        header = p.header,
        hover = p.hover,
        active = p.active,
        border = p.border,
        outlines = outlines,
    )
}
