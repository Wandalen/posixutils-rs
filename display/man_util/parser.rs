//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use pest::{iterators::Pair, Parser};
use pest_derive::Parser;
use std::collections::HashSet;
use std::ops::Index;
use text_production::{AtType, BsxType, BxType, DxType, FxType, NxType, OxType, StType};
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
    Eoi, // "End of input" marker
}

#[derive(Debug, PartialEq)]
pub struct MdocDocument {
    pub elements: Vec<Element>,
}

#[derive(Error, Debug, PartialEq)]
pub enum MdocError {
    #[error("mdoc: {0}")]
    Pest(#[from] Box<pest::error::Error<Rule>>),
    #[error("mdoc: {0}")]
    Parsing(String),
    #[error("mdoc: {0}")]
    Validation(String),
    #[error("mdoc: {0}")]
    Formatting(String),
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
                        // Recursively check the last child node
                        is_last_element_nd(last_node)
                    } else {
                        // If the node is empty, check the macro itself
                        matches!(mdoc_macro, Macro::Nd { .. })
                    }
                }
                _ => false,
            }
        }

        if let Macro::Sh { title } = &sh_node.mdoc_macro {
            if !self.sh_titles.insert(title.clone()) {
                return Err(MdocError::Validation(format!(
                    "Duplicate .Sh title found: {title}"
                )));
            }
            if title == "NAME" && !sh_node.nodes.is_empty() {
                let last_element = sh_node.nodes.last().unwrap();
                if !is_last_element_nd(last_element) {
                    return Err(MdocError::Validation(
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
                return Err(MdocError::Validation(format!(
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
    fn parse_element(pair: Pair<Rule>) -> Element {
        match pair.as_rule() {
            Rule::element => Self::parse_element(pair.into_inner().next().unwrap()),
            Rule::block_full_explicit => Self::parse_block_full_explicit(pair),
            Rule::block_full_implicit => Self::parse_block_full_implicit(pair),
            Rule::block_partial_implicit => Self::parse_block_partial_implicit(pair),
            Rule::inline => Self::parse_inline(pair),
            Rule::arg => Self::parse_arg(pair.into_inner().next().unwrap()),
            Rule::macro_arg => Self::parse_element(pair.into_inner().next().unwrap()),
            Rule::EOI => Element::Eoi,
            _ => Element::Text(pair.as_str().to_string()),
        }
    }

    fn parse_arg(pair: Pair<Rule>) -> Element {
        match pair.as_rule() {
            Rule::text_arg => Element::Text(pair.as_str().to_string()),
            Rule::macro_arg => Self::parse_element(pair.into_inner().next().unwrap()),
            _ => unreachable!(),
        }
    }

    pub fn parse_mdoc(input: impl AsRef<str>) -> Result<MdocDocument, MdocError> {
        let pairs = MdocParser::parse(Rule::mdoc, input.as_ref())
            .map_err(|err| MdocError::Pest(Box::new(err)))?;
        println!("Pairs:\n{pairs:#?}\n\n");

        // Iterate each pair (macro or text element)
        let mut elements: Vec<Element> = pairs
            .flat_map(|p| {
                let inner_rules = p.into_inner();
                inner_rules.map(Self::parse_element)
            })
            .collect();
        elements.pop(); // Remove `Element::Eoi` element

        // TODO: debug only
        // elements.iter().for_each(|e| println!("{e:?}"));

        let mut mdoc = MdocDocument { elements };

        let validator = &mut MdocValidator::default();
        validator.validate(&mut mdoc)?;

        Ok(mdoc)
    }
}

// Block full-explicit macros parsing
impl MdocParser {
    /// Parses (`Bd`)[https://man.openbsd.org/mdoc#Bd]:
    /// `Bd -type [-offset width] [-compact]`
    fn parse_bd_block(pair: Pair<Rule>) -> Element {
        fn parse_bd_open(pair: Pair<Rule>) -> Macro {
            let mut inner = pair.into_inner();

            // -type
            let block_type = BdType::from(inner.next().unwrap());

            let mut offset: Option<OffsetType> = None;
            let mut compact = false;

            for opt_pair in inner {
                match opt_pair.as_rule() {
                    Rule::offset => offset = Some(OffsetType::from(opt_pair)),
                    Rule::compact => compact = true,
                    _ => unreachable!(),
                }
            }

            Macro::Bd {
                block_type,
                offset,
                compact,
            }
        }

        let mut pairs = pair.into_inner();

        let bd_macro = parse_bd_open(pairs.next().unwrap());

        let nodes = pairs
            .take_while(|p| p.as_rule() != Rule::ed_close)
            .map(Self::parse_element)
            .collect();

        Element::Macro(MacroNode {
            mdoc_macro: bd_macro,
            nodes,
        })
    }

    /// Parses (`Bf`)[https://man.openbsd.org/mdoc#Bf]:
    /// `Bf -emphasis | -literal | -symbolic | Em | Li | Sy`
    fn parse_bf_block(pair: Pair<Rule>) -> Element {
        fn parse_bf_open(pair: Pair<Rule>) -> Macro {
            let mut inner = pair.into_inner();

            // -type
            let block_type = BfType::from(inner.next().unwrap());

            Macro::Bf(block_type)
        }

        let mut pairs = pair.into_inner();

        let bf_macro = parse_bf_open(pairs.next().unwrap());

        let nodes = pairs
            .take_while(|p| p.as_rule() != Rule::ef_close)
            .map(Self::parse_element)
            .collect();

        Element::Macro(MacroNode {
            mdoc_macro: bf_macro,
            nodes,
        })
    }

    /// Parses (`Bk`)[https://man.openbsd.org/mdoc#Bk]:
    /// `Bk -words`
    fn parse_bk_block(pair: Pair<Rule>) -> Element {
        let mut pairs = pair.into_inner();

        // `bk_open`
        let _ = pairs.next().unwrap();

        let nodes = pairs
            .take_while(|p| p.as_rule() != Rule::ek_close)
            .map(Self::parse_element)
            .collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Bk,
            nodes,
        })
    }

    // Parses (`Bl`)[https://man.openbsd.org/mdoc#Bl]
    // `Bl -type [-width val] [-offset val] [-compact] [col ...]`
    fn parse_bl_block(pair: Pair<Rule>) -> Element {
        fn parse_bl_open(pair: Pair<Rule>) -> Macro {
            let mut inner = pair.into_inner();

            // -type
            let bl_type_pair = inner.next().unwrap();
            let list_type = BlType::from(bl_type_pair);

            let mut offset: Option<OffsetType> = None;
            let mut compact = false;
            let mut columns = vec![];

            for opt_pair in inner {
                match opt_pair.as_rule() {
                    Rule::offset => offset = Some(OffsetType::from(opt_pair)),
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

        let bl_macro = parse_bl_open(pairs.next().unwrap());

        let nodes = pairs
            .take_while(|p| p.as_rule() != Rule::el_close)
            .map(Self::parse_element)
            .collect();

        Element::Macro(MacroNode {
            mdoc_macro: bl_macro,
            nodes,
        })
    }

    fn parse_block_full_explicit(pair: Pair<Rule>) -> Element {
        let pair = pair.into_inner().next().unwrap();
        match pair.as_rule() {
            Rule::bd_block => Self::parse_bd_block(pair),
            Rule::bf_block => Self::parse_bf_block(pair),
            Rule::bk_block => Self::parse_bk_block(pair),
            Rule::bl_block => Self::parse_bl_block(pair),
            _ => unreachable!(),
        }
    }
}

// Block full-implicit macros parsing
impl MdocParser {
    // Parses (`Nd`)[https://man.openbsd.org/mdoc#Nd]
    // `Nd line`
    fn parse_nd(pair: Pair<Rule>) -> Element {
        let mut inner = pair.into_inner();

        let line = inner
            .next() // `nd_block` -> `nd_open`
            .unwrap()
            .into_inner()
            .next() // `nd_open` -> `nd_line`
            .expect("Expected title for 'Nd' block")
            .as_str()
            .trim_end()
            .to_string();

        // Parse `nd_block_element`
        let nodes = inner
            .filter_map(|p| p.into_inner().next().map(Self::parse_element))
            .collect();

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

        // `nm_block` -> `nm_open`
        let nm_pairs = inner
            .next()
            .unwrap()
            .into_inner()
            .flat_map(|p| p.into_inner());

        let is_name = |item: &Pair<Rule>| matches!(item.as_rule(), Rule::text_arg);

        // While `nm_open` contains `text_arg` consider it as name
        let name: Vec<String> = nm_pairs
            .clone()
            .take_while(is_name)
            .map(|p| p.as_str().to_string())
            .collect();
        // Other arguments are not names
        let mut nodes: Vec<Element> = nm_pairs.skip_while(is_name).map(Self::parse_arg).collect();

        let name = if name.is_empty() { None } else { Some(name) };

        // Parse `nm_block_element`
        nodes.extend(inner.filter_map(|p| p.into_inner().next().map(Self::parse_element)));

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

        // Parse `sh_block_element`
        let nodes = inner
            .filter_map(|p| p.into_inner().next().map(Self::parse_element))
            .collect();

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

        // Parse `ss_block_element`
        let nodes = inner
            .filter_map(|p| p.into_inner().next().map(Self::parse_element))
            .collect();

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
            _ => unreachable!(),
        }
    }
}

// Block partial-implicit macros parsing
impl MdocParser {
    // Parses (`Aq`)[https://man.openbsd.org/mdoc#Aq]:
    // `Aq line`
    fn parse_aq_block(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Aq,
            nodes,
        })
    }

    // Parses (`Bq`)[https://man.openbsd.org/mdoc#Bq]:
    // `Bq line`
    fn parse_bq_block(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Bq,
            nodes,
        })
    }

    // Parses (`Brq`)[https://man.openbsd.org/mdoc#Brq]:
    // `Brq line`
    fn parse_brq_block(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Brq,
            nodes,
        })
    }

    // Parses (`D1`)[https://man.openbsd.org/mdoc#D1]:
    // `D1 line`
    fn parse_d1_block(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::D1,
            nodes,
        })
    }

    // Parses (`Dl`)[https://man.openbsd.org/mdoc#Dl]:
    // `Dl line`
    fn parse_dl_block(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Dl,
            nodes,
        })
    }

    // Parses (`Dq`)[https://man.openbsd.org/mdoc#Dq]:
    // `Dq line`
    fn parse_dq_block(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Dq,
            nodes,
        })
    }

    // Parses (`En`)[https://man.openbsd.org/mdoc#En]:
    // `En word ...`
    fn parse_en_block(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::En,
            nodes,
        })
    }

    // Parses (`Op`)[https://man.openbsd.org/mdoc#Op]:
    // `Op line`
    fn parse_op_block(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Op,
            nodes,
        })
    }

    // Parses (`Pq`)[https://man.openbsd.org/mdoc#Pq]:
    // `Pq line`
    fn parse_pq_block(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Pq,
            nodes,
        })
    }

    // Parses (`Ql`)[https://man.openbsd.org/mdoc#Ql]:
    // `Ql line`
    fn parse_ql_block(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Ql,
            nodes,
        })
    }

    // Parses (`Qq`)[https://man.openbsd.org/mdoc#Qq]:
    // `Qq line`
    fn parse_qq_block(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Qq,
            nodes,
        })
    }

    // Parses (`Sq`)[https://man.openbsd.org/mdoc#Sq]:
    // `Sq line`
    fn parse_sq_block(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Sq,
            nodes,
        })
    }

    // Parses (`Vt`)[https://man.openbsd.org/mdoc#Vt]:
    // `Vt type [identifier] ...`
    fn parse_vt_block(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Vt,
            nodes,
        })
    }

    fn parse_block_partial_implicit(pair: Pair<Rule>) -> Element {
        let pair = pair.into_inner().next().unwrap();
        match pair.as_rule() {
            Rule::aq_block => Self::parse_aq_block(pair),
            Rule::bq_block => Self::parse_bq_block(pair),
            Rule::brq_block => Self::parse_brq_block(pair),
            Rule::d1_block => Self::parse_d1_block(pair),
            Rule::dl_block => Self::parse_dl_block(pair),
            Rule::dq_block => Self::parse_dq_block(pair),
            Rule::en_block => Self::parse_en_block(pair),
            Rule::op_block => Self::parse_op_block(pair),
            Rule::pq_block => Self::parse_pq_block(pair),
            Rule::ql_block => Self::parse_ql_block(pair),
            Rule::qq_block => Self::parse_qq_block(pair),
            Rule::sq_block => Self::parse_sq_block(pair),
            Rule::vt_block => Self::parse_vt_block(pair),
            _ => unreachable!(),
        }
    }
}

