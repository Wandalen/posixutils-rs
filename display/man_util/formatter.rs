use std::collections::HashMap;
use aho_corasick::AhoCorasick;
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
            Element::Macro(macro_node) => Self::format_macro_node(node),
            Element::Text(text) => Self::format_text_node(text.as_str()),
            Element::Eoi => EOF
        }
    }

    pub fn format_macro_node(macro_node: MacroNode) -> String {
        match macro_node.mdoc_macro {
            Macro::Rs => format_rs_block(macro_node),
            _ => unimplemented!()   
        }
    }

    // TODO: Add all cases
    pub fn format_text_node(text: &str) -> String {
        let patterns = vec![
            r"\(ba", r"\(br", r"\(rs",
            r"\(ul", r"\(ru", r"\(rn",
            r"\(bb", r"\(sl", 
        ];

        let replacements: HashMap<&str, &str> = [
            (r"\(ba", r"|"),
            (r"\(br", r"│"),
            (r"\(ul", r"_"),
            (r"\(ru", r"_"),
            (r"\(rn", r"‾"),
            (r"\(bb", r"¦"),
            (r"\(sl", r"/"),
            (r"\(rs", r"\"),
        ].iter().cloned().collect();

        let ac = AhoCorasick::new(&patterns)
            .expect("Build error");

        ac.replace_all(text, |mat| {
            let pat = patterns[mat.pattern()];
            replacements.get(pat).unwrap_or(&"")
        })
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