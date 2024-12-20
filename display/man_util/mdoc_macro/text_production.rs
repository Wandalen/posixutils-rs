use std::fmt::Display;

#[derive(Debug, PartialEq)]
pub enum AtAndTUnix {
    General,
    Version(String),
    V32,
    SystemIII,
    SystemV(Option<String>),
}

impl TryFrom<String> for AtAndTUnix {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "" => Ok(Self::General),
            "32v" => Ok(Self::V32),
            "III" => Ok(Self::SystemIII),
            "V" => Ok(Self::SystemV(None)),
            version if version.starts_with("v") => {
                if let Ok(v) = version[1..].parse::<u8>() {
                    if (1..=7).contains(&v) {
                        Ok(Self::Version(v.to_string()))
                    } else {
                        Err(format!("Invalid version for v[1-7]: {v}"))
                    }
                } else {
                    Err(format!("Invalid numeric value for v[1-7]: {}", version))
                }
            }
            version if version.starts_with("V.") => {
                if let Ok(v) = version[2..].parse::<u8>() {
                    if (1..=4).contains(&v) {
                        Ok(Self::SystemV(Some(v.to_string())))
                    } else {
                        Err(format!("Invalid version for V.[1-4]: {version}"))
                    }
                } else {
                    Err(format!("Invalid numeric value for V.[...]: {version}"))
                }
            }
            _ => Err(format!("Unrecognized .At argument: {value}")),
        }
    }
}

impl Display for AtAndTUnix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let at_n_t_unix = match self {
            AtAndTUnix::General => "AT&T UNIX".to_string(),
            AtAndTUnix::Version(v) => format!("Version {v} AT&T UNIX"),
            AtAndTUnix::V32 => "AT&T UNIX v32".to_string(),
            AtAndTUnix::SystemIII => "AT&T System III UNIX".to_string(),
            AtAndTUnix::SystemV(None) => "AT&T System V UNIX".to_string(),
            AtAndTUnix::SystemV(Some(v)) => format!("AT&T System V Release {v} UNIX"),
        };

        write!(f, "{at_n_t_unix}")
    }
}

#[derive(Debug, PartialEq)]
pub struct Bsd {
    version: Option<String>,
    variant: Option<String>,
}

impl TryFrom<String> for Bsd {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = value.split_whitespace().collect();

        let (version, variant) = match parts.as_slice() {
            [] => (None, None),
            [version] => (Some(version.to_string()), None),
            [version, variant] => (Some(version.to_string()), Some(variant.to_string())),
            _ => return Err(format!("Invalid Bx format: {value}")),
        };

        Ok(Self { version, variant })
    }
}

impl Display for Bsd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = self
            .version
            .as_ref()
            .map_or_else(|| "".to_string(), |v| v.to_string());
        let variant = self
            .variant
            .as_deref()
            .map_or_else(|| "".to_string(), |v| format!(" {v}"));

        write!(f, "{version}BSD{variant}")
    }
}

#[derive(Debug, PartialEq)]
pub struct BsdOs {
    version: Option<String>,
}

impl From<String> for BsdOs {
    fn from(value: String) -> Self {
        let version = if value.is_empty() { None } else { Some(value) };

        Self { version }
    }
}

impl Display for BsdOs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = self
            .version
            .as_ref()
            .map_or_else(|| "".to_string(), |v| format!(" {v}"));

        write!(f, "BSD/OS{version}")
    }
}

#[derive(Debug, PartialEq)]
pub struct NetBsd {
    version: Option<String>,
}

impl From<String> for NetBsd {
    fn from(value: String) -> Self {
        let version = if value.is_empty() { None } else { Some(value) };

        Self { version }
    }
}

impl Display for NetBsd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = self
            .version
            .as_ref()
            .map_or_else(|| "".to_string(), |v| format!(" {v}"));

        write!(f, "NetBSD{version}")
    }
}

#[derive(Debug, PartialEq)]
pub struct FreeBsd {
    version: Option<String>,
}

