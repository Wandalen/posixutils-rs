//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::fmt::Display;

#[derive(Debug, PartialEq)]
pub enum AtType {
    General,
    Version(String),
    V32,
    SystemIII,
    SystemV(Option<String>),
}

impl TryFrom<&str> for AtType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
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

impl Display for AtType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let at_n_t_unix = match self {
            AtType::General => "AT&T UNIX".to_string(),
            AtType::Version(v) => format!("Version {v} AT&T UNIX"),
            AtType::V32 => "AT&T UNIX v32".to_string(),
            AtType::SystemIII => "AT&T System III UNIX".to_string(),
            AtType::SystemV(None) => "AT&T System V UNIX".to_string(),
            AtType::SystemV(Some(v)) => format!("AT&T System V Release {v} UNIX"),
        };

        write!(f, "{at_n_t_unix}")
    }
}

#[derive(Debug, PartialEq)]
pub struct BsxType {
    pub version: Vec<String>,
}

impl Display for BsxType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = if self.version.is_empty() {
            "".to_string()
        } else {
            format!(" {}", self.version.join(" "))
        };

        write!(f, "BSD/OS{version}")
    }
}

#[derive(Debug, PartialEq)]
pub struct BxType {
    pub version: Option<String>,
    pub variant: Vec<String>,
}

impl Display for BxType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = self
            .version
            .as_ref()
            .map_or_else(|| "".to_string(), |v| v.to_string());
        let variant = if self.variant.is_empty() {
            "".to_string()
        } else {
            format!(" {}", self.variant.join(" "))
        };

        write!(f, "{version}BSD{variant}")
    }
}

#[derive(Debug, PartialEq)]
pub struct DxType {
    pub version: Vec<String>,
}

impl Display for DxType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = if self.version.is_empty() {
            "".to_string()
        } else {
            format!(" {}", self.version.join(" "))
        };

        write!(f, "DragonFly{version}")
    }
}

#[derive(Debug, PartialEq)]
pub struct FxType {
    pub version: Vec<String>,
}

impl Display for FxType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = if self.version.is_empty() {
            "".to_string()
        } else {
            format!(" {}", self.version.join(" "))
        };

        write!(f, "FreeBSD{version}")
    }
}

#[derive(Debug, PartialEq)]
pub struct NxType {
    pub version: Vec<String>,
}

impl Display for NxType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = if self.version.is_empty() {
            "".to_string()
        } else {
            format!(" {}", self.version.join(" "))
        };

        write!(f, "NetBSD{version}")
    }
}

#[derive(Debug, PartialEq)]
pub struct OxType {
    pub version: Vec<String>,
}

impl Display for OxType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = if self.version.is_empty() {
            "".to_string()
        } else {
            format!(" {}", self.version.join(" "))
        };

        write!(f, "OpenBSD{version}")
    }
}

