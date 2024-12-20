use pest::{iterators::Pair, Parser};
use pest_derive::Parser;
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

#[derive(Debug)]
pub struct MdocDocument {
    elements: Vec<Element>,
}

impl MdocParser {
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

    /// Parses (`Bd`)[https://man.openbsd.org/mdoc#Bd]:
    /// `Bd -type [-offset width] [-compact]`
    fn parse_bd_block(pair: Pair<Rule>) -> Element {
        let mut pairs = pair.into_inner();

        let bd_open = pairs
            .next()
            .expect("Expected '.Bd -type [-offset width] [-compact]'");
        let bd_macro = Self::parse_bd_open(bd_open);

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

    /// Parses (`Bf`)[https://man.openbsd.org/mdoc#Bf]:
    /// `Bf -emphasis | -literal | -symbolic | Em | Li | Sy`
    fn parse_bf_block(pair: Pair<Rule>) -> Element {
        let mut pairs = pair.into_inner();

        let bf_open = pairs
            .next()
            .expect("Expected '.Bf -emphasis | -literal | -symbolic | Em | Li | Sy'");
        let bf_macro = Self::parse_bf_open(bf_open);

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

    // Parses (`Bl`)[https://man.openbsd.org/mdoc#Bl]
    // `Bl -type [-width val] [-offset val] [-compact] [col ...]`
    fn parse_bl_block(pair: Pair<Rule>) -> Element {
        let mut pairs = pair.into_inner();

        let bl_open = pairs
            .next()
            .expect("Expected '.Bl -type [-width val] [-offset val] [-compact] [col ...]'");
        let bl_macro = Self::parse_bl_open(bl_open);

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

    fn parse_element(pair: Pair<Rule>) -> Element {
        match pair.as_rule() {
            Rule::block_full_explicit => Self::parse_block_full_explicit(pair),
            _ => Element::Text(pair.as_str().to_string()),
        }
    }

    pub fn parse_mdoc(input: impl AsRef<str>) -> MdocDocument {
        match MdocParser::parse(Rule::document, input.as_ref()) {
            Ok(pairs) => {
                println!("Pairs:\n{pairs:#?}\n\n");
                println!("Parsing successful!");

                // Iterate each pair (macro or text element)
                let elements = pairs
                    .flat_map(|p| {
                        let inner_rules = p.into_inner();
                        inner_rules.map(|p| Self::parse_element(p))
                    })
                    .collect();
                MdocDocument { elements }
            }
            Err(e) => {
                eprintln!("Error: {e}");
                MdocDocument { elements: vec![] }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::man_util::parser::*;

    // Block full-explicit
    #[test]
    fn bd() {
        let content = format!(
            r".Bd -literal -offset indent -compact
Example line 1
Example line 2
.Ed
#"
        );

        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bd {
                block_type: BdType::Literal,
                offset: Some(OffsetType::Indent),
                compact: true,
            },
            nodes: vec![Element::Text(
                "Example line 1\nExample line 2\n".to_string(),
            )],
        });

        let mdoc = MdocParser::parse_mdoc(content);
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn bd_no_body() {
        let content = ".Bd -literal\n.Ed\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bd {
                block_type: BdType::Literal,
                offset: None,
                compact: false,
            },
            nodes: vec![Element::Text("".to_string())],
        });

        let mdoc = MdocParser::parse_mdoc(content);
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
        // TODO: handle invalid value??
        // bd_types.insert("invalid_value", ...);

        for (str_type, enum_type) in bd_types {
            let content = format!(".Bd {str_type}\n.Ed\n");
            let element = Element::Macro(MacroNode {
                mdoc_macro: Macro::Bd {
                    block_type: enum_type,
                    offset: None,
                    compact: false,
                },
                nodes: vec![Element::Text("".to_string())],
            });

            let mdoc = MdocParser::parse_mdoc(content);
            assert_eq!(*mdoc.elements.get(0).unwrap(), element);
        }
    }

    #[test]
    fn bd_offset() {
        let mut offset_types: HashMap<&str, OffsetType> = Default::default();
        offset_types.insert("indent", OffsetType::Indent);
        offset_types.insert("indent-two", OffsetType::IndentTwo);
        offset_types.insert("left", OffsetType::Left);
        offset_types.insert("right", OffsetType::Right);
        offset_types.insert("custom", OffsetType::Value("custom".to_string()));

        for (str_type, enum_type) in offset_types {
            let content = format!(".Bd -literal -offset {str_type}\n.Ed\n");
            let element = Element::Macro(MacroNode {
                mdoc_macro: Macro::Bd {
                    block_type: BdType::Literal,
                    offset: Some(enum_type),
                    compact: false,
                },
                nodes: vec![Element::Text("".to_string())],
            });

            let mdoc = MdocParser::parse_mdoc(content);
            assert_eq!(*mdoc.elements.get(0).unwrap(), element);
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
            nodes: vec![Element::Text("".to_string())],
        });

        let mdoc = MdocParser::parse_mdoc(content);
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn bf() {
        let content = r".Bf -emphasis
Example line 1
Example line 2
.Ef
#";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bf(BfType::Emphasis),
            nodes: vec![Element::Text(
                "Example line 1\nExample line 2\n".to_string(),
            )],
        });

        let mdoc = MdocParser::parse_mdoc(content);
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
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
        // TODO: handle invalid value??
        // bf_types.insert("invalid_value", ...);

        for (str_type, enum_type) in bf_types {
            let content = format!(".Bf {str_type}\n.Ef\n");
            let element = Element::Macro(MacroNode {
                mdoc_macro: Macro::Bf(enum_type),
                nodes: vec![Element::Text("".to_string())],
            });

            let mdoc = MdocParser::parse_mdoc(content);
            assert_eq!(*mdoc.elements.get(0).unwrap(), element);
        }
    }

