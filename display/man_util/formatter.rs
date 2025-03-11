use std::collections::HashMap;
use libc::EOF;
use super::{mdoc_macro::Macro, parser::{Element, MacroNode, MdocDocument}};

#[derive(Debug)]
pub struct MdocFormatter {
    pub formatted_mdoc: String
}

// Base formatting functions.
impl MdocFormatter {
    pub fn format_mdoc(&mut self, ast: MdocDocument) -> String {
        for node in ast.iter() {
            let formatted_node = format_node(node);
            self.formatted_mdoc.push_str(&formatted_node);
        }

        self.formatted_mdoc
    }

    pub fn format_node(&self, node: Element) -> String {
        match node {
            Element::Macro(macro_node) => format_macro_node(node),
            Element::Text(line) => line,
            Element::Eoi => EOF
        }
    }

    pub fn format_macro_node(macro_node: MacroNode) -> String {
        match macro_node.mdoc_macro {
            Macro::Rs => format_rs_block(macro_node),
            _ => unimplemented!()   
        }
    }
}

// Formatting Rs-Re bloock. Can contain only %* macros
// TODO:
//  - RsMacro instead of MacroNode.
// Notes:
//  - All macros are comma separated.
//  - Before the last '%A' macro has to be 'and' word. 
//  - These macros have order!
impl MdocFormatter {
    pub fn format_rs_block(macro_node: MacroNode) -> String {
        unimplemented!()
    }

    pub fn format_d(month_day: Option<String>, year: i32) -> String {
        match month_day {
            Some(md) => format!("{md} {year}"),
            None => format!("{year}")
        }
    }

    pub fn format_p(macro_node: MacroNode) -> String {
        macro_node.nodes
            .iter()
            .map(|el| el.as_str().to_string())
            .collect::<String>()
    }
}