// In-line macros parsing
impl MdocParser {
    fn parse_rs_submacro(pair: Pair<Rule>) -> Element {
        // Parses (`%A`)[https://man.openbsd.org/mdoc#_A]:
        // `%A first_name ... last_name`
        fn parse_a(pair: Pair<Rule>) -> Element {
            let author_names = pair
                .into_inner()
                .map(|p| p.as_str().to_string())
                .collect::<Vec<_>>()
                .join(" ");
            Element::Macro(MacroNode {
                mdoc_macro: Macro::A {
                    author_name: author_names,
                },
                nodes: vec![],
            })
        }

        // Parses (`%B`)[https://man.openbsd.org/mdoc#_B]:
        // `%B title`
        fn parse_b(pair: Pair<Rule>) -> Element {
            let book_title = pair
                .into_inner()
                .map(|p| p.as_str().to_string())
                .collect::<Vec<_>>()
                .join(" ");
            Element::Macro(MacroNode {
                mdoc_macro: Macro::B { book_title },
                nodes: vec![],
            })
        }

        // Parses (`%C`)[https://man.openbsd.org/mdoc#_C]:
        // `%C location`
        fn parse_c(pair: Pair<'_, Rule>) -> Element {
            let publication_location = pair
                .into_inner()
                .map(|p| p.as_str().to_string())
                .collect::<Vec<_>>()
                .join(" ");
            Element::Macro(MacroNode {
                mdoc_macro: Macro::C {
                    publication_location,
                },
                nodes: vec![],
            })
        }

        // Parses (`%D`)[https://man.openbsd.org/mdoc#_D]:
        // `%D [month day,] year`
        fn parse_d(pair: Pair<'_, Rule>) -> Element {
            let mut inner = pair.into_inner();

            let mut month_day = None;

            let inner_pair = inner.next().unwrap();
            let year = match inner_pair.as_rule() {
                Rule::month_day => {
                    let mut md = inner_pair.into_inner();

                    let month = md.next().unwrap().as_str().to_string();
                    let day = md.next().unwrap().as_str().parse().unwrap();

                    month_day = Some((month, day));

                    inner.next().unwrap().as_str().parse::<i32>().unwrap()
                }
                Rule::year => inner_pair.as_str().parse::<i32>().unwrap(),
                _ => unreachable!(),
            };

            Element::Macro(MacroNode {
                mdoc_macro: Macro::D { month_day, year },
                nodes: vec![],
            })
        }

        // Parses (`%I`)[https://man.openbsd.org/mdoc#_I]:
        // `%I name`
        fn parse_i(pair: Pair<'_, Rule>) -> Element {
            let issuer_name = pair
                .into_inner()
                .map(|p| p.as_str().to_string())
                .collect::<Vec<String>>()
                .join(" ");
            Element::Macro(MacroNode {
                mdoc_macro: Macro::I { issuer_name },
                nodes: vec![],
            })
        }

        // Parses (`%J`)[https://man.openbsd.org/mdoc#_J]:
        // `%J name`
        fn parse_j(pair: Pair<'_, Rule>) -> Element {
            let journal_name = pair
                .into_inner()
                .map(|p| p.as_str().to_string())
                .collect::<Vec<String>>()
                .join(" ");
            Element::Macro(MacroNode {
                mdoc_macro: Macro::J { journal_name },
                nodes: vec![],
            })
        }

        // Parses (`%N`)[https://man.openbsd.org/mdoc#_N]:
        // `%N number`
        fn parse_n(pair: Pair<'_, Rule>) -> Element {
            let issue_number = pair
                .into_inner()
                .map(|p| p.as_str().to_string())
                .collect::<Vec<String>>()
                .join(" ");
            Element::Macro(MacroNode {
                mdoc_macro: Macro::N { issue_number },
                nodes: vec![],
            })
        }

        // Parses (`%O`)[https://man.openbsd.org/mdoc#_O]:
        // `%O line`
        fn parse_o(pair: Pair<'_, Rule>) -> Element {
            let information = pair
                .into_inner()
                .map(|p| p.as_str().to_string())
                .collect::<Vec<String>>()
                .join(" ");
            Element::Macro(MacroNode {
                mdoc_macro: Macro::O { information },
                nodes: vec![],
            })
        }

        // Parses (`%P`)[https://man.openbsd.org/mdoc#_P]:
        // `%P number`
        fn parse_p(pair: Pair<'_, Rule>) -> Element {
            let page_number = pair
                .into_inner()
                .map(|p| p.as_str().to_string())
                .collect::<Vec<String>>()
                .join(" ");
            Element::Macro(MacroNode {
                mdoc_macro: Macro::P { page_number },
                nodes: vec![],
            })
        }

        // Parses (`%Q`)[https://man.openbsd.org/mdoc#_Q]:
        // `%Q name`
        fn parse_q(pair: Pair<'_, Rule>) -> Element {
            let insitution_author = pair
                .into_inner()
                .map(|p| p.as_str().to_string())
                .collect::<Vec<String>>()
                .join(" ");
            Element::Macro(MacroNode {
                mdoc_macro: Macro::Q { insitution_author },
                nodes: vec![],
            })
        }

        // Parses (`%R`)[https://man.openbsd.org/mdoc#_R]:
        // `%R name`
        fn parse_r(pair: Pair<'_, Rule>) -> Element {
            let report_name = pair
                .into_inner()
                .map(|p| p.as_str().to_string())
                .collect::<Vec<String>>()
                .join(" ");
            Element::Macro(MacroNode {
                mdoc_macro: Macro::R { report_name },
                nodes: vec![],
            })
        }

        // Parses (`%T`)[https://man.openbsd.org/mdoc#_T]:
        // `%T title`
        fn parse_t(pair: Pair<'_, Rule>) -> Element {
            let article_title = pair
                .into_inner()
                .map(|p| p.as_str().to_string())
                .collect::<Vec<String>>()
                .join(" ");
            Element::Macro(MacroNode {
                mdoc_macro: Macro::T { article_title },
                nodes: vec![],
            })
        }

        // Parses (`%U`)[https://man.openbsd.org/mdoc#_U]:
        // `%U protocol://path`
        fn parse_u(pair: Pair<'_, Rule>) -> Element {
            let uri = pair.into_inner().next().unwrap().as_str().to_string();
            Element::Macro(MacroNode {
                mdoc_macro: Macro::U { uri },
                nodes: vec![],
            })
        }

        // Parses (`%V`)[https://man.openbsd.org/mdoc#_V]:
        // `%V number`
        fn parse_v(pair: Pair<'_, Rule>) -> Element {
            let volume_number = pair
                .into_inner()
                .map(|p| p.as_str().to_string())
                .collect::<Vec<String>>()
                .join(" ");
            Element::Macro(MacroNode {
                mdoc_macro: Macro::V { volume_number },
                nodes: vec![],
            })
        }

        let pair = pair.into_inner().next().unwrap();
        match pair.as_rule() {
            Rule::a => parse_a(pair),
            Rule::b => parse_b(pair),
            Rule::c => parse_c(pair),
            Rule::d => parse_d(pair),
            Rule::i => parse_i(pair),
            Rule::j => parse_j(pair),
            Rule::n => parse_n(pair),
            Rule::o => parse_o(pair),
            Rule::p => parse_p(pair),
            Rule::q => parse_q(pair),
            Rule::r => parse_r(pair),
            Rule::t => parse_t(pair),
            Rule::u => parse_u(pair),
            Rule::v => parse_v(pair),
            _ => unreachable!(),
        }
    }

    fn parse_text_production(pair: Pair<Rule>) -> Element {
        fn parse_x_args(pair: Pair<Rule>) -> (Vec<String>, Vec<Element>) {
            let args = pair.into_inner().flat_map(|p| p.into_inner());

            let is_version = |item: &Pair<Rule>| matches!(item.as_rule(), Rule::text_arg);

            // While macro contains `text_arg` consider it as version
            let version: Vec<String> = args
                .clone()
                .take_while(is_version)
                .map(|p| p.as_str().to_string())
                .collect();
            // Other arguments are not version
            let nodes: Vec<Element> = args
                .skip_while(is_version)
                .map(MdocParser::parse_arg)
                .collect();

            (version, nodes)
        }

        // Parses (`At`)[https://man.openbsd.org/mdoc#At]:
        // `At [version]`
        fn parse_at(pair: Pair<Rule>) -> Element {
            let mut inner = pair.into_inner();

            let mut at_type = AtType::General;
            let mut nodes = vec![];

            if let Some(first_arg) = inner.next() {
                match first_arg.as_rule() {
                    Rule::at_type => {
                        at_type = AtType::from(first_arg);
                    }
                    Rule::arg => {
                        nodes.push(MdocParser::parse_element(first_arg));
                    }
                    _ => unreachable!(),
                }
            }
            nodes.extend(inner.map(MdocParser::parse_element));

            Element::Macro(MacroNode {
                mdoc_macro: Macro::At(at_type),
                nodes,
            })
        }

        // Parses (`Bsx`)[https://man.openbsd.org/mdoc#Bsx]:
        // `Bsx [version]`
        fn parse_bsx(pair: Pair<Rule>) -> Element {
            let (version, nodes) = parse_x_args(pair);

            Element::Macro(MacroNode {
                mdoc_macro: Macro::Bsx(BsxType { version }),
                nodes,
            })
        }

        // Parses (`Bx`)[https://man.openbsd.org/mdoc#Bx]:
        // `Bx [version [variant]]`
        fn parse_bx(pair: Pair<Rule>) -> Element {
            let (mut variant, nodes) = parse_x_args(pair);

            let version = if variant.is_empty() {
                None
            } else {
                Some(variant.remove(0))
            };

            Element::Macro(MacroNode {
                mdoc_macro: Macro::Bx(BxType { version, variant }),
                nodes,
            })
        }

        // Parses (`Dx`)[https://man.openbsd.org/mdoc#Dx]:
        // `Dx [version]`
        fn parse_dx(pair: Pair<Rule>) -> Element {
            let (version, nodes) = parse_x_args(pair);

            Element::Macro(MacroNode {
                mdoc_macro: Macro::Dx(DxType { version }),
                nodes,
            })
        }

        // Parses (`Fx`)[https://man.openbsd.org/mdoc#Fx]:
        // `Fx [version]`
        fn parse_fx(pair: Pair<Rule>) -> Element {
            let (version, nodes) = parse_x_args(pair);

            Element::Macro(MacroNode {
                mdoc_macro: Macro::Fx(FxType { version }),
                nodes,
            })
        }

        // Parses (`Nx`)[http://man.openbsd.org/mdoc#Nx]:
        // `Nx [version]`
        fn parse_nx(pair: Pair<Rule>) -> Element {
            let (version, nodes) = parse_x_args(pair);

            Element::Macro(MacroNode {
                mdoc_macro: Macro::Nx(NxType { version }),
                nodes,
            })
        }

        // Parses (`Ox`)[https://man.openbsd.org/mdoc#Ox]:
        // `Ox [version]`
        fn parse_ox(pair: Pair<Rule>) -> Element {
            let (version, nodes) = parse_x_args(pair);

            Element::Macro(MacroNode {
                mdoc_macro: Macro::Ox(OxType { version }),
                nodes,
            })
        }

        // Parses (`St`)[https://man.openbsd.org/mdoc#St]:
        // `St -abbreviation`
        fn parse_st(pair: Pair<Rule>) -> Element {
            let mut inner = pair.into_inner();

            let st_type = StType::from(inner.next().unwrap());

            let nodes = inner.map(MdocParser::parse_element).collect();

            Element::Macro(MacroNode {
                mdoc_macro: Macro::St(st_type),
                nodes,
            })
        }

        let pair = pair.into_inner().next().unwrap();
        match pair.as_rule() {
            Rule::at => parse_at(pair),
            Rule::bsx => parse_bsx(pair),
            Rule::bx => parse_bx(pair),
            Rule::dx => parse_dx(pair),
            Rule::fx => parse_fx(pair),
            Rule::nx => parse_nx(pair),
            Rule::ox => parse_ox(pair),
            Rule::st => parse_st(pair),
            _ => unreachable!(),
        }
    }