impl From<String> for FreeBsd {
    fn from(value: String) -> Self {
        let version = if value.is_empty() { None } else { Some(value) };

        Self { version }
    }
}

impl Display for FreeBsd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = self
            .version
            .as_ref()
            .map_or_else(|| "".to_string(), |v| format!(" {v}"));

        write!(f, "FreeBSD{version}")
    }
}

#[derive(Debug, PartialEq)]
pub struct OpenBsd {
    version: Option<String>,
}

impl From<String> for OpenBsd {
    fn from(value: String) -> Self {
        let version = if value.is_empty() { None } else { Some(value) };

        Self { version }
    }
}

impl Display for OpenBsd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = self
            .version
            .as_ref()
            .map_or_else(|| "".to_string(), |v| format!(" {v}"));

        write!(f, "OpenBSD{version}")
    }
}

#[derive(Debug, PartialEq)]
pub struct DragonFly {
    version: Option<String>,
}

impl From<String> for DragonFly {
    fn from(value: String) -> Self {
        let version = if value.is_empty() { None } else { Some(value) };

        Self { version }
    }
}

impl Display for DragonFly {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = self
            .version
            .as_ref()
            .map_or_else(|| "".to_string(), |v| format!(" {v}"));

        write!(f, "DragonFly{version}")
    }
}

#[derive(Debug, PartialEq)]
pub enum Standard {
    // C Language Standards
    AnsiC,
    AnsiC89,
    IsoC,
    IsoC90,
    IsoCAmd1,
    IsoCTcor1,
    IsoCTcor2,
    IsoC99,
    IsoC2011,
    // POSIX.1 Standards before XPG4.2
    P1003188,
    P10031,
    P1003190,
    Iso9945190,
    P10031B93,
    P10031B,
    P10031C95,
    P10031I95,
    P1003196,
    Iso9945196,
    // X/Open Portability Guide before XPG4.2
    Xpg3,
    P10032,
    P1003292,
    Iso9945293,
    P10032A92,
    Xpg4,
    // X/Open Portability Guide Issue 4 Version 2 and Related Standards
    Susv1,
    Xpg42,
    XCurses42,
    P10031G2000,
    Svid4,
    // X/Open Portability Guide Issue 5 and Related Standards
    Susv2,
    Xbd5,
    Xsh5,
    Xcu5,
    Xns5,
    Xns52,
    // POSIX Issue 6 Standards
    P100312001,
    Susv3,
    P100312004,
    // POSIX Issues 7 and 8 Standards
    P100312008,
    Susv4,
    P100312024,
    // Other Standards
    Ieee754,
    Iso8601,
    Iso88023,
    Ieee127594,
}

