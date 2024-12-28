use pest::{iterators::Pair, Parser};
use pest_derive::Parser;
use std::collections::HashSet;
use thiserror::Error;
use types::{BdType, BfType, OffsetType};

use super::mdoc_macro::types::*;
use super::mdoc_macro::*;

#[derive(Parser)]
#[grammar = "./man_util/mdoc.pest"]
pub struct MdocParser;

#[derive(Debug, PartialEq)]
pub struct MacroNode {
    mdoc_macro: Macro,
    nodes: Vec<Element>,
}

#[derive(Debug, PartialEq)]
pub enum Element {
    Text(String),
    Macro(MacroNode),
}

#[derive(Debug, PartialEq)]
pub struct MdocDocument {
    pub elements: Vec<Element>,
}

#[derive(Error, Debug, PartialEq)]
pub enum MdocError {
    #[error("mdoc: {0}")]
    PestError(#[from] Box<pest::error::Error<Rule>>),
    #[error("mdoc: {0}")]
    ParsingError(String),
    #[error("mdoc: {0}")]
    ValidationError(String),
    #[error("mdoc: {0}")]
    FormattingError(String),
}

#[derive(Default)]
struct MdocValidator {
    sh_titles: HashSet<String>,
    ss_titles: HashSet<String>,
    first_name: Option<Vec<String>>,
}

impl MdocValidator {
    fn validate_nm(&mut self, nm_node: &mut MacroNode) -> Result<(), MdocError> {
        if let Macro::Nm { ref mut name } = nm_node.mdoc_macro {
            match (&self.first_name, &name) {
                // Both remembered name and Nm name are present, or both are absent
                (Some(_), Some(_)) | (None, None) => {}
                // Nm has a name, but no remembered name
                (None, Some(name)) => {
                    self.first_name = Some(name.clone());
                }
                // Nm has no name, but remembered name is present
                (Some(_), None) => {
                    *name = self.first_name.clone();
                }
            }
        }
        Ok(())
    }