    // Parses (`Ad`)[https://man.openbsd.org/mdoc#Ad]:
    // `Ad address`
    fn parse_ad(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Ad,
            nodes,
        })
    }

    // Parses (`An`)[https://man.openbsd.org/mdoc#An]:
    // `An -split | -nosplit | first_name ... last_name`
    fn parse_an(pair: Pair<Rule>) -> Element {
        let an_arg = pair.into_inner().next().unwrap();
        let (author_name_type, nodes) = match an_arg.as_rule() {
            Rule::an_split => (AnType::Split, vec![]),
            Rule::an_no_split => (AnType::NoSplit, vec![]),
            Rule::an_name => (
                AnType::Name,
                an_arg.into_inner().map(Self::parse_element).collect(),
            ),
            _ => unreachable!(),
        };

        Element::Macro(MacroNode {
            mdoc_macro: Macro::An { author_name_type },
            nodes,
        })
    }

    // Parses (`Ap`)[https://man.openbsd.org/mdoc#Ap]:
    // `Ap`
    fn parse_ap(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Ap,
            nodes,
        })
    }

    // Parses (`Ar`)[https://man.openbsd.org/mdoc#Ar]:
    // `Ar [placeholder ...]`
    fn parse_ar(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Ar,
            nodes,
        })
    }

    // Parses (`Bt`)[https://man.openbsd.org/mdoc#Bt]:
    // `Bt`
    fn parse_bt(_pair: Pair<Rule>) -> Element {
        Element::Macro(MacroNode {
            mdoc_macro: Macro::Bt,
            nodes: vec![],
        })
    }

    // Parses (`Cd`)[https://man.openbsd.org/mdoc#Cd]:
    // `Cd line`
    fn parse_cd(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Cd,
            nodes,
        })
    }

    // Parses (`Cd`)[https://man.openbsd.org/mdoc#Cm]:
    // `Cm keyword ...`
    fn parse_cm(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Cm,
            nodes,
        })
    }

    // Parses (`Db`)[https://man.openbsd.org/mdoc#Db]
    // Obsolete
    fn parse_db(_pair: Pair<Rule>) -> Element {
        Element::Macro(MacroNode {
            mdoc_macro: Macro::Db,
            nodes: vec![]
        })
    }

    // Parses (`Dd`)[https://man.openbsd.org/mdoc#Dd]
    // `Dd [date]`
    fn parse_dd(pair: Pair<Rule>) -> Element {
        use chrono;
        use chrono::Datelike;

        fn parse_date(date: chrono::NaiveDate) -> DdDate {
            let month = match date.month() {
                1  => "January",
                2  => "February",
                3  => "March",
                4  => "April",
                5  => "May",
                6  => "June",
                7  => "July",
                8  => "August",
                9  => "September",
                10 => "October",
                11 => "November",
                12 => "December",
                _  => unreachable!() 
            };

            DdDate::MDYFormat(Date {
                month_day: (month.to_string(), date.day() as u8),
                year: date.year() as u16
            })
        }

        fn parse_block(pair: Pair<Rule>) -> DdDate {
            match pair.as_rule() {
                Rule::mdocdate => {
                    let mut mdy = pair.clone().into_inner();
    
                    let mut md = match mdy.next() {
                        Some(md) => md.into_inner(),
                        None => return DdDate::StrFormat(pair.as_str().to_string())
                    };
    
                    let month = match md.next() {
                        Some(month) => month.as_str().to_string(),
                        None => return DdDate::StrFormat(pair.as_str().to_string())
                    };
                    let day = match md.next() {
                        Some(day) => match day.as_str().parse::<u8>() {
                            Ok(day) => day,
                            Err(_) => return DdDate::StrFormat(pair.as_str().to_string())
                        },
                        None => return DdDate::StrFormat(pair.as_str().to_string())
                    };
    
                    let year = match mdy.next() {
                        Some(year) => match year.as_str().parse::<u16>() {
                            Ok(year) => year,
                            Err(_) => return DdDate::StrFormat(pair.as_str().to_string())
                        },
                        None => return DdDate::StrFormat(pair.as_str().to_string())
                    };
    
                    DdDate::MDYFormat(Date {
                        month_day: (month, day),
                        year: year
                    })
                },
                Rule::traditional_date => {
                    let date_str = pair.as_str();
                    let date = match chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                        Ok(date) => date,
                        Err(_) => return DdDate::StrFormat(date_str.to_string())
                    };
                    parse_date(date)
                },
                Rule::wrong_date => {
                    DdDate::StrFormat(pair.as_str().to_string())                    
                },
                _ => {
                    unreachable!()
                },
            }
        };

        match pair.into_inner().next() {
            Some(inner_pair) => {
                let date = parse_block(inner_pair.into_inner().next().unwrap());

                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Dd { 
                        date: date
                    },
                    nodes: vec![]
                })
            },
            None => {
                let date = parse_date(chrono::offset::Utc::now().date_naive());

                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Dd { 
                        date: date
                    },
                    nodes: vec![]
                })
            }
        }
    }

    // Parses (`Dt`)[https://man.openbsd.org/mdoc#Dt]
    fn parse_dt(pair: Pair<Rule>) -> Element {
        let mut inner = pair.into_inner();

        let title = inner.next().unwrap().as_str().trim().to_string().to_uppercase();
        let section = match inner.next().unwrap().as_str().trim() {
            "1"  => "General Commands",
            "2"  => "System Calls",
            "3"  => "Library Functions",
            "3p" => "Perl Library",
            "4"  => "Device Drivers",
            "5"  => "File Formats",
            "6"  => "Games",
            "7"  => "Miscellaneous Information",
            "8"  => "System Manager's Manual",
            "9"  => "Kernel Developer's Manual",
            _    => unreachable!()
        };
        let arch = match inner.next() {
            Some(arch) => Some(arch.as_str().trim().to_string()),
            None => None
        };

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Dt { 
                title: title, 
                section: section.to_string(), 
                arch: arch 
            },
            nodes: vec![]
        })
    }

    // Parses (`Dv`)[https://man.openbsd.org/mdoc#Dv]
    fn parse_dv(pair: Pair<Rule>) -> Element {
        let inner = pair.into_inner();
        
        let args = inner.flat_map(|p| p.into_inner());
        let is_constant = |item: &Pair<Rule>| matches!(item.as_rule(), Rule::text_arg);

        let identifiers: Vec<String> = args
            .clone()
            .take_while(is_constant)
            .map(|p| p.as_str().to_string())
            .collect();

        let nodes: Vec<Element> = args.skip_while(is_constant).map(Self::parse_arg).collect();
        Element::Macro(MacroNode { 
            mdoc_macro: Macro::Dv { 
                identifiers:  identifiers
            }, 
            nodes
        })
    }

    // Parses (`Em`)[https://man.openbsd.org/mdoc#Em]
    // .Em word ...
    fn parse_em(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Em,
            nodes
        })
    }

    // Parses (`Er`)[https://man.openbsd.org/mdoc#Er]
    // .Er CONSTANT ...
    fn parse_er(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Er,
            nodes
        })
    }

    // Parses (`Es`)[https://man.openbsd.org/mdoc#Es]
    // .Es opening_delimiter closing_delimiter
    fn parse_es(pair: Pair<Rule>) -> Element {
        let mut inner = pair.into_inner();

        let mut nodes = Vec::new();

        let arg1 = inner.next().unwrap();
        let arg2 = inner.next().unwrap();
        let mut opening_delimiter = None;
        let mut closing_delimiter = None;

        if matches!(arg1.as_rule(), Rule::macro_arg) {
            nodes.push(Self::parse_arg(arg1));
        } else {
            opening_delimiter = Some(arg1.as_str().parse::<char>().unwrap());
        }

        if matches!(arg2.as_rule(), Rule::macro_arg) {
            nodes.push(Self::parse_arg(arg2));
        } else {
            closing_delimiter = Some(arg2.as_str().parse::<char>().unwrap());
        }

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Es {
                opening_delimiter: opening_delimiter,
                closing_delimiter: closing_delimiter
            }, 
            nodes
        })
    }

    // Parses (`Ev`)[https://man.openbsd.org/mdoc#Ev]
    // .Ev VAR, ...
    fn parse_ev(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Ev,
            nodes
        })
    }

    // Parses (`Ex`)[https://man.openbsd.org/mdoc#Ex]
    // .Ex VAR, ...
    fn parse_ex(pair: Pair<Rule>) -> Element {
        let utilities = pair.into_inner().map(|p| p.as_str().to_string()).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Ex {
                utilities: utilities
            },
            nodes: vec![]
        })
    }

    // Parses (`Fa`)[https://man.openbsd.org/mdoc#Fa]
    // .Fa [args]
    fn parse_fa(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Fa,
            nodes
        })
    }

    // Parses (`Fd`)[https://man.openbsd.org/mdoc#Fd]
    // .Fd directive [args]
    fn parse_fd(pair: Pair<Rule>) -> Element {
        let mut inner = pair.into_inner();

        let directive = inner.next().unwrap().as_str().to_string();

        let mut args = vec![];

        while let Some(arg) = inner.next() {
            args.push(arg.as_str().to_string());
        };

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Fd {
                directive: directive,
                arguments: args
            },
            nodes: vec![]
        })
    }

    // Parses (`Fl`)[https://man.openbsd.org/mdoc#Fl]
    fn parse_fl(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Fl,
            nodes
        })
    }

    // Parses (`Fn`)[https://man.openbsd.org/mdoc#Fn]
    fn parse_fn(pair: Pair<Rule>) -> Element {
        let mut inner = pair.into_inner();
        
        let mut nodes = Vec::new();

        let funcname = if let Some(arg) = inner.next() {
            if matches!(arg.as_rule(), Rule::fn_funcname) {
                Some(arg.as_str().to_string())
            } else {
                nodes.push(Self::parse_element(arg));
                None
            }
        } else {
            unreachable!();
        };

        nodes.extend(inner.map(Self::parse_element));

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Fn {
                funcname: funcname
            },
            nodes
        })
    }

    // Parses (`Fr`)[https://man.openbsd.org/mdoc#Fr]
    // Obsolete
    // .Fr num
    fn parse_fr(pair: Pair<Rule>) -> Element {
        let mut inner = pair.into_inner();

        let mut num = None;
        let mut nodes = Vec::new();
        
        let arg = inner.next().unwrap();
        if !matches!(arg.as_rule(), Rule::arg) {
            num = Some(arg.as_str().parse::<i64>().unwrap());
        } else {
            nodes.push(Self::parse_element(arg));
        }

        nodes.extend(inner.map(Self::parse_element));

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Fr {
                num: num
            },
            nodes
        })
    }

    // Parses (`Ft`)[https://man.openbsd.org/mdoc#Ft]
    fn parse_ft(pair: Pair<Rule>) -> Element {
        let nodes = pair.into_inner().map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Ft,
            nodes
        })
    }

    fn parse_hf(pair: Pair<Rule>) -> Element {
        let file_name = match pair.into_inner().next() {
            Some(file_name) => Some(file_name.as_str().to_string()),
            None => None
        };

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Hf {
                file_name: file_name
            },
            nodes: vec![]
        })
    }

    fn parse_ic(pair: Pair<Rule>) -> Element {
        let mut inner = pair.into_inner();

        let command = inner.next().unwrap().as_str().to_string();
        let nodes = inner.map(Self::parse_element).collect();

        Element::Macro(MacroNode {
            mdoc_macro: Macro::Ic { 
                keyword: command
            },
            nodes
        })
    }

    fn parse_inline(pair: Pair<Rule>) -> Element {
        let pair = pair.into_inner().next().unwrap();
        match pair.as_rule() {
            Rule::rs_submacro => Self::parse_rs_submacro(pair),
            Rule::text_production => Self::parse_text_production(pair),
            Rule::ad => Self::parse_ad(pair),
            Rule::an => Self::parse_an(pair),
            Rule::ap => Self::parse_ap(pair),
            Rule::ar => Self::parse_ar(pair),
            Rule::bt => Self::parse_bt(pair),
            Rule::cd => Self::parse_cd(pair),
            Rule::cm => Self::parse_cm(pair),
            Rule::db => Self::parse_db(pair),
            Rule::dd => Self::parse_dd(pair),
            Rule::dt => Self::parse_dt(pair),
            Rule::dv => Self::parse_dv(pair),
            Rule::em => Self::parse_em(pair),
            Rule::er => Self::parse_er(pair),
            Rule::es => Self::parse_es(pair),
            Rule::ev => Self::parse_ev(pair),
            Rule::ex => Self::parse_ex(pair),
            Rule::fa => Self::parse_fa(pair),
            Rule::fd => Self::parse_fd(pair),
            Rule::fl => Self::parse_fl(pair),
            Rule::Fn => Self::parse_fn(pair),
            Rule::fr => Self::parse_fr(pair),
            Rule::ft => Self::parse_ft(pair),
            Rule::hf => Self::parse_hf(pair),
            Rule::ic => Self::parse_ic(pair),
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod test {
    use chrono;
    use crate::man_util::parser::*;

    #[test]
    fn text_line() {
        let content = "Line 1\nLine 2\nLine 3\n";
        let elements = vec![
            Element::Text("Line 1\n".to_string()),
            Element::Text("Line 2\n".to_string()),
            Element::Text("Line 3\n".to_string()),
        ];

        let mdoc = MdocParser::parse_mdoc(content).unwrap();
        assert_eq!(mdoc.elements, elements);
    }

    mod block_full_explicit {
        use std::collections::HashMap;

        use crate::man_util::parser::*;

        #[test]
        fn bd() {
            let content = ".Bd -literal -offset indent -compact\nLine 1\nLine 2\n.Ed";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bd {
                    block_type: BdType::Literal,
                    offset: Some(OffsetType::Indent),
                    compact: true,
                },
                nodes: vec![
                    Element::Text("Line 1\n".to_string()),
                    Element::Text("Line 2\n".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bd_no_closing_macro() {
            let content = ".Bd -literal -offset indent -compact\nLine 1\nLine 2\n";

            let mdoc = MdocParser::parse_mdoc(content);
            // TODO: Format and compare pest errors??
            assert!(mdoc.is_err());
        }

        #[test]
        fn bd_foreign_closing_macros() {
            let closing_macros = vec![".Ef", ".Ek", ".El"];
            let content = ".Bd -literal -offset indent -compact\nLine 1\nLine 2\n";

            for closing_macro in closing_macros {
                let content = format!("{content}.{closing_macro}");
                let mdoc = MdocParser::parse_mdoc(content);
                // TODO: Format and compare pest errors??
                assert!(mdoc.is_err());
            }
        }

        #[test]
        fn bd_no_body() {
            let content = ".Bd -literal\n.Ed";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bd {
                    block_type: BdType::Literal,
                    offset: None,
                    compact: false,
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bd_type() {
            let mut bd_types: HashMap<&str, BdType> = Default::default();
            bd_types.insert("-centered", BdType::Centered);
            bd_types.insert("-filled", BdType::Filled);
            bd_types.insert("-literal", BdType::Literal);
            bd_types.insert("-ragged", BdType::Ragged);
            bd_types.insert("-unfilled", BdType::Unfilled);

            for (str_type, enum_type) in bd_types {
                let content = format!(".Bd {str_type}\n.Ed");
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Bd {
                        block_type: enum_type,
                        offset: None,
                        compact: false,
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements, "Bd type: {str_type}");
            }
        }

        #[test]
        fn bd_offset() {
            let mut offset_types: HashMap<&str, OffsetType> = Default::default();
            offset_types.insert("indent", OffsetType::Indent);
            offset_types.insert("indent-two", OffsetType::IndentTwo);
            offset_types.insert("left", OffsetType::Left);
            offset_types.insert("right", OffsetType::Right);

            for (str_type, enum_type) in offset_types {
                let content = format!(".Bd -literal -offset {str_type}\n.Ed");
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Bd {
                        block_type: BdType::Literal,
                        offset: Some(enum_type),
                        compact: false,
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements, "Bd offset: {str_type}");
            }
        }

        #[test]
        fn bd_invalid_offset() {
            // TODO: Format and compare pest errors??
            assert!(MdocParser::parse_mdoc(".Bd -literal -offset invalid_offset\n.Ed").is_err())
        }

        #[test]
        fn bd_compact() {
            let content = ".Bd -literal -compact\n.Ed";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bd {
                    block_type: BdType::Literal,
                    offset: None,
                    compact: true,
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bd_not_parsed() {
            // TODO: Format and compare pest errors??
            assert!(MdocParser::parse_mdoc(".Bd -literal -compact Ad addr1\n.Ed").is_err());
        }

        #[test]
        fn bd_not_callable() {
            // TODO: Format and compare pest errors??
            assert!(MdocParser::parse_mdoc(".Ad addr1 Bd -literal\n.Ed").is_err());
        }

        #[test]
        fn bf() {
            let content = ".Bf -emphasis\nLine 1\nLine 2\n.Ef";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bf(BfType::Emphasis),
                nodes: vec![
                    Element::Text("Line 1\n".to_string()),
                    Element::Text("Line 2\n".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bf_no_closing_macro() {
            let content = ".Bf -emphasis\nLine 1\nLine 2\n";

            let mdoc = MdocParser::parse_mdoc(content);
            // TODO: Format and compare pest errors??
            assert!(mdoc.is_err());
        }

        #[test]
        fn bf_foreign_closing_macros() {
            let closing_macros = vec![".Ed", ".Ek", ".El"];
            let content = ".Bf -emphasis\nLine 1\nLine 2\n";

            for closing_macro in closing_macros {
                let content = format!("{content}.{closing_macro}");
                let mdoc = MdocParser::parse_mdoc(content);
                // TODO: Format and compare pest errors??
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

            for (str_type, enum_type) in bf_types {
                let content = format!(".Bf {str_type}\n.Ef");
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Bf(enum_type),
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements, "Bf type: {str_type}");
            }
        }

        #[test]
        fn bf_invalid_type() {
            // TODO: Format and compare pest errors??
            assert!(MdocParser::parse_mdoc(".Bf -invalid\n.Ef").is_err())
        }

        #[test]
        fn bf_not_parsed() {
            // TODO: Format and compare pest errors??
            assert!(MdocParser::parse_mdoc(".Bf Em Ad addr1\n.Ef").is_err());
        }

        #[test]
        fn bf_not_callable() {
            // TODO: Format and compare pest errors??
            assert!(MdocParser::parse_mdoc(".Ad addr1 Bf Em\n.Ef").is_err());
        }

        #[test]
        fn bk() {
            let content = ".Bk -words\nLine 1\nLine 2\n.Ek";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bk,
                nodes: vec![
                    Element::Text("Line 1\n".to_string()),
                    Element::Text("Line 2\n".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bk_no_body() {
            let content = ".Bk -words\n.Ek";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bk,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bk_no_words() {
            let content = ".Bk\n.Ek";

            let mdoc = MdocParser::parse_mdoc(content);
            // TODO: Format and compare pest errors??
            assert!(mdoc.is_err());
        }

        #[test]
        fn bk_not_parsed() {
            // Ignore callable macro as argument
            let content = ".Bk -words Ad addr1\n.Ek";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bk,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bk_not_callable() {
            // TODO: Format and compare pest errors??
            assert!(MdocParser::parse_mdoc(".Ad addr1 Bk -words\n.Ek").is_err());
        }

        #[test]
        fn bl() {
            let content =
                ".Bl -bullet -width indent-two -compact col1 col2 col3\nLine 1\nLine 2\n.El";
            let elements = vec![Element::Macro(MacroNode {
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
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bl_no_closing_macro() {
            let content = ".Bl -bullet\nLine 1\nLine 2\n";

            let mdoc = MdocParser::parse_mdoc(content);
            // TODO: Format and compare pest errors??
            assert!(mdoc.is_err());
        }

        #[test]
        fn bl_foreign_closing_macros() {
            let closing_macros = vec![".Ed", ".Ef", ".Ek"];
            let content = ".Bl -bullet\nLine 1\nLine 2\n";

            for closing_macro in closing_macros {
                let content = format!("{content}.{closing_macro}");
                let mdoc = MdocParser::parse_mdoc(content);
                // TODO: Format and compare pest errors??
                assert!(mdoc.is_err());
            }
        }

        #[test]
        fn bl_no_body() {
            let content = ".Bl -bullet\n.El";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bl {
                    list_type: BlType::Bullet,
                    offset: None,
                    compact: false,
                    columns: vec![],
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
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
                let content = format!(".Bl {str_type}\n.El");
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Bl {
                        list_type: enum_type,
                        offset: None,
                        compact: false,
                        columns: vec![],
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements, "Bl type: {str_type}");
            }
        }

        #[test]
        fn bl_width() {
            let mut width_types: HashMap<&str, OffsetType> = Default::default();
            width_types.insert("indent", OffsetType::Indent);
            width_types.insert("indent-two", OffsetType::IndentTwo);
            width_types.insert("left", OffsetType::Left);
            width_types.insert("right", OffsetType::Right);

            for (str_type, enum_type) in width_types {
                let content = format!(".Bl -bullet -width {str_type}\n.El");
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Bl {
                        list_type: BlType::Bullet,
                        offset: Some(enum_type),
                        compact: false,
                        columns: vec![],
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements, "Bl width: {str_type}");
            }
        }

        #[test]
        fn bl_offset() {
            let mut offset_types: HashMap<&str, OffsetType> = Default::default();
            offset_types.insert("indent", OffsetType::Indent);
            offset_types.insert("indent-two", OffsetType::IndentTwo);
            offset_types.insert("left", OffsetType::Left);
            offset_types.insert("right", OffsetType::Right);

            for (str_type, enum_type) in offset_types {
                let content = format!(".Bl -bullet -offset {str_type}\n.El");
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Bl {
                        list_type: BlType::Bullet,
                        offset: Some(enum_type),
                        compact: false,
                        columns: vec![],
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements, "Bl offset: {str_type}");
            }
        }

        #[test]
        fn bl_invalid_offset() {
            // Because of invalid offset, it is considered as column
            let content = ".Bl -bullet -offset invalid_offset\n.El";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bl {
                    list_type: BlType::Bullet,
                    offset: None,
                    compact: false,
                    columns: vec!["-offset".to_string(), "invalid_offset".to_string()],
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bl_compact() {
            let content = format!(".Bl -bullet -compact\n.El");
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bl {
                    list_type: BlType::Bullet,
                    offset: None,
                    compact: true,
                    columns: vec![],
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bl_columns() {
            let content = format!(".Bl -bullet col1 col2 col3\n.El");
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bl {
                    list_type: BlType::Bullet,
                    offset: None,
                    compact: false,
                    columns: vec!["col1".to_string(), "col2".to_string(), "col3".to_string()],
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bl_not_parsed() {
            // Callable macro as opaque text
            let content = ".Bl -bullet Ad addr1\n.El";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bl {
                    list_type: BlType::Bullet,
                    offset: None,
                    compact: false,
                    columns: vec!["Ad".to_string(), "addr1".to_string()],
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bl_not_callable() {
            // TODO: Format and compare pest errors??
            assert!(MdocParser::parse_mdoc(".Ad addr1 Bl Em\n.El").is_err());
        }
    }

    mod block_full_implicit {
        use crate::man_util::parser::*;

        #[test]
        fn nd() {
            let content = ".Nd short description of the manual";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Nd {
                    line: "short description of the manual".to_string(),
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn nd_with_line_whitespaces_and_tabs() {
            let content = ".Nd short description of the manual\t    \t";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Nd {
                    line: "short description of the manual".to_string(),
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
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
            ];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn nd_with_sh_closure() {
            let content = ".Nd short description\nLine 1\nLine 2\n.Sh SECTION";
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
            ];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn nd_macro_in_body() {
            let content = ".Nd name description\n.Nm name1 name2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Nd {
                    line: "name description".to_string(),
                },
                nodes: vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Nm {
                        name: Some(vec!["name1".to_string(), "name2".to_string()]),
                    },
                    nodes: vec![],
                })],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn nd_not_parsed() {
            let content = ".Nd name Ad addr1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Nd {
                    line: "name Ad addr1".to_string(),
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn nd_not_callable() {
            let content = ".Ad addr1 Nd name description";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("Nd".to_string()),
                    Element::Text("name".to_string()),
                    Element::Text("description".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn nm() {
            let content = ".Nm command_name";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Nm {
                    name: Some(vec!["command_name".to_string()]),
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn nm_multiple_names() {
            let content = ".Nm command few name parts";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Nm {
                    name: Some(vec![
                        "command".to_string(),
                        "few".to_string(),
                        "name".to_string(),
                        "parts".to_string(),
                    ]),
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn nm_with_line_whitespaces_and_tabs() {
            let content = ".Nm command few   name\t\tparts    \t";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Nm {
                    name: Some(vec![
                        "command".to_string(),
                        "few".to_string(),
                        "name".to_string(),
                        "parts".to_string(),
                    ]),
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn nm_no_name() {
            let content = ".Nm";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Nm { name: None },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
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
            let content = ".Nm\n.Nm name 1";
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
            ];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn nm_remember_use_defined() {
            let content = ".Nm name 1\n.Nm";
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
            ];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn nm_remember_use_defined_with_local_overring() {
            let content = ".Nm name 1\n.Nm\n.Nm name 2";
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
            ];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn nm_macro_in_body() {
            let content = ".Nm name1 name2\n.Nd name description";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Nm {
                    name: Some(vec!["name1".to_string(), "name2".to_string()]),
                },
                nodes: vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Nd {
                        line: "name description".to_string(),
                    },
                    nodes: vec![],
                })],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn nm_parsed() {
            let content = ".Nm name1 name2 Ad addr1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Nm {
                    name: Some(vec!["name1".to_string(), "name2".to_string()]),
                },
                nodes: vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![Element::Text("addr1".to_string())],
                })],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn nm_not_callable() {
            let content = ".Ad addr1 Nm name1 name2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("Nm".to_string()),
                    Element::Text("name1".to_string()),
                    Element::Text("name2".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn sh() {
            let content = ".Sh SECTION\nThis is the SECTION section.\n";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Sh {
                    title: "SECTION".to_string(),
                },
                nodes: vec![Element::Text("This is the SECTION section.\n".to_string())],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn sh_with_multiple_lines() {
            let content = ".Sh SECTION\nLine 1\nLine 2\nLine 3\n";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Sh {
                    title: "SECTION".to_string(),
                },
                nodes: vec![
                    Element::Text("Line 1\n".to_string()),
                    Element::Text("Line 2\n".to_string()),
                    Element::Text("Line 3\n".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn sh_without_title() {
            let content = ".Sh\nLine 1\n";

            let mdoc = MdocParser::parse_mdoc(content);
            // TODO: Format and compare pest errors??
            assert!(mdoc.is_err());
        }

        #[test]
        fn sh_without_body() {
            let content = ".Sh SECTION";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Sh {
                    title: "SECTION".to_string(),
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn sh_title_line() {
            let content = ".Sh TITLE LINE\nLine 1\n";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Sh {
                    title: "TITLE LINE".to_string(),
                },
                nodes: vec![Element::Text("Line 1\n".to_string())],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
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
                Err(MdocError::Validation(
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
                Err(MdocError::Validation(
                    ".Sh NAME must end with .Nd".to_string()
                ))
            );
        }

        #[test]
        fn sh_name_with_nd() {
            let content = ".Sh NAME\nLine 1\n.Nd short description";
            let elements = vec![Element::Macro(MacroNode {
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
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn sh_name_with_nd_in_nm() {
            let content = ".Sh NAME\nLine 1\n.Nm utility\n.Nd short description";
            let elements = vec![Element::Macro(MacroNode {
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
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn sh_parsed() {
            // Although this macro is parsed, it should not consist of child
            // node or it may not be linked with Sx.
            let content = ".Sh SECTION Ad addr1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Sh {
                    title: "SECTION Ad addr1".to_string(),
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn sh_not_callable() {
            let content = ".Ad addr1 Sh SECTION";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("Sh".to_string()),
                    Element::Text("SECTION".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ss() {
            let content = ".Ss Options\nThese are the available options.\n";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ss {
                    title: "Options".to_string(),
                },
                nodes: vec![Element::Text(
                    "These are the available options.\n".to_string(),
                )],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ss_with_multiple_lines() {
            let content = ".Ss Options\nLine 1\nLine 2\nLine 3\n";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ss {
                    title: "Options".to_string(),
                },
                nodes: vec![
                    Element::Text("Line 1\n".to_string()),
                    Element::Text("Line 2\n".to_string()),
                    Element::Text("Line 3\n".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ss_without_title() {
            let content = ".Ss\nLine 1\n";

            let mdoc = MdocParser::parse_mdoc(content);
            // TODO: Format and compare pest errors??
            assert!(mdoc.is_err());
        }

        #[test]
        fn ss_without_body() {
            let content = ".Ss Options";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ss {
                    title: "Options".to_string(),
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ss_title_line() {
            let content = ".Ss TITLE LINE\nLine 1\n";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ss {
                    title: "TITLE LINE".to_string(),
                },
                nodes: vec![Element::Text("Line 1\n".to_string())],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ss_nested_in_sh() {
            let content = ".Sh SECTION\n.Ss Subsection\nLine 1\n";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Sh {
                    title: "SECTION".to_string(),
                },
                nodes: vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ss {
                        title: "Subsection".to_string(),
                    },
                    nodes: vec![Element::Text("Line 1\n".to_string())],
                })],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
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
                Err(MdocError::Validation(
                    "Duplicate .Ss title found: Subchapter 1".to_string()
                ))
            );
        }

        #[test]
        fn ss_macro_in_body() {
            let content = ".Ss Subchapter\n.Nm name1 name2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ss {
                    title: "Subchapter".to_string(),
                },
                nodes: vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Nm {
                        name: Some(vec!["name1".to_string(), "name2".to_string()]),
                    },
                    nodes: vec![],
                })],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ss_parsed() {
            // Although this macro is parsed, it should not consist of child
            // node or it may not be linked with Sx.
            let content = ".Ss Subchapter Ad addr1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ss {
                    title: "Subchapter Ad addr1".to_string(),
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ss_not_callable() {
            let content = ".Ad addr1 Ss Subchapter";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("Ss".to_string()),
                    Element::Text("Subchapter".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }
    }

    mod block_partial_implicit {
        use crate::man_util::parser::*;

        #[test]
        fn aq_empty() {
            let content = ".Aq";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Aq,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn aq_text_line() {
            let content = ".Aq Line 1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Aq,
                nodes: vec![
                    Element::Text("Line".to_string()),
                    Element::Text("1".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn aq_parsed() {
            let content = ".Aq Text Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Aq,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn aq_callable() {
            let content = ".Ad addr1 Aq addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Aq,
                        nodes: vec![Element::Text("addr2".to_string())],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bq_empty() {
            let content = ".Bq";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bq,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bq_text_line() {
            let content = ".Bq Line 1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bq,
                nodes: vec![
                    Element::Text("Line".to_string()),
                    Element::Text("1".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bq_parsed() {
            let content = ".Bq Text Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bq,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bq_callable() {
            let content = ".Ad addr1 Bq addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Bq,
                        nodes: vec![Element::Text("addr2".to_string())],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn brq_empty() {
            let content = ".Brq";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Brq,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn brq_text_line() {
            let content = ".Brq Line 1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Brq,
                nodes: vec![
                    Element::Text("Line".to_string()),
                    Element::Text("1".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn brq_parsed() {
            let content = ".Brq Text Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Brq,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn brq_callable() {
            let content = ".Ad addr1 Brq addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Brq,
                        nodes: vec![Element::Text("addr2".to_string())],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn d1_empty() {
            let content = ".D1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::D1,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn d1_text_line() {
            let content = ".D1 Line 1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::D1,
                nodes: vec![
                    Element::Text("Line".to_string()),
                    Element::Text("1".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn d1_parsed() {
            let content = ".D1 Text Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::D1,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn d1_not_callable() {
            let content = ".Ad addr1 D1 addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("D1".to_string()),
                    Element::Text("addr2".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dl_empty() {
            let content = ".Dl";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dl,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dl_text_line() {
            let content = ".Dl Line 1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dl,
                nodes: vec![
                    Element::Text("Line".to_string()),
                    Element::Text("1".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dl_parsed() {
            let content = ".Dl Text Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dl,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dl_not_callable() {
            let content = ".Ad addr1 Dl addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("Dl".to_string()),
                    Element::Text("addr2".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dq_empty() {
            let content = ".Dq";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dq,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dq_text_line() {
            let content = ".Dq Line 1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dq,
                nodes: vec![
                    Element::Text("Line".to_string()),
                    Element::Text("1".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dq_parsed() {
            let content = ".Dq Text Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dq,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dq_callable() {
            let content = ".Ad addr1 Dq addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Dq,
                        nodes: vec![Element::Text("addr2".to_string())],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn en() {
            let content = ".En word1 word2 word3";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::En,
                nodes: vec![
                    Element::Text("word1".to_string()),
                    Element::Text("word2".to_string()),
                    Element::Text("word3".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn en_no_words() {
            // TODO: Format and compare pest errors??
            assert!(MdocParser::parse_mdoc(".En").is_err());
        }

        #[test]
        fn en_parsed() {
            let content = ".En Text Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::En,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn en_callable() {
            let content = ".Ad addr1 En addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::En,
                        nodes: vec![Element::Text("addr2".to_string())],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn op_empty() {
            let content = ".Op";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Op,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn op_text_line() {
            let content = ".Op Line 1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Op,
                nodes: vec![
                    Element::Text("Line".to_string()),
                    Element::Text("1".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn op_parsed() {
            let content = ".Op Text Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Op,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn op_callable() {
            let content = ".Ad addr1 Op addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Op,
                        nodes: vec![Element::Text("addr2".to_string())],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn pq_empty() {
            let content = ".Pq";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Pq,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn pq_text_line() {
            let content = ".Pq Line 1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Pq,
                nodes: vec![
                    Element::Text("Line".to_string()),
                    Element::Text("1".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn pq_parsed() {
            let content = ".Pq Text Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Pq,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn pq_callable() {
            let content = ".Ad addr1 Pq addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Pq,
                        nodes: vec![Element::Text("addr2".to_string())],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ql_empty() {
            let content = ".Ql";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ql,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ql_text_line() {
            let content = ".Ql Line 1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ql,
                nodes: vec![
                    Element::Text("Line".to_string()),
                    Element::Text("1".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ql_parsed() {
            let content = ".Ql Text Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ql,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ql_callable() {
            let content = ".Ad addr1 Ql addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ql,
                        nodes: vec![Element::Text("addr2".to_string())],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn qq_empty() {
            let content = ".Qq";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Qq,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn qq_text_line() {
            let content = ".Qq Line 1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Qq,
                nodes: vec![
                    Element::Text("Line".to_string()),
                    Element::Text("1".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn qq_parsed() {
            let content = ".Qq Text Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Qq,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn qq_callable() {
            let content = ".Ad addr1 Qq addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Qq,
                        nodes: vec![Element::Text("addr2".to_string())],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn sq_empty() {
            let content = ".Sq";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Sq,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn sq_text_line() {
            let content = ".Sq Line 1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Sq,
                nodes: vec![
                    Element::Text("Line".to_string()),
                    Element::Text("1".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn sq_parsed() {
            let content = ".Sq Text Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Sq,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn sq_callable() {
            let content = ".Ad addr1 Sq addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Sq,
                        nodes: vec![Element::Text("addr2".to_string())],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn vt() {
            let content = ".Vt type some identifier";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Vt,
                nodes: vec![
                    Element::Text("type".to_string()),
                    Element::Text("some".to_string()),
                    Element::Text("identifier".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn vt_empty() {
            // TODO: Format and compare pest errors??
            assert!(MdocParser::parse_mdoc(".Vt").is_err());
        }

        #[test]
        fn vt_only_type() {
            let content = ".Vt type";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Vt,
                nodes: vec![Element::Text("type".to_string())],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn vt_parsed() {
            let content = ".Vt Text Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Vt,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn vt_callable() {
            let content = ".Ad addr1 Vt addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Vt,
                        nodes: vec![Element::Text("addr2".to_string())],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }
    }

    mod inline {
        use chrono::Datelike;

        use crate::man_util::parser::*;

        mod rs_submacros {
            use crate::man_util::parser::*;

            #[test]
            fn a() {
                let content = ".%A John Doe";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::A {
                        author_name: "John Doe".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn a_with_whitespaces() {
                let content = ".%A John  \t  Doe\n";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::A {
                        author_name: "John Doe".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn a_no_args() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%A").is_err());
            }

            #[test]
            fn a_not_parsed() {
                let content = ".%A John Doe Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::A {
                        author_name: "John Doe Ad addr1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn a_not_callable() {
                let content = ".Ad addr1 %A John Doe";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("%A".to_string()),
                        Element::Text("John".to_string()),
                        Element::Text("Doe".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn b() {
                let content = ".%B Title Line";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::B {
                        book_title: "Title Line".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn b_with_whitespaces() {
                let content = ".%B Title  \t  Line\n";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::B {
                        book_title: "Title Line".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn b_no_args() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%B").is_err());
            }

            #[test]
            fn b_not_parsed() {
                let content = ".%B Title Line Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::B {
                        book_title: "Title Line Ad addr1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn b_not_callable() {
                let content = ".Ad addr1 %B Title Line";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("%B".to_string()),
                        Element::Text("Title".to_string()),
                        Element::Text("Line".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn c() {
                let content = ".%C Location line";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::C {
                        publication_location: "Location line".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn c_with_whitespaces() {
                let content = ".%C Location  \t  Line\n";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::C {
                        publication_location: "Location Line".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn c_no_args() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%C").is_err());
            }

            #[test]
            fn c_not_parsed() {
                let content = ".%C Location Line Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::C {
                        publication_location: "Location Line Ad addr1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn c_not_callable() {
                let content = ".Ad addr1 %C Location Line";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("%C".to_string()),
                        Element::Text("Location".to_string()),
                        Element::Text("Line".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn d() {
                let content = ".%D January 1, 1970";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::D {
                        month_day: Some(("January".to_string(), 1)),
                        year: 1970,
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn d_with_whitespaces() {
                let content = ".%D January  \t  1,  \t  1970\n";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::D {
                        month_day: Some(("January".to_string(), 1)),
                        year: 1970,
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn d_no_month_day() {
                let content = ".%D 1970";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::D {
                        month_day: None,
                        year: 1970,
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn d_no_args() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%D").is_err());
            }

            #[test]
            fn d_not_parsed() {
                assert!(MdocParser::parse_mdoc(".%D January 1, 1970 Ad addr1").is_err());
            }

            #[test]
            fn d_not_callable() {
                let content = ".Ad addr1 %D January 1, 1970";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("%D".to_string()),
                        Element::Text("January".to_string()),
                        Element::Text("1,".to_string()),
                        Element::Text("1970".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn i() {
                let content = ".%I John Doe";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::I {
                        issuer_name: "John Doe".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn i_with_whitespaces() {
                let content = ".%I John  \t  Doe\n";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::I {
                        issuer_name: "John Doe".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn i_no_args() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%I").is_err());
            }

            #[test]
            fn i_not_parsed() {
                let content = ".%I John Doe Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::I {
                        issuer_name: "John Doe Ad addr1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn i_not_callable() {
                let content = ".Ad addr1 %I John Doe";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("%I".to_string()),
                        Element::Text("John".to_string()),
                        Element::Text("Doe".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn j() {
                let content = ".%J Journal Name Line";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::J {
                        journal_name: "Journal Name Line".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn j_with_whitespaces() {
                let content = ".%J Journal  \t  Name  \t  Line\n";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::J {
                        journal_name: "Journal Name Line".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn j_no_args() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%J").is_err());
            }

            #[test]
            fn j_not_parsed() {
                let content = ".%J Journal Name Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::J {
                        journal_name: "Journal Name Ad addr1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn j_not_callable() {
                let content = ".Ad addr1 %J Journal Name";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("%J".to_string()),
                        Element::Text("Journal".to_string()),
                        Element::Text("Name".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn n() {
                let content = ".%N Issue No. 1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::N {
                        issue_number: "Issue No. 1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn n_with_whitespaces() {
                let content = ".%N Issue  \t  No.  \t  1\n";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::N {
                        issue_number: "Issue No. 1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn n_no_args() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%N").is_err());
            }

            #[test]
            fn n_not_parsed() {
                let content = ".%N Issue No. 1 Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::N {
                        issue_number: "Issue No. 1 Ad addr1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn n_not_callable() {
                let content = ".Ad addr1 %N Issue No. 1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("%N".to_string()),
                        Element::Text("Issue".to_string()),
                        Element::Text("No.".to_string()),
                        Element::Text("1".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn o() {
                let content = ".%O Optional information line";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::O {
                        information: "Optional information line".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn o_with_whitespaces() {
                let content = ".%O Optional  \t  information  \t  line\n";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::O {
                        information: "Optional information line".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn o_no_args() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%O").is_err());
            }

            #[test]
            fn o_not_parsed() {
                let content = ".%O Optional information Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::O {
                        information: "Optional information Ad addr1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn o_not_callable() {
                let content = ".Ad addr1 %O Optional information";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("%O".to_string()),
                        Element::Text("Optional".to_string()),
                        Element::Text("information".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn p() {
                let content = ".%P pp. 1-100";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::P {
                        page_number: "pp. 1-100".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn p_with_whitespaces() {
                let content = ".%P pp.  \t  1-100\n";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::P {
                        page_number: "pp. 1-100".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn p_no_args() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%P").is_err());
            }

            #[test]
            fn p_not_parsed() {
                let content = ".%P pp. 1-100 Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::P {
                        page_number: "pp. 1-100 Ad addr1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn p_not_callable() {
                let content = ".Ad addr1 %P pp. 1-100";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("%P".to_string()),
                        Element::Text("pp.".to_string()),
                        Element::Text("1-100".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn q() {
                let content = ".%Q John Doe";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Q {
                        insitution_author: "John Doe".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn q_with_whitespaces() {
                let content = ".%Q John  \t  Doe\n";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Q {
                        insitution_author: "John Doe".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn q_no_args() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%Q").is_err());
            }

            #[test]
            fn q_not_parsed() {
                let content = ".%Q John Doe Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Q {
                        insitution_author: "John Doe Ad addr1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn q_not_callable() {
                let content = ".Ad addr1 %Q John Doe";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("%Q".to_string()),
                        Element::Text("John".to_string()),
                        Element::Text("Doe".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn r() {
                let content = ".%R Technical report No. 1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::R {
                        report_name: "Technical report No. 1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn r_with_whitespaces() {
                let content = ".%R Technical  \t  report  \t  No.  \t  1\n";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::R {
                        report_name: "Technical report No. 1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn r_no_args() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%R").is_err());
            }

            #[test]
            fn r_not_parsed() {
                let content = ".%R Technical report Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::R {
                        report_name: "Technical report Ad addr1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn r_not_callable() {
                let content = ".Ad addr1 %R Technical report";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("%R".to_string()),
                        Element::Text("Technical".to_string()),
                        Element::Text("report".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn t() {
                let content = ".%T Article title line";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::T {
                        article_title: "Article title line".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn t_with_whitespaces() {
                let content = ".%T Article  \t  title  \t  line\n";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::T {
                        article_title: "Article title line".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn t_no_args() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%T").is_err());
            }

            #[test]
            fn t_not_parsed() {
                let content = ".%T Article title Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::T {
                        article_title: "Article title Ad addr1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn t_not_callable() {
                let content = ".Ad addr1 %T Article title";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("%T".to_string()),
                        Element::Text("Article".to_string()),
                        Element::Text("title".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn u() {
                let content = ".%U protocol://path";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::U {
                        uri: "protocol://path".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn u_with_whitespaces() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%U protocol :// path").is_err());
            }

            #[test]
            fn u_invalid_uri() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%U some_non_uri_text").is_err());
            }

            #[test]
            fn u_no_args() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%U").is_err());
            }

            #[test]
            fn u_not_parsed() {
                assert!(MdocParser::parse_mdoc(".%U Ad addr1").is_err());
            }

            #[test]
            fn u_not_callable() {
                let content = ".Ad addr1 %U protocol://path";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("%U".to_string()),
                        Element::Text("protocol://path".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn v() {
                let content = ".%V Volume No. 1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::V {
                        volume_number: "Volume No. 1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn v_with_whitespaces() {
                let content = ".%V Volume  \t  No.  \t  1\n";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::V {
                        volume_number: "Volume No. 1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn v_no_args() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".%V").is_err());
            }

            #[test]
            fn v_not_parsed() {
                let content = ".%V Volume No. 1 Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::V {
                        volume_number: "Volume No. 1 Ad addr1".to_string(),
                    },
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn v_not_callable() {
                let content = ".Ad addr1 %V Volume No. 1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("%V".to_string()),
                        Element::Text("Volume".to_string()),
                        Element::Text("No.".to_string()),
                        Element::Text("1".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }
        }

        mod text_production {
            use std::collections::HashMap;

            use crate::man_util::parser::*;

            #[test]
            fn at() {
                let mut at_types: HashMap<&str, AtType> = Default::default();
                at_types.insert("", AtType::General);
                at_types.insert("v1", AtType::Version("1".to_string()));
                at_types.insert("v2", AtType::Version("2".to_string()));
                at_types.insert("v3", AtType::Version("3".to_string()));
                at_types.insert("v4", AtType::Version("4".to_string()));
                at_types.insert("v5", AtType::Version("5".to_string()));
                at_types.insert("v6", AtType::Version("6".to_string()));
                at_types.insert("v7", AtType::Version("7".to_string()));
                at_types.insert("32v", AtType::V32);
                at_types.insert("III", AtType::SystemIII);
                at_types.insert("V", AtType::SystemV(None));
                at_types.insert("V.1", AtType::SystemV(Some("1".to_string())));
                at_types.insert("V.2", AtType::SystemV(Some("2".to_string())));
                at_types.insert("V.3", AtType::SystemV(Some("3".to_string())));
                at_types.insert("V.4", AtType::SystemV(Some("4".to_string())));

                for (str_type, enum_type) in at_types {
                    let content = format!(".At {str_type} word");
                    let elements = vec![Element::Macro(MacroNode {
                        mdoc_macro: Macro::At(enum_type),
                        nodes: vec![Element::Text("word".to_string())],
                    })];

                    let mdoc = MdocParser::parse_mdoc(content).unwrap();
                    assert_eq!(mdoc.elements, elements, "At type: {str_type}");
                }
            }

            #[test]
            fn at_other_text_values() {
                let at_args = vec![
                    "v0".to_string(),
                    "v8".to_string(),
                    "V.0".to_string(),
                    "V.5".to_string(),
                    "word".to_string(),
                ];

                for arg in at_args {
                    let content = format!(".At {arg} word");
                    let elements = vec![Element::Macro(MacroNode {
                        mdoc_macro: Macro::At(AtType::General),
                        nodes: vec![
                            Element::Text(arg.clone()),
                            Element::Text("word".to_string()),
                        ],
                    })];

                    let mdoc = MdocParser::parse_mdoc(content).unwrap();
                    assert_eq!(mdoc.elements, elements, "At type: {arg}");
                }
            }

            #[test]
            fn at_parsed() {
                let content = ".At v1 Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::At(AtType::Version("1".to_string())),
                    nodes: vec![Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![Element::Text("addr1".to_string())],
                    })],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn at_callable_with_arg() {
                let content = ".Ad addr1 At v1 word";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Macro(MacroNode {
                            mdoc_macro: Macro::At(AtType::Version(1.to_string())),
                            nodes: vec![Element::Text("word".to_string())],
                        }),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn at_callable_without_arg() {
                let content = ".Ad addr1 At word";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Macro(MacroNode {
                            mdoc_macro: Macro::At(AtType::General),
                            nodes: vec![Element::Text("word".to_string())],
                        }),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn bsx() {
                let content = ".Bsx Version 1.0";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Bsx(BsxType {
                        version: vec!["Version".to_string(), "1.0".to_string()],
                    }),
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn bsx_no_args() {
                let content = ".Bsx";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Bsx(BsxType { version: vec![] }),
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn bsx_parsed() {
                let content = ".Bsx Version 1.0 Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Bsx(BsxType {
                        version: vec!["Version".to_string(), "1.0".to_string()],
                    }),
                    nodes: vec![Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![Element::Text("addr1".to_string())],
                    })],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn bsx_callable_with_arg() {
                let content = ".Ad addr1 Bsx Version 1.0";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Macro(MacroNode {
                            mdoc_macro: Macro::Bsx(BsxType {
                                version: vec!["Version".to_string(), "1.0".to_string()],
                            }),
                            nodes: vec![],
                        }),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn bsx_callable_without_arg() {
                let content = ".Ad addr1 Bsx";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Macro(MacroNode {
                            mdoc_macro: Macro::Bsx(BsxType { version: vec![] }),
                            nodes: vec![],
                        }),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn bx() {
                let mut bx_args: HashMap<&str, (Option<String>, Vec<String>)> = Default::default();
                bx_args.insert("", (None, vec![]));
                bx_args.insert("4.3", (Some("4.3".to_string()), vec![]));
                bx_args.insert(
                    "4.3 Tahoe Example",
                    (
                        Some("4.3".to_string()),
                        vec!["Tahoe".to_string(), "Example".to_string()],
                    ),
                );

                for (args, (version, variant)) in bx_args {
                    let content = format!(".Bx {args}");
                    let elements = vec![Element::Macro(MacroNode {
                        mdoc_macro: Macro::Bx(BxType { version, variant }),
                        nodes: vec![],
                    })];

                    let mdoc = MdocParser::parse_mdoc(content).unwrap();
                    assert_eq!(mdoc.elements, elements, "Bx args: {args}");
                }
            }

            #[test]
            fn bx_parsed() {
                let content = ".Bx 4.3 Tahoe Example Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Bx(BxType {
                        version: Some("4.3".to_string()),
                        variant: vec!["Tahoe".to_string(), "Example".to_string()],
                    }),
                    nodes: vec![Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![Element::Text("addr1".to_string())],
                    })],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn bx_callable_with_arg() {
                let content = ".Ad addr1 Bx 4.3 Tahoe Example";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Macro(MacroNode {
                            mdoc_macro: Macro::Bx(BxType {
                                version: Some("4.3".to_string()),
                                variant: vec!["Tahoe".to_string(), "Example".to_string()],
                            }),
                            nodes: vec![],
                        }),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn bx_callable_without_arg() {
                let content = ".Ad addr1 Bx";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Macro(MacroNode {
                            mdoc_macro: Macro::Bx(BxType {
                                version: None,
                                variant: vec![],
                            }),
                            nodes: vec![],
                        }),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn dx() {
                let content = ".Dx Version 1.0";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Dx(DxType {
                        version: vec!["Version".to_string(), "1.0".to_string()],
                    }),
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn dx_no_args() {
                let content = ".Dx";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Dx(DxType { version: vec![] }),
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn dx_parsed() {
                let content = ".Dx Version 1.0 Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Dx(DxType {
                        version: vec!["Version".to_string(), "1.0".to_string()],
                    }),
                    nodes: vec![Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![Element::Text("addr1".to_string())],
                    })],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn dx_callable_with_arg() {
                let content = ".Ad addr1 Dx Version 1.0";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Macro(MacroNode {
                            mdoc_macro: Macro::Dx(DxType {
                                version: vec!["Version".to_string(), "1.0".to_string()],
                            }),
                            nodes: vec![],
                        }),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn dx_callable_without_arg() {
                let content = ".Ad addr1 Dx";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Macro(MacroNode {
                            mdoc_macro: Macro::Dx(DxType { version: vec![] }),
                            nodes: vec![],
                        }),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn nx() {
                let content = ".Nx Version 1.0";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Nx(NxType {
                        version: vec!["Version".to_string(), "1.0".to_string()],
                    }),
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn nx_no_args() {
                let content = ".Nx";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Nx(NxType { version: vec![] }),
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn nx_parsed() {
                let content = ".Nx Version 1.0 Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Nx(NxType {
                        version: vec!["Version".to_string(), "1.0".to_string()],
                    }),
                    nodes: vec![Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![Element::Text("addr1".to_string())],
                    })],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn nx_callable_with_arg() {
                let content = ".Ad addr1 Nx Version 1.0";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Macro(MacroNode {
                            mdoc_macro: Macro::Nx(NxType {
                                version: vec!["Version".to_string(), "1.0".to_string()],
                            }),
                            nodes: vec![],
                        }),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn nx_callable_without_arg() {
                let content = ".Ad addr1 Nx";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Macro(MacroNode {
                            mdoc_macro: Macro::Nx(NxType { version: vec![] }),
                            nodes: vec![],
                        }),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn ox() {
                let content = ".Ox Version 1.0";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ox(OxType {
                        version: vec!["Version".to_string(), "1.0".to_string()],
                    }),
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn ox_no_args() {
                let content = ".Ox";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ox(OxType { version: vec![] }),
                    nodes: vec![],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn ox_parsed() {
                let content = ".Ox Version 1.0 Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ox(OxType {
                        version: vec!["Version".to_string(), "1.0".to_string()],
                    }),
                    nodes: vec![Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![Element::Text("addr1".to_string())],
                    })],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn ox_callable_with_arg() {
                let content = ".Ad addr1 Ox Version 1.0";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Macro(MacroNode {
                            mdoc_macro: Macro::Ox(OxType {
                                version: vec!["Version".to_string(), "1.0".to_string()],
                            }),
                            nodes: vec![],
                        }),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn ox_callable_without_arg() {
                let content = ".Ad addr1 Ox";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Macro(MacroNode {
                            mdoc_macro: Macro::Ox(OxType { version: vec![] }),
                            nodes: vec![],
                        }),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn st() {
                let mut st_types: HashMap<&str, StType> = Default::default();
                // C Language Standards
                st_types.insert("-ansiC", StType::AnsiC);
                st_types.insert("-ansiC-89", StType::AnsiC89);
                st_types.insert("-isoC", StType::IsoC);
                st_types.insert("-isoC-90", StType::IsoC90);
                st_types.insert("-isoC-amd1", StType::IsoCAmd1);
                st_types.insert("-isoC-tcor1", StType::IsoCTcor1);
                st_types.insert("-isoC-tcor2", StType::IsoCTcor2);
                st_types.insert("-isoC-99", StType::IsoC99);
                st_types.insert("-isoC-2011", StType::IsoC2011);
                // POSIX.1 Standards before XPG4.2
                st_types.insert("-p1003.1-88", StType::P1003188);
                st_types.insert("-p1003.1", StType::P10031);
                st_types.insert("-p1003.1-90", StType::P1003190);
                st_types.insert("-iso9945-1-90", StType::Iso9945190);
                st_types.insert("-p1003.1b-93", StType::P10031B93);
                st_types.insert("-p1003.1b", StType::P10031B);
                st_types.insert("-p1003.1c-95", StType::P10031C95);
                st_types.insert("-p1003.1i-95", StType::P10031I95);
                st_types.insert("-p1003.1-96", StType::P1003196);
                st_types.insert("-iso9945-1-96", StType::Iso9945196);
                // X/Open Portability Guide before XPG4.2
                st_types.insert("-xpg3", StType::Xpg3);
                st_types.insert("-p1003.2", StType::P10032);
                st_types.insert("-p1003.2-92", StType::P1003292);
                st_types.insert("-iso9945-2-93", StType::Iso9945293);
                st_types.insert("-p1003.2a-92", StType::P10032A92);
                st_types.insert("-xpg4", StType::Xpg4);
                // X/Open Portability Guide Issue 4 Version 2 and Related Standards
                st_types.insert("-susv1", StType::Susv1);
                st_types.insert("-xpg4.2", StType::Xpg42);
                st_types.insert("-xcurses4.2", StType::XCurses42);
                st_types.insert("-p1003.1g-2000", StType::P10031G2000);
                st_types.insert("-svid4", StType::Svid4);
                // X/Open Portability Guide Issue 5 and Related Standards
                st_types.insert("-susv2", StType::Susv2);
                st_types.insert("-xbd5", StType::Xbd5);
                st_types.insert("-xsh5", StType::Xsh5);
                st_types.insert("-xcu5", StType::Xcu5);
                st_types.insert("-xns5", StType::Xns5);
                st_types.insert("-xns5.2", StType::Xns52);
                // POSIX Issue 6 Standards
                st_types.insert("-p1003.1-2001", StType::P100312001);
                st_types.insert("-susv3", StType::Susv3);
                st_types.insert("-p1003.1-2004", StType::P100312004);
                // POSIX Issues 7 and 8 Standards
                st_types.insert("-p1003.1-2008", StType::P100312008);
                st_types.insert("-susv4", StType::Susv4);
                st_types.insert("-p1003.1-2024", StType::P100312024);
                // Other Standards
                st_types.insert("-ieee754", StType::Ieee754);
                st_types.insert("-iso8601", StType::Iso8601);
                st_types.insert("-iso8802-3", StType::Iso88023);
                st_types.insert("-ieee1275-94", StType::Ieee127594);

                for (str_type, enum_type) in st_types {
                    let content = format!(".St {str_type} word");
                    let elements = vec![Element::Macro(MacroNode {
                        mdoc_macro: Macro::St(enum_type),
                        nodes: vec![Element::Text("word".to_string())],
                    })];

                    let mdoc = MdocParser::parse_mdoc(content).unwrap();
                    assert_eq!(mdoc.elements, elements, "St type: {str_type}");
                }
            }

            #[test]
            fn st_no_abbreviation() {
                // TODO: Format and compare pest errors??
                assert!(MdocParser::parse_mdoc(".St word").is_err())
            }

            #[test]
            fn st_parsed() {
                let content = ".St -ansiC word Ad addr1";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::St(StType::AnsiC),
                    nodes: vec![
                        Element::Text("word".to_string()),
                        Element::Macro(MacroNode {
                            mdoc_macro: Macro::Ad,
                            nodes: vec![Element::Text("addr1".to_string())],
                        }),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }

            #[test]
            fn st_not_callable() {
                let content = ".Ad addr1 St -ansiC word";
                let elements = vec![Element::Macro(MacroNode {
                    mdoc_macro: Macro::Ad,
                    nodes: vec![
                        Element::Text("addr1".to_string()),
                        Element::Text("St".to_string()),
                        Element::Text("-ansiC".to_string()),
                        Element::Text("word".to_string()),
                    ],
                })];

                let mdoc = MdocParser::parse_mdoc(content).unwrap();
                assert_eq!(mdoc.elements, elements);
            }
        }

        #[test]
        fn ad() {
            let content = ".Ad addr1 addr2 addr3";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("addr2".to_string()),
                    Element::Text("addr3".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ad_no_args() {
            // TODO: Format and compare pest errors??
            assert!(MdocParser::parse_mdoc(".Ad").is_err());
        }

        #[test]
        fn ad_parsed() {
            let content = ".Ad addr1 Ar arg1 arg2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ar,
                        nodes: vec![
                            Element::Text("arg1".to_string()),
                            Element::Text("arg2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ad_callable() {
            let content = ".Ap word1 Ad addr1 addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ap,
                nodes: vec![
                    Element::Text("word1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn an_split() {
            let content = ".An -split";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::An {
                    author_name_type: AnType::Split,
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn an_nosplit() {
            let content = ".An -nosplit";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::An {
                    author_name_type: AnType::NoSplit,
                },
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn an_name() {
            let content = ".An John Doe";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::An {
                    author_name_type: AnType::Name,
                },
                nodes: vec![
                    Element::Text("John".to_string()),
                    Element::Text("Doe".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn an_no_args() {
            // TODO: Format and compare pest errors??
            assert!(MdocParser::parse_mdoc(".An").is_err());
        }

        #[test]
        fn an_parsed() {
            let content = ".An Name Ad addr1 addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::An {
                    author_name_type: AnType::Name,
                },
                nodes: vec![
                    Element::Text("Name".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn an_callable() {
            let content = ".Ap word1 An -nosplit";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ap,
                nodes: vec![
                    Element::Text("word1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::An {
                            author_name_type: AnType::NoSplit,
                        },
                        nodes: vec![],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ap() {
            let content = ".Ap Text Line";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ap,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Text("Line".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ap_no_args() {
            let content = ".Ap";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ap,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ap_parsed() {
            let content = ".Ap Text Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ap,
                nodes: vec![
                    Element::Text("Text".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ap_callable() {
            let content = ".Ad addr1 Ap word1 word2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ap,
                        nodes: vec![
                            Element::Text("word1".to_string()),
                            Element::Text("word2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ar() {
            let content = ".Ar arg1 arg2 arg3";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ar,
                nodes: vec![
                    Element::Text("arg1".to_string()),
                    Element::Text("arg2".to_string()),
                    Element::Text("arg3".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ar_no_args() {
            let content = ".Ar";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ar,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ar_parsed() {
            let content = ".Ar arg1 Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ar,
                nodes: vec![
                    Element::Text("arg1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ar_callable() {
            let content = ".Ad addr1 Ap word1 word2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ap,
                        nodes: vec![
                            Element::Text("word1".to_string()),
                            Element::Text("word2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bt() {
            // "Text Line" will be ignored
            let content = ".Bt Text Line";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bt,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bt_no_args() {
            let content = ".Bt";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bt,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bt_not_parsed() {
            // "Ad" macro will be ignored
            let content = ".Bt Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Bt,
                nodes: vec![],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn bt_not_callable() {
            let content = ".Ad addr1 Bt addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("Bt".to_string()),
                    Element::Text("addr2".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn cd() {
            let content = ".Cd kernel configuration declaration";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Cd,
                nodes: vec![
                    Element::Text("kernel".to_string()),
                    Element::Text("configuration".to_string()),
                    Element::Text("declaration".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn cd_no_args() {
            // TODO: Format and compare pest errors??
            assert!(MdocParser::parse_mdoc(".Cd").is_err());
        }

        #[test]
        fn cd_parsed() {
            let content = ".Cd declaration Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Cd,
                nodes: vec![
                    Element::Text("declaration".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn cd_callable() {
            let content = ".Ad addr1 Cd configuration declaration";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Cd,
                        nodes: vec![
                            Element::Text("configuration".to_string()),
                            Element::Text("declaration".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn cm() {
            let content = ".Cm mod1 mod2 mod3";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Cm,
                nodes: vec![
                    Element::Text("mod1".to_string()),
                    Element::Text("mod2".to_string()),
                    Element::Text("mod3".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn cm_no_args() {
            // TODO: Format and compare pest errors??
            assert!(MdocParser::parse_mdoc(".Cm").is_err());
        }

        #[test]
        fn cm_parsed() {
            let content = ".Cm cmdm1 cmdm2 Ad addr1 addr2";

            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Cm,
                nodes: vec![
                    Element::Text("cmdm1".to_string()),
                    Element::Text("cmdm2".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn cm_callable() {
            let content = ".Ad addr1 Cm mod1 mod2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Cm,
                        nodes: vec![
                            Element::Text("mod1".to_string()),
                            Element::Text("mod2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn db() {
            let content = ".Db text_argument";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Db,
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn db_not_callable() {
            let content = ".Ad addr1 Db addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("Db".to_string()),
                    Element::Text("addr2".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn db_not_parsed() {
            let content = ".Db Ad";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Db,
                nodes: vec![]
            })];
            
            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn db_not_args() {
            assert!(MdocParser::parse_mdoc(".Db").is_err());
        }

        #[test]
        fn dd() {
            let content = ".Dd $Mdocdate: July 2, 2018$";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dd { 
                    date: DdDate::MDYFormat(
                        Date { 
                            month_day: ("July".to_string(), 2), 
                            year: 2018 
                        }
                    ) 
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dd_only_date() {
            let content = ".Dd July 2, 2018";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dd { 
                    date: DdDate::MDYFormat(
                        Date { 
                            month_day: ("July".to_string(), 2), 
                            year: 2018 
                        }
                    )  
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dd_no_date() {
            let content = ".Dd $Mdocdate$";
            let date = chrono::offset::Utc::now().date_naive();
            let month = match date.month() {
                1  => "January",
                2  => "February",
                3  => "March",
                4  => "April",
                5  => "May",
                6  => "June",
                7  => "July",
                8  => "August",
                9  => "September",
                10 => "October",
                11 => "November",
                12 => "December",
                _  => unreachable!() 
            };
            let date = DdDate::MDYFormat(Date { 
                month_day: (month.to_string(), date.day() as u8), 
                year: date.year() as u16 
            });
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dd { 
                    date: date 
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dd_numeric_date() {
            let content = ".Dd 2018-12-31";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dd { 
                    date: DdDate::MDYFormat(Date { 
                        month_day: ("December".to_string(), 31), 
                        year: 2018 
                    })
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dd_not_args() {
            let content = ".Dd";
            let date = chrono::offset::Utc::now().date_naive();
            let month = match date.month() {
                1  => "January",
                2  => "February",
                3  => "March",
                4  => "April",
                5  => "May",
                6  => "June",
                7  => "July",
                8  => "August",
                9  => "September",
                10 => "October",
                11 => "November",
                12 => "December",
                _  => unreachable!() 
            };
            let date = DdDate::MDYFormat(Date { 
                month_day: (month.to_string(), date.day() as u8), 
                year: date.year() as u16 
            });
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dd { 
                    date: date 
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dd_not_callable() {
            let content = ".Ad addr1 Dd addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("Dd".to_string()),
                    Element::Text("addr2".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dd_not_parsed() {
            let content = ".Dd Ad 2, 2018";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dd { 
                    date: DdDate::MDYFormat(Date { 
                        month_day: ("Ad".to_string(), 2), 
                        year: 2018 
                    })
                },
                nodes: vec![]
            })];
            
            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dt() {
            let content = ".Dt PROGNAME 1 i386";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dt { 
                    title: "PROGNAME".to_string(),
                    section: "General Commands".to_string(),
                    arch: Some("i386".to_string())
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dt_not_arhc() {
            let content = ".Dt PROGNAME 1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dt { 
                    title: "PROGNAME".to_string(),
                    section: "General Commands".to_string(),
                    arch: None
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dt_not_callable() {
            let content = ".Ad addr1 Dt addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("Dt".to_string()),
                    Element::Text("addr2".to_string()),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dt_not_parsed() {
            let content = ".Dt Ad 1";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dt { 
                    title: "AD".to_string(),
                    section: "General Commands".to_string(),
                    arch: None
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dt_not_args() {
            assert!(MdocParser::parse_mdoc(".Dt").is_err())
        }

        #[test]
        fn dv() {
            let content = ".Dv CONSTANT1 CONSTANT2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dv { 
                    identifiers: vec![
                        "CONSTANT1".to_string(),
                        "CONSTANT2".to_string()
                    ]
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn dv_not_args() {
            assert!(MdocParser::parse_mdoc(".Dv").is_err())
        }

        #[test]
        fn dv_callable() {
            let content = ".Ad addr1 addr2 Dv CONST1";
            let elemenets = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("addr2".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Dv {
                            identifiers: vec![
                                "CONST1".to_string()
                            ]
                        },
                        nodes: vec![]
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elemenets)
        }

        #[test]
        fn dv_parsed() {
            let content = ".Dv CONST1 Ad addr1 addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Dv { 
                    identifiers: vec![
                        "CONST1".to_string()
                    ]
                },
                nodes: vec![
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string()),
                        ],
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(content).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn em() {
            let input = ".Em word1 word2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Em,
                nodes: vec![
                    Element::Text("word1".to_string()),
                    Element::Text("word2".to_string())
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn em_not_args() {
            assert!(MdocParser::parse_mdoc(".Em").is_err());
        }

        #[test]
        fn em_parsed() {
            let input = ".Em word1 Ad addr1 addr2";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Em,
                nodes: vec![
                    Element::Text("word1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("addr1".to_string()),
                            Element::Text("addr2".to_string())
                        ]
                    })
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn em_callable() {
            let input = ".Ad addr1 addr2 Em word1";
            let elemenets = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("addr2".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Em,
                        nodes: vec![
                            Element::Text("word1".to_string())
                        ]
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elemenets)
        }

        #[test]
        fn er() {
            let input = ".Er ERROR";
            let elements = vec![Element::Macro(MacroNode { 
                mdoc_macro: Macro::Er, 
                nodes: vec![
                    Element::Text("ERROR".to_string())
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn er_not_args() {
            assert!(MdocParser::parse_mdoc(".Er").is_err());
        }

        #[test]
        fn er_parsed() {
            let input = ".Er ERROR Ad addr1";
            let elements = vec![Element::Macro(MacroNode { 
                mdoc_macro: Macro::Er, 
                nodes: vec![
                    Element::Text("ERROR".to_string()),
                    Element::Macro(MacroNode { 
                        mdoc_macro: Macro::Ad, 
                        nodes: vec![
                            Element::Text("addr1".to_string())
                        ] 
                    })
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn er_callable() {
            let input = ".Ad addr1 addr2 Er ERROR";
            let elemenets = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("addr2".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Er,
                        nodes: vec![
                            Element::Text("ERROR".to_string())
                        ]
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elemenets)
        }

        // .Es -----------------------------------------------------------

        #[test]
        fn es() {
            let input = ".Es ( )";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Es {
                    opening_delimiter: Some('('),
                    closing_delimiter: Some(')')
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn es_bad_args() {
            assert!(MdocParser::parse_mdoc(".Es").is_err());
            assert!(MdocParser::parse_mdoc(".Es (").is_err());
            assert!(MdocParser::parse_mdoc(".Es { }").is_err());
            assert!(MdocParser::parse_mdoc(".Es ( ) (").is_err());
        }

        #[test]
        fn es_parsed() {
            let input = ".Es [ At 2.32";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Es {
                    opening_delimiter: Some('['),
                    closing_delimiter: None
                },
                nodes: vec![
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::At(AtType::General),
                        nodes: vec![
                            Element::Text("2.32".to_string())
                        ]
                    }),
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn es_callable() {
            let input = ".Ad addr1 addr2 Es ( )";
            let elemenets = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Text("addr2".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Es {
                            opening_delimiter: Some('('),
                            closing_delimiter: Some(')')
                        },
                        nodes: vec![]
                    }),
                ],
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elemenets)
        }

        // .Ev -----------------------------------------------------------
        #[test]
        fn ev() {
            let input = ".Ev DISPLAY";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ev,
                nodes: vec![
                    Element::Text("DISPLAY".to_string())
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ev_not_args() {
            assert!(MdocParser::parse_mdoc(".Ev").is_err());
        }

        #[test]
        fn ev_parsed() {
            let input = ".Ev DISPLAY Ad ADDRESS";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ev,
                nodes: vec![
                    Element::Text("DISPLAY".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("ADDRESS".to_string())
                        ]
                    })
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ev_callable() {
            let input = ".Ad addr1 Ev ADDRESS";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("addr1".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ev,
                        nodes: vec![
                            Element::Text("ADDRESS".to_string())                            
                        ]
                    })
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        // .Ex -----------------------------------------------------------
        #[test]
        fn ex() {
            let input = ".Ex -std grep";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ex {
                    utilities: vec![
                        "grep".to_string()
                    ]
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ex_not_args() {
            assert!(MdocParser::parse_mdoc(".Ex").is_err());
        }

        #[test]
        fn ex_not_parsed() {
            let input = ".Ex -std grep Ad addr";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ex {
                    utilities: vec![
                        "grep".to_string(),
                        "Ad".to_string(),
                        "addr".to_string()
                    ]
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ex_not_callable() {
            let input = ".Ad addr Ex -std grep";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad, 
                nodes: vec![
                    Element::Text("addr".to_string()),
                    Element::Text("Ex".to_string()),
                    Element::Text("-std".to_string()),
                    Element::Text("grep".to_string()),
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        // .Fa -----------------------------------------------------------
        #[test]
        fn fa() {
            let input = ".Fa \"const char *p\"\n.Fa \"int a\" \"int b\" \"int c\"\n.Fa \"char *\" size_t";
            let elements = vec![
                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Fa,
                    nodes: vec![
                        Element::Text("\"const char *p\"".to_string()),
                    ]
                }),
                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Fa,
                    nodes: vec![
                        Element::Text("\"int a\"".to_string()),
                        Element::Text("\"int b\"".to_string()),
                        Element::Text("\"int c\"".to_string()),
                    ]
                }),
                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Fa,
                    nodes: vec![
                        Element::Text("\"char *\"".to_string()),
                        Element::Text("size_t".to_string()),
                    ]
                }),
            ];
            
            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn fa_not_args() {
            assert!(MdocParser::parse_mdoc(".Fa").is_err())
        }

        #[test]
        fn fa_parsed() {
            let input = ".Fa Ft const char *";
            let elemets = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Fa,
                nodes: vec![
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ft,
                        nodes: vec![
                            Element::Text("const".to_string()),
                            Element::Text("char".to_string()),
                            Element::Text("*".to_string()),
                        ]
                    })
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elemets);
        }

        #[test]
        fn fa_callable() {
            let input = ".Ft Fa \"const char *p\"";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ft,
                nodes: vec![
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Fa,
                        nodes: vec![
                            Element::Text("\"const char *p\"".to_string())
                        ]
                    })
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        // .Fd -----------------------------------------------------------
        #[test]
        fn fd() {
            let input = ".Fd #define sa_handler __sigaction_u.__sa_handler\n.Fd #define SIO_MAXNFDS\n.Fd #endif";
            let elements = vec![
                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Fd {
                        directive: "#define".to_string(),
                        arguments: vec![
                            "sa_handler".to_string(),
                            "__sigaction_u.__sa_handler".to_string()
                        ]
                    },
                    nodes: vec![]
                }),
                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Fd {
                        directive: "#define".to_string(),
                        arguments: vec![
                            "SIO_MAXNFDS".to_string(),
                        ]
                    },
                    nodes: vec![]
                }),
                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Fd {
                        directive: "#endif".to_string(),
                        arguments: vec![]
                    },
                    nodes: vec![]
                }),
            ];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn fd_not_args() {
            assert!(MdocParser::parse_mdoc(".Fd").is_err());
        }

        #[test]
        fn fd_not_parsed() {
            let input = ".Fd #define Ad addr";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Fd {
                    directive: "#define".to_string(),
                    arguments: vec![
                        "Ad".to_string(),
                        "addr".to_string()
                    ]
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn fd_not_callable() {
            let input = ".Ad Fd #define ADDRESS";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("Fd".to_string()),
                    Element::Text("#define".to_string()),
                    Element::Text("ADDRESS".to_string()),
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        // .Fl -----------------------------------------------------------
        #[test]
        fn fl() {
            let input = ".Fl H | L | P\n.Fl inet";
            let elements = vec![
                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Fl,
                    nodes: vec![
                        Element::Text("H".to_string()),
                        Element::Text("|".to_string()),
                        Element::Text("L".to_string()),
                        Element::Text("|".to_string()),
                        Element::Text("P".to_string()),
                    ]
                }),
                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Fl,
                    nodes: vec![
                        Element::Text("inet".to_string())
                    ]
                })
            ];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn fl_not_args() {
            let input = ".Fl";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Fl,
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn fl_parsed() {
            let input = ".Fl inet Ar destination gateway";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Fl,
                nodes: vec![
                    Element::Text("inet".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ar,
                        nodes: vec![
                            Element::Text("destination".to_string()),
                            Element::Text("gateway".to_string()),
                        ]
                    })
                ]
            })];
            
            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn fl_callable() {
            let input = ".Cm add Fl inet";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Cm,
                nodes: vec![
                    Element::Text("add".to_string()),
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Fl,
                        nodes: vec![
                            Element::Text("inet".to_string())
                        ]
                    })
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }
        
        // .Fn -----------------------------------------------------------
        #[test]
        #[allow(non_snake_case)]
        fn Fn() {
            let input = ".Fn \"int funcname\" \"int arg0\" \"int arg1\"\n.Fn funcname \"int arg0\"\n.Fn funcname arg0";
            let elements = vec![
                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Fn {
                        funcname: None
                    },
                    nodes: vec![
                        Element::Text("\"int funcname\"".to_string()),
                        Element::Text("\"int arg0\"".to_string()),
                        Element::Text("\"int arg1\"".to_string()),
                    ]
                }),
                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Fn {
                        funcname: Some("funcname".to_string())
                    },
                    nodes: vec![
                        Element::Text("\"int arg0\"".to_string()),
                    ]
                }),
                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Fn {
                        funcname: Some("funcname".to_string())
                    },
                    nodes: vec![
                        Element::Text("arg0".to_string()),
                    ]
                }),
            ];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        #[allow(non_snake_case)]
        fn Fn_not_args() {
            assert!(MdocParser::parse_mdoc(".Fn").is_err());
        }

        #[test]
        #[allow(non_snake_case)]
        fn Fn_parsed() {
            let input = ".Fn \"int funcname\" \"int arg0\" Ft int";
            let elements = vec![
                Element::Macro(MacroNode {
                    mdoc_macro: Macro::Fn {
                        funcname: None
                    },
                    nodes: vec![
                        Element::Text("\"int funcname\"".to_string()),
                        Element::Text("\"int arg0\"".to_string()),
                        Element::Macro(MacroNode {
                            mdoc_macro: Macro::Ft,
                            nodes: vec![
                                Element::Text("int".to_string()),
                            ]
                        })
                    ]
                }),
            ];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        #[allow(non_snake_case)]
        fn Fn_callable() {
            let input = ".Ft Fn \"const char *p\"";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ft,
                nodes: vec![
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Fn {
                            funcname: None
                        },
                        nodes: vec![
                            Element::Text("\"const char *p\"".to_string())
                        ]
                    })
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        // .Fr -----------------------------------------------------------
        #[test]
        fn fr() {
            let input = ".Fr 32";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Fr {
                    num: Some(32)
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn fr_not_args() {
            assert!(MdocParser::parse_mdoc(".Fr").is_err());
        }

        #[test]
        fn fr_parsed() {
            let input = ".Fr Ad 32";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Fr {
                    num: None
                },
                nodes: vec![
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ad,
                        nodes: vec![
                            Element::Text("32".to_string())
                        ]
                    })
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn fr_callable() {
            let input = ".Ft Fr 12";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ft,
                nodes: vec![
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Fr { 
                            num: Some(12) 
                        },
                        nodes: vec![]
                    })
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }
        
        // .Ft -----------------------------------------------------------
        #[test]
        fn ft() {
            let input = ".Ft int32 void";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ft,
                nodes: vec![
                    Element::Text("int32".to_string()),
                    Element::Text("void".to_string())
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ft_not_args() {
            assert!(MdocParser::parse_mdoc(".Ft").is_err());
        }

        #[test]
        fn ft_parsed() {
            let input = ".Ft Fa \"const char *p\"";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ft,
                nodes: vec![
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Fa,
                        nodes: vec![
                            Element::Text("\"const char *p\"".to_string())
                        ]
                    })
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ft_callable() {
            let input = ".Ad Ft void*";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ft,
                        nodes: vec![
                            Element::Text("void*".to_string())
                        ]
                    })
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        // Hf ----------------------------------------------

        #[test]
        fn hf() {
            let input = ".Hf file/path skip/argument";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Hf { 
                    file_name: Some("file/path".to_string()) 
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn hf_not_args() {
            let input = ".Hf";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Hf { 
                    file_name: None
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn hf_not_parsed() {
            let input = ".Hf Ad addr";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Hf { 
                    file_name: Some("Ad".to_string())
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn hf_not_callable() {
            let input = ".Ad Hf path/to/some/file";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Text("Hf".to_string()),
                    Element::Text("path/to/some/file".to_string()),
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ic() {
            let input = ".Ic :wq";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ic {
                    keyword: ":wq".to_string()
                },
                nodes: vec![]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ic_not_args() {
            assert!(MdocParser::parse_mdoc(".Ic").is_err());
        }

        #[test]
        fn ic_parsed() {
            let input = ".Ic lookup Cm file bind";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ic {
                    keyword: "lookup".to_string()
                },
                nodes: vec![
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Cm,
                        nodes: vec![
                            Element::Text("file".to_string()),
                            Element::Text("bind".to_string())
                        ]
                    })
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }

        #[test]
        fn ic_callable() {
            let input = ".Ad Ic :wq";
            let elements = vec![Element::Macro(MacroNode {
                mdoc_macro: Macro::Ad,
                nodes: vec![
                    Element::Macro(MacroNode {
                        mdoc_macro: Macro::Ic {
                            keyword: ":wq".to_string()
                        },
                        nodes: vec![]
                    })
                ]
            })];

            let mdoc = MdocParser::parse_mdoc(input).unwrap();
            assert_eq!(mdoc.elements, elements);
        }
    }
}