    #[test]
    fn bk() {
        let content = r".Bk -words
Example line 1
Example line 2
.Ek
#";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bk,
            nodes: vec![Element::Text(
                "Example line 1\nExample line 2\n".to_string(),
            )],
        });

        let mdoc = MdocParser::parse_mdoc(content);
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    #[test]
    fn bk_no_body() {
        let content = ".Bk -words\n.Ek\n";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bk,
            nodes: vec![Element::Text("".to_string())],
        });

        let mdoc = MdocParser::parse_mdoc(content);
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }

    // TODO: fix recognition ".MACRO" as Text
    // #[test]
    // fn bk_no_words() {
    //     let content = ".Bk\n.Ek\n";
    //     let element = Element::Macro(MacroNode {
    //         mdoc_macro: Macro::Bk,
    //         nodes: vec![Element::Text(
    //             "Example line 1\nExample line 2\n".to_string(),
    //         )],
    //     });

    //     let mdoc = MdocParser::parse_mdoc(content);
    //     assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    // }

    #[test]
    fn bl() {
        let content = r".Bl -bullet -width indent-two -compact col1 col2 col3
Example line 1
Example line 2
.El
#";
        let element = Element::Macro(MacroNode {
            mdoc_macro: Macro::Bl {
                list_type: BlType::Bullet,
                offset: Some(OffsetType::IndentTwo),
                compact: true,
                columns: vec!["col1".to_string(), "col2".to_string(), "col3".to_string()],
            },
            nodes: vec![Element::Text(
                "Example line 1\nExample line 2\n".to_string(),
            )],
        });

        let mdoc = MdocParser::parse_mdoc(content);
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
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
            nodes: vec![Element::Text("".to_string())],
        });

        let mdoc = MdocParser::parse_mdoc(content);
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
                nodes: vec![Element::Text("".to_string())],
            });

            let mdoc = MdocParser::parse_mdoc(content);
            assert_eq!(*mdoc.elements.get(0).unwrap(), element);
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
                nodes: vec![Element::Text("".to_string())],
            });

            let mdoc = MdocParser::parse_mdoc(content);
            assert_eq!(*mdoc.elements.get(0).unwrap(), element);
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
                nodes: vec![Element::Text("".to_string())],
            });

            let mdoc = MdocParser::parse_mdoc(content);
            assert_eq!(*mdoc.elements.get(0).unwrap(), element);
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
            nodes: vec![Element::Text("".to_string())],
        });

        let mdoc = MdocParser::parse_mdoc(content);
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
            nodes: vec![Element::Text("".to_string())],
        });

        let mdoc = MdocParser::parse_mdoc(content);
        assert_eq!(*mdoc.elements.get(0).unwrap(), element);
    }
}