    fn validate_sh(&mut self, sh_node: &MacroNode) -> Result<(), MdocError> {
        fn is_last_element_nd(element: &Element) -> bool {
            match element {
                Element::Macro(MacroNode { mdoc_macro, nodes }) => {
                    if let Some(last_node) = nodes.last() {
                        // Рекурсивно перевіряємо останній дочірній вузол
                        is_last_element_nd(last_node)
                    } else {
                        // Якщо вузол пустий, перевіряємо сам макрос
                        matches!(mdoc_macro, Macro::Nd { .. })
                    }
                }
                _ => false,
            }
        }

        if let Macro::Sh { title } = &sh_node.mdoc_macro {
            if !self.sh_titles.insert(title.clone()) {
                return Err(MdocError::ValidationError(format!(
                    "Duplicate .Sh title found: {title}"
                )));
            }
            if title == "NAME" && !sh_node.nodes.is_empty() {
                let last_element = sh_node.nodes.last().unwrap();
                if !is_last_element_nd(last_element) {
                    return Err(MdocError::ValidationError(
                        ".Sh NAME must end with .Nd".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_ss(&mut self, ss_node: &MacroNode) -> Result<(), MdocError> {
        if let Macro::Ss { title } = &ss_node.mdoc_macro {
            if !self.ss_titles.insert(title.clone()) {
                return Err(MdocError::ValidationError(format!(
                    "Duplicate .Ss title found: {title}",
                )));
            }
        }
        Ok(())
    }

    fn validate_element(&mut self, element: &mut Element) -> Result<(), MdocError> {
        if let Element::Macro(macro_node) = element {
            match macro_node.mdoc_macro {
                Macro::Nm { .. } => self.validate_nm(macro_node)?,
                Macro::Sh { .. } => self.validate_sh(macro_node)?,
                Macro::Ss { .. } => self.validate_ss(macro_node)?,
                _ => {}
            }
        }

        // Recursively validate child nodes
        if let Element::Macro(MacroNode { nodes, .. }) = element {
            for child in nodes {
                self.validate_element(child)?;
            }
        }

        Ok(())
    }

    pub fn validate(&mut self, document: &mut MdocDocument) -> Result<(), MdocError> {
        for element in &mut document.elements {
            self.validate_element(element)?;
        }
        Ok(())
    }
}

impl MdocParser {
    /// Parses (`Bd`)[https://man.openbsd.org/mdoc#Bd]:
    /// `Bd -type [-offset width] [-compact]`
    fn parse_bd_block(pair: Pair<Rule>) -> Element {
        fn parse_bd_open(pair: Pair<Rule>) -> Macro {
            let mut inner = pair.into_inner();

            // -type
            let bd_type_pair = inner.next().expect("Expected '-type' for 'Bd'");
            let block_type = match BdType::try_from(bd_type_pair.as_str().to_string()) {
                Ok(bd_type) => bd_type,
                Err(err) => {
                    eprintln!("{err}");
                    BdType::Filled
                }
            };

            let mut offset: Option<OffsetType> = None;
            let mut compact = false;

            for opt_pair in inner {
                match opt_pair.as_rule() {
                    Rule::offset => offset = Some(OffsetType::from(opt_pair.as_str().to_string())),
                    Rule::compact => compact = true,
                    _ => {}
                }
            }

            Macro::Bd {
                block_type,
                offset,
                compact,
            }
        }

        let mut pairs = pair.into_inner();

        let bd_open = pairs
            .next()
            .expect("Expected '.Bd -type [-offset width] [-compact]'");
        let bd_macro = parse_bd_open(bd_open);

        let mut body_elements = vec![];
        for next_pair in pairs {
            if next_pair.as_rule() == Rule::ed_close {
                let node = MacroNode {
                    mdoc_macro: bd_macro,
                    nodes: body_elements,
                };
                return Element::Macro(node);
            } else {
                body_elements.push(Self::parse_element(next_pair));
            }
        }

        eprintln!("Error: Bd block not closed with Ed");
        Element::Text("[unclosed Bd block]".to_string())
    }

    /// Parses (`Bf`)[https://man.openbsd.org/mdoc#Bf]:
    /// `Bf -emphasis | -literal | -symbolic | Em | Li | Sy`
    fn parse_bf_block(pair: Pair<Rule>) -> Element {
        fn parse_bf_open(pair: Pair<Rule>) -> Macro {
            let mut inner = pair.into_inner();

            // -type
            let bf_type_pair = inner
                .next()
                .expect("Expected '-emphasis | -literal | -symbolic | Em | Li | Sy' for 'Bd'");
            let block_type = match BfType::try_from(bf_type_pair.as_str().to_string()) {
                Ok(bf_type) => bf_type,
                Err(err) => {
                    eprintln!("{err}");
                    BfType::Emphasis
                }
            };

            Macro::Bf(block_type)
        }

        let mut pairs = pair.into_inner();

        let bf_open = pairs
            .next()
            .expect("Expected '.Bf -emphasis | -literal | -symbolic | Em | Li | Sy'");
        let bf_macro = parse_bf_open(bf_open);

        let mut body_elements = vec![];
        for next_pair in pairs {
            if next_pair.as_rule() == Rule::ef_close {
                let node = MacroNode {
                    mdoc_macro: bf_macro,
                    nodes: body_elements,
                };
                return Element::Macro(node);
            } else {
                body_elements.push(Self::parse_element(next_pair));
            }
        }

        eprintln!("Error: Bf block not closed with Ef");
        Element::Text("[unclosed Bf block]".to_string())
    }

    /// Parses (`Bk`)[https://man.openbsd.org/mdoc#Bk]:
    /// `Bk -words`
    fn parse_bk_block(pair: Pair<Rule>) -> Element {
        let mut pairs = pair.into_inner();

        let bk_open = pairs.next().expect("Expected '.Bk -words'");
        bk_open
            .into_inner()
            .find(|p| p.as_rule() == Rule::bk_words)
            .expect("Mandatory argument '-words' is absent");

        let mut body_elements = vec![];
        for next_pair in pairs {
            if next_pair.as_rule() == Rule::ek_close {
                let node = MacroNode {
                    mdoc_macro: Macro::Bk,
                    nodes: body_elements,
                };
                return Element::Macro(node);
            } else {
                body_elements.push(Self::parse_element(next_pair));
            }
        }

        eprintln!("Error: Bk block not closed with Ek");
        Element::Text("[unclosed Bk block]".to_string())
    }

    // Parses (`Bl`)[https://man.openbsd.org/mdoc#Bl]
    // `Bl -type [-width val] [-offset val] [-compact] [col ...]`
    fn parse_bl_block(pair: Pair<Rule>) -> Element {
        fn parse_bl_open(pair: Pair<Rule>) -> Macro {
            let mut inner = pair.into_inner();

            // -type
            let bl_type_pair = inner.next().expect("Expected '-type' for 'Bl'");
            let list_type = match BlType::try_from(bl_type_pair.as_str().to_string()) {
                Ok(bl_type) => bl_type,
                Err(err) => {
                    eprintln!("{err}");
                    BlType::Bullet
                }
            };

            let mut offset: Option<OffsetType> = None;
            let mut compact = false;
            let mut columns = vec![];

            for opt_pair in inner {
                match opt_pair.as_rule() {
                    Rule::offset => offset = Some(OffsetType::from(opt_pair.as_str().to_string())),
                    Rule::compact => compact = true,
                    Rule::bl_columns => {
                        for col in opt_pair.into_inner() {
                            columns.push(col.as_str().to_string());
                        }
                    }
                    _ => {}
                }
            }

            Macro::Bl {
                list_type,
                offset,
                compact,
                columns,
            }
        }

        let mut pairs = pair.into_inner();

        let bl_open = pairs
            .next()
            .expect("Expected '.Bl -type [-width val] [-offset val] [-compact] [col ...]'");
        let bl_macro = parse_bl_open(bl_open);

        let mut body_elements = vec![];
        for next_pair in pairs {
            if next_pair.as_rule() == Rule::el_close {
                let node = MacroNode {
                    mdoc_macro: bl_macro,
                    nodes: body_elements,
                };
                return Element::Macro(node);
            } else {
                body_elements.push(Self::parse_element(next_pair));
            }
        }

        eprintln!("Error: Bl block not closed with El");
        Element::Text("[unclosed Bl block]".to_string())
    }

    fn parse_block_full_explicit(pair: Pair<Rule>) -> Element {
        let pair = pair.into_inner().next().unwrap();
        match pair.as_rule() {
            Rule::bd_block => Self::parse_bd_block(pair),
            Rule::bf_block => Self::parse_bf_block(pair),
            Rule::bk_block => Self::parse_bk_block(pair),
            Rule::bl_block => Self::parse_bl_block(pair),
            _ => Element::Text("Unsupported block".to_string()),
        }
    }

    // Parses (`Nd`)[https://man.openbsd.org/mdoc#Nd]
    // `Nd line`
    fn parse_nd(pair: Pair<Rule>) -> Element {
        let mut inner = pair.into_inner();

        let line = inner
            .next() // `nd_block` -> `nd_open`
            .unwrap()
            .into_inner()
            .next() // `nd_open` -> `nd_line`
            .expect("Expected title for 'Sh' block")
            .as_str()
            .trim_end()
            .to_string();

        let nodes = inner.map(|p| Self::parse_element(p)).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Nd {
                line: line.to_string(),
            },
            nodes,
        })
    }

    // Parses (`Nm`)[https://man.openbsd.org/mdoc#Nm]
    // `Nm [name]`
    fn parse_nm(pair: Pair<Rule>) -> Element {
        let mut inner = pair.into_inner();

        let name: Option<Vec<String>> = inner
            .next() // `nm_block` -> `nm_open`
            .unwrap()
            .into_inner()
            .next() // `nm_open` -> `nm_name`
            .map(|p| p.into_inner().map(|n| n.as_str().to_string()).collect());

        let nodes = inner.map(|p| Self::parse_element(p)).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Nm { name },
            nodes,
        })
    }

    // Parses (`Sh`)[https://man.openbsd.org/mdoc#Sh]
    // `Sh TITLE LINE`
    fn parse_sh_block(pair: Pair<Rule>) -> Element {
        let mut inner = pair.into_inner();

        let title = inner
            .next() // `sh_block` -> `sh_open`
            .unwrap()
            .into_inner()
            .next() // `sh_open` -> `sh_title_line`
            .expect("Expected title for 'Sh' block")
            .as_str()
            .trim_end()
            .to_string();

        let nodes = inner.map(|p| Self::parse_element(p)).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Sh { title },
            nodes,
        })
    }