#[derive(Debug, PartialEq)]
pub enum StType {
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

impl TryFrom<&str> for StType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
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

impl Display for StType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let standard = match self {
            // C Language Standards
            StType::AnsiC => "ANSI X3.159-1989 (“ANSI C89”)".to_string(),
            StType::AnsiC89 => "ANSI X3.159-1989 (“ANSI C89”)".to_string(),
            StType::IsoC => "ISO/IEC 9899:1990 (“ISO C90”)".to_string(),
            StType::IsoC90 => "ISO/IEC 9899:1990 (“ISO C90”)".to_string(),
            StType::IsoCAmd1 => "ISO/IEC 9899/AMD1:1995 (“ISO C90, Amendment 1”)".to_string(),
            StType::IsoCTcor1 => {
                "ISO/IEC 9899/TCOR1:1994 (“ISO C90, Technical Corrigendum 1”)".to_string()
            }
            StType::IsoCTcor2 => {
                "ISO/IEC 9899/TCOR2:1995 (“ISO C90, Technical Corrigendum 2”)".to_string()
            }
            StType::IsoC99 => "ISO/IEC 9899:1999 (“ISO C99”)".to_string(),
            StType::IsoC2011 => "ISO/IEC 9899:2011 (“ISO C11”)".to_string(),
            // POSIX.1 Standards before XPG4.2
            StType::P1003188 => "IEEE Std 1003.1-1988 (“POSIX.1”)".to_string(),
            StType::P10031 => "IEEE Std 1003.1 (“POSIX.1”)".to_string(),
            StType::P1003190 => "IEEE Std 1003.1-1990 (“POSIX.1”)".to_string(),
            StType::Iso9945190 => "ISO/IEC 9945-1:1990 (“POSIX.1”)".to_string(),
            StType::P10031B93 => "IEEE Std 1003.1b-1993 (“POSIX.1b”)".to_string(),
            StType::P10031B => "IEEE Std 1003.1b (“POSIX.1b”)".to_string(),
            StType::P10031C95 => "IEEE Std 1003.1c-1995 (“POSIX.1c”)".to_string(),
            StType::P10031I95 => "IEEE Std 1003.1i-1995 (“POSIX.1i”)".to_string(),
            StType::P1003196 => "ISO/IEC 9945-1:1996 (“POSIX.1”)".to_string(),
            StType::Iso9945196 => "ISO/IEC 9945-1:1996 (“POSIX.1”)".to_string(),
            // X/Open Portability Guide before XPG4.2
            StType::Xpg3 => "X/Open Portability Guide Issue 3 (“XPG3”)".to_string(),
            StType::P10032 => "IEEE Std 1003.2 (“POSIX.2”)".to_string(),
            StType::P1003292 => "IEEE Std 1003.2-1992 (“POSIX.2”)".to_string(),
            StType::Iso9945293 => "ISO/IEC 9945-2:1993 (“POSIX.2”)".to_string(),
            StType::P10032A92 => "IEEE Std 1003.2a-1992 (“POSIX.2”)".to_string(),
            StType::Xpg4 => "X/Open Portability Guide Issue 4 (“XPG4”)".to_string(),
            // X/Open Portability Guide Issue 4 Version 2 and Related Standards
            StType::Susv1 => "Version 1 of the Single UNIX Specification (“SUSv1”)".to_string(),
            StType::Xpg42 => "X/Open Portability Guide Issue 4, Version 2 (“XPG4.2”)".to_string(),
            StType::XCurses42 => "X/Open Curses Issue 4, Version 2 (“XCURSES4.2”)".to_string(),
            StType::P10031G2000 => "IEEE Std 1003.1g-2000 (“POSIX.1g”)".to_string(),
            StType::Svid4 => "System V Interface Definition, Fourth Edition (“SVID4”)".to_string(),
            // X/Open Portability Guide Issue 5 and Related Standards
            StType::Susv2 => "Version 2 of the Single UNIX Specification (“SUSv2”)".to_string(),
            StType::Xbd5 => "X/Open Base Definitions Issue 5 (“XBD5”)".to_string(),
            StType::Xsh5 => "X/Open System Interfaces and Headers Issue 5 (“XSH5”)".to_string(),
            StType::Xcu5 => "X/Open Commands and Utilities Issue 5 (“XCU5”)".to_string(),
            StType::Xns5 => "X/Open Networking Services Issue 5 (“XNS5”)".to_string(),
            StType::Xns52 => "X/Open Networking Services Issue 5.2 (“XNS5.2”)".to_string(),
            // POSIX Issue 6 Standards
            StType::P100312001 => "IEEE Std 1003.1-2001 (“POSIX.1”)".to_string(),
            StType::Susv3 => "Version 3 of the Single UNIX Specification (“SUSv3”)".to_string(),
            StType::P100312004 => "IEEE Std 1003.1-2004 (“POSIX.1”)".to_string(),
            // POSIX Issues 7 and 8 Standards
            StType::P100312008 => "IEEE Std 1003.1-2008 (“POSIX.1”)".to_string(),
            StType::Susv4 => "Version 4 of the Single UNIX Specification (“SUSv4”)".to_string(),
            // TODO: documentation doesn't containt needed text.
            StType::P100312024 => "".to_string(),
            // Other Standards
            StType::Ieee754 => "IEEE Std 754-1985".to_string(),
            StType::Iso8601 => "ISO 8601".to_string(),
            StType::Iso88023 => "ISO 8802-3: 1989".to_string(),
            StType::Ieee127594 => "IEEE Std 1275-1994 (“Open Firmware”)".to_string(),
        };

        write!(f, "{standard}")
    }
}