impl TryFrom<String> for Standard {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            // C Language Standards
            "-ansiC" => Ok(Self::AnsiC),
            "-ansiC-89" => Ok(Self::AnsiC89),
            "-isoC" => Ok(Self::IsoC),
            "-isoC-90" => Ok(Self::IsoC90),
            "-isoC-amd1" => Ok(Self::IsoCAmd1),
            "-isoC-tcor1" => Ok(Self::IsoCTcor1),
            "-isoC-tcor2" => Ok(Self::IsoCTcor2),
            "-isoC-99" => Ok(Self::IsoC99),
            "-isoC-2011" => Ok(Self::IsoC2011),
            // POSIX.1 Standards before XPG4.2
            "-p1003.1-88" => Ok(Self::P1003188),
            "-p1003.1" => Ok(Self::P10031),
            "-p1003.1-90" => Ok(Self::P1003190),
            "-iso9945-1-90" => Ok(Self::Iso9945190),
            "-p1003.1b-93" => Ok(Self::P10031B93),
            "-p1003.1b" => Ok(Self::P10031B),
            "-p1003.1c-95" => Ok(Self::P10031C95),
            "-p1003.1i-95" => Ok(Self::P10031I95),
            "-p1003.1-96" => Ok(Self::P1003196),
            "-iso9945-1-96" => Ok(Self::Iso9945196),
            // X/Open Portability Guide before XPG4.2
            "-xpg3" => Ok(Self::Xpg3),
            "-p1003.2" => Ok(Self::P10032),
            "-p1003.2-92" => Ok(Self::P1003292),
            "-iso9945-2-93" => Ok(Self::Iso9945293),
            "-p1003.2a-92" => Ok(Self::P10032A92),
            "-xpg4" => Ok(Self::Xpg4),
            // X/Open Portability Guide Issue 4 Version 2 and Related Standards
            "-susv1" => Ok(Self::Susv1),
            "-xpg4.2" => Ok(Self::Xpg42),
            "-xcurses4.2" => Ok(Self::XCurses42),
            "-p1003.1g-2000" => Ok(Self::P10031G2000),
            "-svid4" => Ok(Self::Svid4),
            // X/Open Portability Guide Issue 5 and Related Standards
            "-susv2" => Ok(Self::Susv2),
            "-xbd5" => Ok(Self::Xbd5),
            "-xsh5" => Ok(Self::Xsh5),
            "-xcu5" => Ok(Self::Xcu5),
            "-xns5" => Ok(Self::Xns5),
            "-xns5.2" => Ok(Self::Xns52),
            // POSIX Issue 6 Standards
            "-p1003.1-2001" => Ok(Self::P100312001),
            "-susv3" => Ok(Self::Susv3),
            "-p1003.1-2004" => Ok(Self::P100312004),
            // POSIX Issues 7 and 8 Standards
            "-p1003.1-2008" => Ok(Self::P100312008),
            "-susv4" => Ok(Self::Susv4),
            "-p1003.1-2024" => Ok(Self::P100312024),
            // Other Standards
            "-ieee754" => Ok(Self::Ieee754),
            "-iso8601" => Ok(Self::Iso8601),
            "-iso8802-3" => Ok(Self::Iso88023),
            "-ieee1275-94" => Ok(Self::Ieee127594),
            // Error
            _ => Err(format!("Unrecognized .St standard abbreviation: {value}")),
        }
    }
}

