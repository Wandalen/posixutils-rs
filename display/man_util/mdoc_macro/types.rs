#[derive(Debug, PartialEq)]
pub enum BdType {
    Centered,
    Filled,
    Literal,
    Ragged,
    Unfilled,
}

impl TryFrom<String> for BdType {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "-centered" => Ok(Self::Centered),
            "-filled" => Ok(Self::Filled),
            "-literal" => Ok(Self::Literal),
            "-ragged" => Ok(Self::Ragged),
            "-unfilled" => Ok(Self::Unfilled),
            _ => Err(format!("Unrecognized '.Bd' type argument: {value}")),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum OffsetType {
    Indent,
    IndentTwo,
    Left,
    Right,
    Value(String),
}

impl From<String> for OffsetType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "indent" => Self::Indent,
            "indent-two" => Self::IndentTwo,
            "left" => Self::Left,
            "right" => Self::Right,
            _ => Self::Value(value.to_string()),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum BfType {
    Emphasis,
    Literal,
    Symbolic,
}

impl TryFrom<String> for BfType {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "-centered" | "Em" => Ok(Self::Emphasis),
            "-literal" | "Li" => Ok(Self::Literal),
            "-symbolic" | "Sy" => Ok(Self::Symbolic),
            _ => Err(format!("Unrecognized '.Bf' type argument: {value}")),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum BlType {
    Bullet,
    Column,
    Dash,
    Diag,
    Enum,
    Hang,
    Inset,
    Item,
    Ohang,
    Tag,
}

impl TryFrom<String> for BlType {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "-bullet" => Ok(Self::Bullet),
            "-column" => Ok(Self::Column),
            "-dash" | "-hyphen" => Ok(Self::Dash),
            "-diag" => Ok(Self::Diag),
            "-enum" => Ok(Self::Enum),
            "-hang" => Ok(Self::Hang),
            "-inset" => Ok(Self::Inset),
            "-item" => Ok(Self::Item),
            "-ohang" => Ok(Self::Ohang),
            "-tag" => Ok(Self::Tag),
            _ => Err(format!("Unrecognized '.Bl' type argument: {value}")),
        }
    }
}

#[derive(Debug, PartialEq)]

pub enum ItType {
    MandatoryArgs(Vec<String>),
    OptionalArgs(Vec<String>),
    None,
    Cell { cells: Vec<String> },
}

#[derive(Debug, PartialEq)]
pub enum SmMode {
    On,
    Off,
}

#[derive(Debug, PartialEq)]
pub enum RsSubmacro {
    A {
        author_names: Vec<String>,
    },
    B {
        book_title: String,
    },
    C {
        publication_location: String,
    },
    D {
        month_day: Option<(String, String)>,
        year: i32,
    },
    I {
        issuer_name: String,
    },
    J {
        journal_name: String,
    },
    N {
        issue_number: u16,
    },
    O {
        information: String,
    },
    P {
        page_number: String,
    },
    Q {
        insitution_author: String,
    },
    R {
        report_name: String,
    },
    T {
        article_title: String,
    },
    U {
        uri: String,
    },
    V {
        volume_number: u16,
    },
}
