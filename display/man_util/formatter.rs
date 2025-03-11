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
            Macro::Rs => parse_rs_block(macro_node),
            _ => unimplemented!()   
        }
    }
}

// Formatting Rs-Re bloock. Can contain only %* macros
impl MdocFormatter {
    pub fn parse_rs_block(macro_node: MacroNode) -> String {
        unimplemented!()
    }
}