impl Display for Standard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let standard = match self {
            // C Language Standards
            Standard::AnsiC => "ANSI X3.159-1989 (“ANSI C89”)".to_string(),
            Standard::AnsiC89 => "ANSI X3.159-1989 (“ANSI C89”)".to_string(),
            Standard::IsoC => "ISO/IEC 9899:1990 (“ISO C90”)".to_string(),
            Standard::IsoC90 => "ISO/IEC 9899:1990 (“ISO C90”)".to_string(),
            Standard::IsoCAmd1 => "ISO/IEC 9899/AMD1:1995 (“ISO C90, Amendment 1”)".to_string(),
            Standard::IsoCTcor1 => {
                "ISO/IEC 9899/TCOR1:1994 (“ISO C90, Technical Corrigendum 1”)".to_string()
            }
            Standard::IsoCTcor2 => {
                "ISO/IEC 9899/TCOR2:1995 (“ISO C90, Technical Corrigendum 2”)".to_string()
            }
            Standard::IsoC99 => "ISO/IEC 9899:1999 (“ISO C99”)".to_string(),
            Standard::IsoC2011 => "ISO/IEC 9899:2011 (“ISO C11”)".to_string(),
            // POSIX.1 Standards before XPG4.2
            Standard::P1003188 => "IEEE Std 1003.1-1988 (“POSIX.1”)".to_string(),
            Standard::P10031 => "IEEE Std 1003.1 (“POSIX.1”)".to_string(),
            Standard::P1003190 => "IEEE Std 1003.1-1990 (“POSIX.1”)".to_string(),
            Standard::Iso9945190 => "ISO/IEC 9945-1:1990 (“POSIX.1”)".to_string(),
            Standard::P10031B93 => "IEEE Std 1003.1b-1993 (“POSIX.1b”)".to_string(),
            Standard::P10031B => "IEEE Std 1003.1b (“POSIX.1b”)".to_string(),
            Standard::P10031C95 => "IEEE Std 1003.1c-1995 (“POSIX.1c”)".to_string(),
            Standard::P10031I95 => "IEEE Std 1003.1i-1995 (“POSIX.1i”)".to_string(),
            Standard::P1003196 => "ISO/IEC 9945-1:1996 (“POSIX.1”)".to_string(),
            Standard::Iso9945196 => "ISO/IEC 9945-1:1996 (“POSIX.1”)".to_string(),
            // X/Open Portability Guide before XPG4.2
            Standard::Xpg3 => "X/Open Portability Guide Issue 3 (“XPG3”)".to_string(),
            Standard::P10032 => "IEEE Std 1003.2 (“POSIX.2”)".to_string(),
            Standard::P1003292 => "IEEE Std 1003.2-1992 (“POSIX.2”)".to_string(),
            Standard::Iso9945293 => "ISO/IEC 9945-2:1993 (“POSIX.2”)".to_string(),
            Standard::P10032A92 => "IEEE Std 1003.2a-1992 (“POSIX.2”)".to_string(),
            Standard::Xpg4 => "X/Open Portability Guide Issue 4 (“XPG4”)".to_string(),
            // X/Open Portability Guide Issue 4 Version 2 and Related Standards
            Standard::Susv1 => "Version 1 of the Single UNIX Specification (“SUSv1”)".to_string(),
            Standard::Xpg42 => "X/Open Portability Guide Issue 4, Version 2 (“XPG4.2”)".to_string(),
            Standard::XCurses42 => "X/Open Curses Issue 4, Version 2 (“XCURSES4.2”)".to_string(),
            Standard::P10031G2000 => "IEEE Std 1003.1g-2000 (“POSIX.1g”)".to_string(),
            Standard::Svid4 => {
                "System V Interface Definition, Fourth Edition (“SVID4”)".to_string()
            }
            // X/Open Portability Guide Issue 5 and Related Standards
            Standard::Susv2 => "Version 2 of the Single UNIX Specification (“SUSv2”)".to_string(),
            Standard::Xbd5 => "X/Open Base Definitions Issue 5 (“XBD5”)".to_string(),
            Standard::Xsh5 => "X/Open System Interfaces and Headers Issue 5 (“XSH5”)".to_string(),
            Standard::Xcu5 => "X/Open Commands and Utilities Issue 5 (“XCU5”)".to_string(),
            Standard::Xns5 => "X/Open Networking Services Issue 5 (“XNS5”)".to_string(),
            Standard::Xns52 => "X/Open Networking Services Issue 5.2 (“XNS5.2”)".to_string(),
            // POSIX Issue 6 Standards
            Standard::P100312001 => "IEEE Std 1003.1-2001 (“POSIX.1”)".to_string(),
            Standard::Susv3 => "Version 3 of the Single UNIX Specification (“SUSv3”)".to_string(),
            Standard::P100312004 => "IEEE Std 1003.1-2004 (“POSIX.1”)".to_string(),
            // POSIX Issues 7 and 8 Standards
            Standard::P100312008 => "IEEE Std 1003.1-2008 (“POSIX.1”)".to_string(),
            Standard::Susv4 => "Version 4 of the Single UNIX Specification (“SUSv4”)".to_string(),
            // TODO: documentation doesn't containt needed text.
            Standard::P100312024 => "".to_string(),
            // Other Standards
            Standard::Ieee754 => "IEEE Std 754-1985".to_string(),
            Standard::Iso8601 => "ISO 8601".to_string(),
            Standard::Iso88023 => "ISO 8802-3: 1989".to_string(),
            Standard::Ieee127594 => "IEEE Std 1275-1994 (“Open Firmware”)".to_string(),
        };

        write!(f, "{standard}")
    }
}