    /// Parses (`Ss`)[https://man.openbsd.org/mdoc#Ss]:
    /// `Ss Title line`
    fn parse_ss_block(pair: Pair<Rule>) -> Element {
        let mut inner = pair.into_inner();

        let title = inner
            .next() // `ss_block` -> `ss_open`
            .unwrap()
            .into_inner()
            .next() // `ss_open` -> `ss_title_line`
            .expect("Expected title for 'Ss' block")
            .as_str()
            .trim_end()
            .to_string();

        let nodes = inner.map(|p| Self::parse_element(p)).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Ss { title },
            nodes,
        })
    }

    fn parse_block_full_implicit(pair: Pair<Rule>) -> Element {
        let pair = pair.into_inner().next().unwrap();
        match pair.as_rule() {
            Rule::nd_block => Self::parse_nd(pair),
            Rule::nm_block => Self::parse_nm(pair),
            Rule::sh_block => Self::parse_sh_block(pair),
            Rule::ss_block => Self::parse_ss_block(pair),
            _ => Element::Text("Unsupported block".to_string()),
        }
    }

    fn parse_element(pair: Pair<Rule>) -> Element {
        match pair.as_rule() {
            Rule::element => Self::parse_element(pair.into_inner().next().unwrap()),
            Rule::block_full_explicit => Self::parse_block_full_explicit(pair),
            Rule::block_full_implicit => Self::parse_block_full_implicit(pair),
            _ => Element::Text(pair.as_str().to_string()),
        }
    }

    pub fn parse_mdoc(input: impl AsRef<str>) -> Result<MdocDocument, MdocError> {
        let pairs = MdocParser::parse(Rule::document, input.as_ref())
            .map_err(|err| MdocError::PestError(Box::new(err)))?;
        println!("Pairs:\n{pairs:#?}\n\n");

        // Iterate each pair (macro or text element)
        let elements = pairs
            .flat_map(|p| {
                let inner_rules = p.into_inner();
                inner_rules.map(|p| Self::parse_element(p))
            })
            .collect();

        let mut mdoc = MdocDocument { elements };

        let validator = &mut MdocValidator::default();
        validator.validate(&mut mdoc)?;

        Ok(mdoc)
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::man_util::parser::*;

    // Block full-explicit
    #[test]
    fn bd() {
        let content = ".Bd -literal -offset indent -compact\nLine 1\nLine 2\n.Ed";

        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bd {
                block_type: BdType::Literal,
                offset: Some(OffsetType::Indent),
                compact: true,
            },
            nodes: vec![
                Element::Text("Line 1\n".to_string()),
                Element::Text("Line 2\n".to_string()),
            ],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn bd_no_closing_macro() {
        let content = ".Bd -literal -offset indent -compact\nLine 1\nLine 2\n";

        let mdoc = MdocParser::parse_mdoc(content);
        assert!(mdoc.is_err());
    }

    #[test]
    fn bd_foreign_closing_macros() {
        let closing_macros = vec![".Ef", ".Ek", ".El"];
        let content = ".Bd -literal -offset indent -compact\nLine 1\nLine 2\n";

        for closing_macro in closing_macros {
            let content = format!("{content}.{closing_macro}");
            let mdoc = MdocParser::parse_mdoc(content);
            assert!(mdoc.is_err());
        }
    }

    #[test]
    fn bd_no_body() {
        let content = ".Bd -literal\n.Ed";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bd {
                block_type: BdType::Literal,
                offset: None,
                compact: false,
            },
            nodes: vec![],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn bd_type() {
        let mut bd_types: HashMap<&str, BdType> = Default::default();
        bd_types.insert("-centered", BdType::Centered);
        bd_types.insert("-filled", BdType::Filled);
        bd_types.insert("-literal", BdType::Literal);
        bd_types.insert("-ragged", BdType::Ragged);
        bd_types.insert("-unfilled", BdType::Unfilled);
        bd_types.insert("invalid_value", BdType::Filled);

        for (str_type, enum_type) in bd_types {
            let content = format!(".Bd {str_type}\n.Ed\n");
            let element = Element::Macro(MacroNode {
                mdoc_macro: Macro::Bd {
                    block_type: enum_type,
                    offset: None,
                    compact: false,
                },
                nodes: vec![],
            });

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(
                *mdoc.elements.get(0).unwrap(),
                element,
                "Bd type: {str_type}"
            );
        }
    }

    #[test]
    fn bd_offset() {
        let mut offset_types: HashMap<&str, OffsetType> = Default::default();
        offset_types.insert("indent", OffsetType::Indent);
        offset_types.insert("indent-two", OffsetType::IndentTwo);
        offset_types.insert("left", OffsetType::Left);
        offset_types.insert("right", OffsetType::Right);
        offset_types.insert(
            "custom_value",
            OffsetType::Value("custom_value".to_string()),
        );

        for (str_type, enum_type) in offset_types {
            let content = format!(".Bd -literal -offset {str_type}\n.Ed\n");
            let element = Element::Macro(MacroNode {
                mdoc_macro: Macro::Bd {
                    block_type: BdType::Literal,
                    offset: Some(enum_type),
                    compact: false,
                },
                nodes: vec![],
            });

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(
                *mdoc.elements.get(0).unwrap(),
                element,
                "Bd offset: {str_type}"
            );
        }
    }

    #[test]
    fn bd_compact() {
        let content = ".Bd -literal -compact\n.Ed\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bd {
                block_type: BdType::Literal,
                offset: None,
                compact: true,
            },
            nodes: vec![],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn bf() {
        let content = ".Bf -emphasis\nLine 1\nLine 2\n.Ef";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bf(BfType::Emphasis),
            nodes: vec![
                Element::Text("Line 1\n".to_string()),
                Element::Text("Line 2\n".to_string()),
            ],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn bf_no_closing_macro() {
        let content = ".Bf -emphasis\nLine 1\nLine 2\n";

        let mdoc = MdocParser::parse_mdoc(content);
        assert!(mdoc.is_err());
    }

    #[test]
    fn bf_foreign_closing_macros() {
        let closing_macros = vec![".Ed", ".Ek", ".El"];
        let content = ".Bf -emphasis\nLine 1\nLine 2\n";

        for closing_macro in closing_macros {
            let content = format!("{content}.{closing_macro}");
            let mdoc = MdocParser::parse_mdoc(content);
            assert!(mdoc.is_err());
        }
    }

    #[test]
    fn bf_type() {
        let mut bf_types: HashMap<&str, BfType> = Default::default();
        bf_types.insert("-emphasis", BfType::Emphasis);
        bf_types.insert("Em", BfType::Emphasis);
        bf_types.insert("-literal", BfType::Literal);
        bf_types.insert("Li", BfType::Literal);
        bf_types.insert("-symbolic", BfType::Symbolic);
        bf_types.insert("Sy", BfType::Symbolic);
        bf_types.insert("invalid_value", BfType::Emphasis);

        for (str_type, enum_type) in bf_types {
            let content = format!(".Bf {str_type}\n.Ef\n");
            let element = Element::Macro(MacroNode {
                mdoc_macro: Macro::Bf(enum_type),
                nodes: vec![],
            });

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(
                *mdoc.elements.get(0).unwrap(),
                element,
                "Bf type: {str_type}"
            );
        }
    }

    #[test]
    fn bk() {
        let content = ".Bk -words\nLine 1\nLine 2\n.Ek";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bk,
            nodes: vec![
                Element::Text("Line 1\n".to_string()),
                Element::Text("Line 2\n".to_string()),
            ],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn bk_no_body() {
        let content = ".Bk -words\n.Ek\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bk,
            nodes: vec![],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn bk_no_words() {
        let content = ".Bk\n.Ek\n";

        let mdoc = MdocParser::parse_mdoc(content);
        assert!(mdoc.is_err());
    }

    #[test]
    fn bl() {
        let content = ".Bl -bullet -width indent-two -compact col1 col2 col3\nLine 1\nLine 2\n.El";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bl {
                list_type: BlType::Bullet,
                offset: Some(OffsetType::IndentTwo),
                compact: true,
                columns: vec!["col1".to_string(), "col2".to_string(), "col3".to_string()],
            },
            nodes: vec![
                Element::Text("Line 1\n".to_string()),
                Element::Text("Line 2\n".to_string()),
            ],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn bl_no_closing_macro() {
        let content = ".Bl -bullet\nLine 1\nLine 2\n";

        let mdoc = MdocParser::parse_mdoc(content);
        assert!(mdoc.is_err());
    }

    #[test]
    fn bl_foreign_closing_macros() {
        let closing_macros = vec![".Ed", ".Ef", ".Ek"];
        let content = ".Bl -bullet\nLine 1\nLine 2\n";

        for closing_macro in closing_macros {
            let content = format!("{content}.{closing_macro}");
            let mdoc = MdocParser::parse_mdoc(content);
            assert!(mdoc.is_err());
        }
    }

    #[test]
    fn bl_no_body() {
        let content = ".Bl -bullet\n.El\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bl {
                list_type: BlType::Bullet,
                offset: None,
                compact: false,
                columns: vec![],
            },
            nodes: vec![],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn bl_types() {
        let mut macro_types: HashMap<&str, BlType> = Default::default();
        macro_types.insert("-bullet", BlType::Bullet);
        macro_types.insert("-column", BlType::Column);
        macro_types.insert("-dash", BlType::Dash);
        macro_types.insert("-hyphen", BlType::Dash);
        macro_types.insert("-diag", BlType::Diag);
        macro_types.insert("-enum", BlType::Enum);
        macro_types.insert("-hang", BlType::Hang);
        macro_types.insert("-inset", BlType::Inset);
        macro_types.insert("-item", BlType::Item);
        macro_types.insert("-ohang", BlType::Ohang);
        macro_types.insert("-tag", BlType::Tag);

        for (str_type, enum_type) in macro_types {
            let content = format!(".Bl {str_type}\n.El\n");
            let element = Element::Macro(MacroNode {
                mdoc_macro: Macro::Bl {
                    list_type: enum_type,
                    offset: None,
                    compact: false,
                    columns: vec![],
                },
                nodes: vec![],
            });

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(
                *mdoc.elements.get(0).unwrap(),
                element,
                "Bl type: {str_type}"
            );
        }
    }

    #[test]
    fn bl_width() {
        let mut width_types: HashMap<&str, OffsetType> = Default::default();
        width_types.insert("indent", OffsetType::Indent);
        width_types.insert("indent-two", OffsetType::IndentTwo);
        width_types.insert("left", OffsetType::Left);
        width_types.insert("right", OffsetType::Right);
        width_types.insert("custom", OffsetType::Value("custom".to_string()));

        for (str_type, enum_type) in width_types {
            let content = format!(".Bl -bullet -width {str_type}\n.El\n");
            let element = Element::Macro(MacroNode {
                mdoc_macro: Macro::Bl {
                    list_type: BlType::Bullet,
                    offset: Some(enum_type),
                    compact: false,
                    columns: vec![],
                },
                nodes: vec![],
            });

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(
                *mdoc.elements.get(0).unwrap(),
                element,
                "Bl width: {str_type}"
            );
        }
    }

    #[test]
    fn bl_offset() {
        let mut offset_types: HashMap<&str, OffsetType> = Default::default();
        offset_types.insert("indent", OffsetType::Indent);
        offset_types.insert("indent-two", OffsetType::IndentTwo);
        offset_types.insert("left", OffsetType::Left);
        offset_types.insert("right", OffsetType::Right);
        offset_types.insert("custom", OffsetType::Value("custom".to_string()));

        for (str_type, enum_type) in offset_types {
            let content = format!(".Bl -bullet -offset {str_type}\n.El\n");
            let element = Element::Macro(MacroNode {
                mdoc_macro: Macro::Bl {
                    list_type: BlType::Bullet,
                    offset: Some(enum_type),
                    compact: false,
                    columns: vec![],
                },
                nodes: vec![],
            });

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(
                *mdoc.elements.get(0).unwrap(),
                element,
                "Bl offset: {str_type}"
            );
        }
    }

    #[test]
    fn bl_compact() {
        let content = format!(".Bl -bullet -compact\n.El\n");
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bl {
                list_type: BlType::Bullet,
                offset: None,
                compact: true,
                columns: vec![],
            },
            nodes: vec![],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn bl_columns() {
        let content = format!(".Bl -bullet col1 col2 col3\n.El\n");
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bl {
                list_type: BlType::Bullet,
                offset: None,
                compact: false,
                columns: vec!["col1".to_string(), "col2".to_string(), "col3".to_string()],
            },
            nodes: vec![],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn nd() {
        let content = ".Nd short description of the manual\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Nd {
                line: "short description of the manual".to_string(),
            },
            nodes: vec![],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn nd_with_line_whitespaces_and_tabs() {
        let content = ".Nd short description of the manual\t    \t\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Nd {
                line: "short description of the manual".to_string(),
            },
            nodes: vec![],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn nd_surrounded_by_text() {
        let content = "Line 1\n.Nd short description\nLine 2\n";
        let elements = vec![
            Element::Text("Line 1\n".to_string()),
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Nd {
                    line: "short description".to_string(),
                },
                nodes: vec![Element::Text("Line 2\n".to_string())],
            }),
            Element::Text("".to_string()),
        ];

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(mdoc.elements, elements);
    }

    #[test]
    fn nd_with_sh_closure() {
        let content = ".Nd short description\nLine 1\nLine 2\n.Sh SECTION\n";
        let elements = vec![
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Nd {
                    line: "short description".to_string(),
                },
                nodes: vec![
                    Element::Text("Line 1\n".to_string()),
                    Element::Text("Line 2\n".to_string()),
                ],
            }),
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Sh {
                    title: "SECTION".to_string(),
                },
                nodes: vec![],
            }),
            Element::Text("".to_string()),
        ];

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(mdoc.elements, elements);
    }

    #[test]
    fn nm() {
        let content = ".Nm command_name\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Nm {
                name: Some(vec!["command_name".to_string()]),
            },
            nodes: vec![],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn nm_multiple_names() {
        let content = ".Nm command few name parts\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Nm {
                name: Some(vec![
                    "command".to_string(),
                    "few".to_string(),
                    "name".to_string(),
                    "parts".to_string(),
                ]),
            },
            nodes: vec![],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn nm_with_line_whitespaces_and_tabs() {
        let content = ".Nm command few   name\t\tparts    \t\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Nm {
                name: Some(vec![
                    "command".to_string(),
                    "few".to_string(),
                    "name".to_string(),
                    "parts".to_string(),
                ]),
            },
            nodes: vec![],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn nm_no_name() {
        let content = ".Nm\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Nm { name: None },
            nodes: vec![],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn nm_enclosing() {
        let content_eof = ".Nm name 1\nLine 1\n.Nm name 2\nLine 2\n";
        let content_sh = ".Nm name 1\nLine 1\n.Sh SECTION\nLine 2\n";
        let content_ss = ".Nm name 1\nLine 1\n.Ss SUBSECTION\nLine 2\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Nm {
                name: Some(vec!["name".to_string(), "1".to_string()]),
            },
            nodes: vec![Element::Text("Line 1\n".to_string())],
        });

        let mdoc_eof = MdocParser::parse_mdoc(content_eof).unwrap();
        assert_eq!(*mdoc_eof.elements.get(0).unwrap(), element);
        let mdoc_sh = MdocParser::parse_mdoc(content_sh).unwrap();
        assert_eq!(*mdoc_sh.elements.get(0).unwrap(), element);
        let mdoc_ss = MdocParser::parse_mdoc(content_ss).unwrap();
        assert_eq!(*mdoc_ss.elements.get(0).unwrap(), element);
    }

    #[test]
    fn nm_remember_name_skip_before_defining() {
        let content = ".Nm\n.Nm name 1\n";
        let elements = vec![
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Nm { name: None },
                nodes: vec![],
            }),
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Nm {
                    name: Some(vec!["name".to_string(), "1".to_string()]),
                },
                nodes: vec![],
            }),
            Element::Text("".to_string()),
        ];

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(mdoc.elements, elements);
    }

    #[test]
    fn nm_remember_use_defined() {
        let content = ".Nm name 1\n.Nm\n";
        let elements = vec![
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Nm {
                    name: Some(vec!["name".to_string(), "1".to_string()]),
                },
                nodes: vec![],
            }),
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Nm {
                    name: Some(vec!["name".to_string(), "1".to_string()]),
                },
                nodes: vec![],
            }),
            Element::Text("".to_string()),
        ];

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(mdoc.elements, elements);
    }

    #[test]
    fn nm_remember_use_defined_with_local_overring() {
        let content = ".Nm name 1\n.Nm\n.Nm name 2\n";
        let elements = vec![
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Nm {
                    name: Some(vec!["name".to_string(), "1".to_string()]),
                },
                nodes: vec![],
            }),
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Nm {
                    name: Some(vec!["name".to_string(), "1".to_string()]),
                },
                nodes: vec![],
            }),
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Nm {
                    name: Some(vec!["name".to_string(), "2".to_string()]),
                },
                nodes: vec![],
            }),
            Element::Text("".to_string()),
        ];

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(mdoc.elements, elements);
    }

    #[test]
    fn sh() {
        let content = ".Sh SECTION\nThis is the SECTION section.\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Sh {
                title: "SECTION".to_string(),
            },
            nodes: vec![Element::Text("This is the SECTION section.\n".to_string())],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn sh_with_multiple_lines() {
        let content = ".Sh SECTION\nLine 1\nLine 2\nLine 3\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Sh {
                title: "SECTION".to_string(),
            },
            nodes: vec![
                Element::Text("Line 1\n".to_string()),
                Element::Text("Line 2\n".to_string()),
                Element::Text("Line 3\n".to_string()),
            ],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn sh_without_title() {
        let content = ".Sh\nLine 1\n";

        let mdoc = MdocParser::parse_mdoc(content);
        // TODO: Format and compare pest errors??
        assert!(mdoc.is_err());
    }

    #[test]
    fn sh_title_line() {
        let content = ".Sh TITLE LINE\nLine 1\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Sh {
                title: "TITLE LINE".to_string(),
            },
            nodes: vec![Element::Text("Line 1\n".to_string())],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn sh_with_multiple_chapters() {
        let content = ".Sh SECTION 1\nLine 1\n.Sh SECTION 2\nLine 2\n";
        let elements = vec![
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Sh {
                    title: "SECTION 1".to_string(),
                },
                nodes: vec![Element::Text("Line 1\n".to_string())],
            }),
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Sh {
                    title: "SECTION 2".to_string(),
                },
                nodes: vec![Element::Text("Line 2\n".to_string())],
            }),
            Element::Text("".to_string()),
        ];

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(mdoc.elements, elements);
    }

    #[test]
    fn sh_duplicating_section_names() {
        let content = ".Sh SECTION\nLine 1\n.Sh NEW_SECTION\nLine 2\n.Sh SECTION\nLine 3\n";

        let mdoc = MdocParser::parse_mdoc(content);
        assert_eq!(
            mdoc,
            Err(MdocError::ValidationError(
                "Duplicate .Sh title found: SECTION".to_string()
            ))
        );
    }

    #[test]
    fn sh_name_without_nd() {
        let content = ".Sh NAME\nLine 1\n";

        let mdoc = MdocParser::parse_mdoc(content);
        assert_eq!(
            mdoc,
            Err(MdocError::ValidationError(
                ".Sh NAME must end with .Nd".to_string()
            ))
        );
    }

    #[test]
    fn sh_name_with_nd() {
        let content = ".Sh NAME\nLine 1\n.Nd short description\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Sh {
                title: "NAME".to_string(),
            },
            nodes: vec![
                Element::Text("Line 1\n".to_string()),
                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Nd {
                        line: "short description".to_string(),
                    },
                    nodes: vec![],
                }),
            ],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn sh_name_with_nd_in_nm() {
        let content = ".Sh NAME\nLine 1\n.Nm utility\n.Nd short description\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Sh {
                title: "NAME".to_string(),
            },
            nodes: vec![
                Element::Text("Line 1\n".to_string()),
                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Nm {
                        name: Some(vec!["utility".to_string()]),
                    },
                    nodes: vec![Element::Macro(MacroNode {
                        mdoc_macro: Macro::Nd {
                            line: "short description".to_string(),
                        },
                        nodes: vec![],
                    })],
                }),
            ],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn ss() {
        let content = ".Ss Options\nThese are the available options.\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Ss {
                title: "Options".to_string(),
            },
            nodes: vec![Element::Text(
                "These are the available options.\n".to_string(),
            )],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn ss_with_multiple_lines() {
        let content = ".Ss Options\nLine 1\nLine 2\nLine 3\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Ss {
                title: "Options".to_string(),
            },
            nodes: vec![
                Element::Text("Line 1\n".to_string()),
                Element::Text("Line 2\n".to_string()),
                Element::Text("Line 3\n".to_string()),
            ],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn ss_without_title() {
        let content = ".Ss\nLine 1\n";

        let mdoc = MdocParser::parse_mdoc(content);
        // TODO: Format and compare pest errors??
        assert!(mdoc.is_err());
    }

    #[test]
    fn ss_title_line() {
        let content = ".Ss TITLE LINE\nLine 1\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Ss {
                title: "TITLE LINE".to_string(),
            },
            nodes: vec![Element::Text("Line 1\n".to_string())],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn ss_nested_in_sh() {
        let content = ".Sh SECTION\n.Ss Subsection\nLine 1\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Sh {
                title: "SECTION".to_string(),
            },
            nodes: vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ss {
                    title: "Subsection".to_string(),
                },
                nodes: vec![Element::Text("Line 1\n".to_string())],
            })],
        });

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn ss_with_multiple_subchapters() {
        let content = ".Ss Subchapter 1\nLine 1\n.Ss Subchapter 2\nLine 2\n";
        let elements = vec![
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Ss {
                    title: "Subchapter 1".to_string(),
                },
                nodes: vec![Element::Text("Line 1\n".to_string())],
            }),
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Ss {
                    title: "Subchapter 2".to_string(),
                },
                nodes: vec![Element::Text("Line 2\n".to_string())],
            }),
            Element::Text("".to_string()),
        ];

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(mdoc.elements, elements);
    }

    #[test]
    fn ss_with_duplicate_titles() {
        let content = ".Ss Subchapter 1\n.Ss Subchapter 2\nLine 2\n.Ss Subchapter 1\nLine 3\n";

        let mdoc = MdocParser::parse_mdoc(content);
        assert_eq!(
            mdoc,
            Err(MdocError::ValidationError(
                "Duplicate .Ss title found: Subchapter 1".to_string()
            ))
        );
    }
}
