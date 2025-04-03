use crate::FormattingSettings;
use aho_corasick::AhoCorasick;
use std::{collections::HashMap, ops::Index};
use terminfo::Database;

use super::{
    mdoc_macro::{text_production::*, types::*, Macro},
    parser::{Element, MacroNode, MdocDocument},
};

static REGEX_UNICODE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
    regex::Regex::new(
        r"(?x)
        (?:
            (?P<unicode_bracket>\\\[u(?P<hex1>[0-9A-F]{4,6})\])      |
            (?P<unicode_c>\\C'u(?P<hex2>[0-9A-F]{4,6})')             |
            (?P<number_n>\\N'(?P<dec1>[0-9]+)')                      |
            (?P<number_char>\\\[char(?P<dec2>[0-9]+)\])
        )
    ",
    )
    .unwrap()
});

#[derive(Debug)]
pub struct FormattingState {
    first_name: Option<String>,
    suppress_space: bool,
    header_text: Option<String>,
    footer_text: Option<String>,
    spacing: String,
    date: String,
    split_mod: bool,
    current_indent: usize,
}

impl Default for FormattingState {
    fn default() -> Self {
        Self {
            first_name: None,
            suppress_space: false,
            header_text: None,
            footer_text: None,
            spacing: " ".to_string(),
            date: String::default(),
            split_mod: false,
            current_indent: 0,
        }
    }
}

#[derive(Debug)]
pub struct MdocFormatter {
    formatting_settings: FormattingSettings,
    formatting_state: FormattingState
}

// Helper funcitons.
impl MdocFormatter {
    pub fn new(settings: FormattingSettings) -> Self {
        Self {
            formatting_settings: settings,
            formatting_state: FormattingState::default(),
        }
    }

    fn supports_italic(&self) -> bool {
        if let Ok(info) = Database::from_env() {
            return info.raw("sitm").is_some();
        }
        false
    }

    fn supports_bold(&self) -> bool {
        if let Ok(info) = Database::from_env() {
            return info.raw("bold").is_some();
        }
        false
    }

    fn supports_underline(&self) -> bool {
        if let Ok(info) = Database::from_env() {
            return info.raw("smul").is_some();
        }
        false
    }

    fn replace_unicode_escapes(&self, text: &str) -> String {
        REGEX_UNICODE
            .replace_all(text, |caps: &regex::Captures| {
                if let Some(hex) = caps
                    .name("hex1")
                    .or_else(|| caps.name("hex2"))
                    .map(|m| m.as_str())
                {
                    if let Ok(codepoint) = u32::from_str_radix(hex, 16) {
                        if codepoint < 0x80 {
                            return "\u{FFFD}".to_string();
                        }
                        if codepoint < 0x10FFFF && !(0xD800..=0xDFFF).contains(&codepoint) {
                            if let Some(ch) = char::from_u32(codepoint) {
                                return ch.to_string();
                            }
                        }
                    }
                } else if let Some(dec) = caps
                    .name("dec1")
                    .or_else(|| caps.name("dec2"))
                    .map(|m| m.as_str())
                {
                    if let Ok(codepoint) = dec.parse::<u32>() {
                        if let Some(ch) = char::from_u32(codepoint) {
                            return ch.to_string();
                        }
                    }
                }
                caps.get(0).unwrap().as_str().to_string()
            })
            .to_string()
    }
}

// Base formatting functions.
impl MdocFormatter {
    fn append_formatted_text( 
        &self, 
        formatted: &str,
        // current_line: &mut String, 
        lines: &mut Vec<String>
    ) {
        let get_indent = |l: &str| {
            l.chars()
                .take_while(|ch|ch.is_whitespace())
                .collect::<String>()
        };

        let is_one_line = formatted.lines().count() == 1;
        let max_width = self.formatting_settings.width;

        for line in formatted.split("\n"){
            if !is_one_line && !current_line.is_empty(){
                lines.push(current_line.clone());
                current_line.clear();
            }

            if current_line.chars().count() + line.chars().count() > max_width || is_one_line {
                let indent = get_indent(line);

                let max_width = max_width.saturating_sub(indent.len());                    

                for word in line.split_whitespace() {
                    if current_line.chars().count() + word.chars().count() + 1 >= max_width {
                        lines.push(indent.clone() + current_line.trim());
                        current_line.clear(); 
                    }
                    current_line.push_str(word);
                    if !word.chars().all(|ch|ch.is_control()){
                        current_line.push(' ');
                    }
                }
                let is_not_empty = !(current_line.chars().all(|ch| ch.is_whitespace()) || 
                    current_line.is_empty());
                if is_not_empty{
                    *current_line = indent + current_line;
                }
            } else{
                lines.push(line.to_string());
            }
        }
    }

    pub fn format_synopsis_section(&mut self, ast: MdocDocument) -> Vec<u8> {
        let mut lines = Vec::new();
        let mut current_line = String::new();

        for node in ast.elements {
            let formatted_node = match node {
                Element::Macro(macro_node) => {
                    if let Macro::Sh { ref title } = macro_node.mdoc_macro {
                        if title.eq_ignore_ascii_case("SYNOPSIS") {
                            self.format_sh_block(title.clone(), macro_node)
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                _ => continue,
            };

            self.append_formatted_text(&formatted_node, &mut lines);
        }

        if !current_line.is_empty() {
            lines.push(current_line.trim_end().to_string());
        }

        lines.join("\n").into_bytes()
    }

    pub fn format_mdoc(&mut self, ast: MdocDocument) -> Vec<u8> {
        //println!("{:?}", ast);

        let mut lines = Vec::new();
        let mut current_line = String::new();

        for node in ast.elements {
            let formatted_node: String = self.format_node(node);            
            if formatted_node.is_empty(){
                continue;
            } 
            
            // self.append_formatted_text(&formatted_node, &mut current_line, &mut lines);
            self.append_formatted_text(&formatted_node, &mut lines);

        }

        if !current_line.is_empty() {
            lines.push(current_line.trim_end().to_string());
        }

        let first_empty_count= lines.iter()
            .take_while(|l|{
                l.chars().all(|ch|ch.is_whitespace())
            })
            .count();

        lines = lines.split_at(first_empty_count).1.to_vec();
        
        lines.insert(0, "".to_string());

        lines.insert(
        0,
        self.formatting_state
            .header_text
            .clone()
            .unwrap_or_else(|| self.format_default_header()),
        );
        lines.push(self.format_footer());

        lines
            .join("\n")
            .replace(" \x08", "")
            .into_bytes()
    }

    fn format_default_header(&mut self) -> String {
        self.format_dt(None, "", None);
        self.formatting_state.header_text.clone().unwrap_or_default()
    }

    fn get_default_footer_text() -> String {
        String::new()
    }

    fn format_footer(&mut self) -> String {
        let footer_text = self
            .formatting_state
            .footer_text
            .clone()
            .unwrap_or(Self::get_default_footer_text());

        if self.formatting_state.date.is_empty() {
            self.formatting_state.date = self.format_dd("$Mdocdate$");
        }

        let mut space_size = self
            .formatting_settings
            .width
            .saturating_sub(2 * footer_text.len() + self.formatting_state.date.len())
            / 2;

        let mut left_footer_text = footer_text.clone();
        let mut right_footer_text = footer_text.clone();

        if space_size <= 1 {
            space_size = self
                .formatting_settings
                .width
                .saturating_sub(self.formatting_state.date.len())
                / 2;

            let space = vec![
                " ";
                self.formatting_settings
                    .width
                    .saturating_sub(footer_text.len())
            ]
            .into_iter()
            .collect::<String>();

            left_footer_text = footer_text.clone() + &space.clone() + "\n";
            right_footer_text = "\n".to_string() + &space.clone() + &footer_text.clone();
        }

        let space = " ".repeat(space_size);

        let mut content = format!(
            "\n{}{}{}{}{}",
            left_footer_text,
            space.clone(),
            self.formatting_state.date,
            space,
            right_footer_text
        );

        let missing_space = self
            .formatting_settings
            .width
            .saturating_sub(content.len() - 1);

        content.insert_str(
            left_footer_text.len() + 1,
            &" ".repeat(missing_space),
        );

        content
    }

    fn format_node(&mut self, node: Element) -> String {
        match node {
            Element::Macro(macro_node) => self.format_macro_node(macro_node),
            Element::Text(text) => self.format_text_node(text.as_str()),
            Element::Eoi => "".to_string(),
        }
    }

    fn format_macro_node(&mut self, macro_node: MacroNode) -> String {
        match macro_node.clone().mdoc_macro {
            // Block full-explicit
            Macro::Bd {
                block_type,
                offset,
                compact,
            } => self.format_bd_block(block_type, offset, compact, macro_node),
            Macro::Bf(bf_type) => self.format_bf_block(bf_type, macro_node),
            Macro::Bk => self.format_bk_block(macro_node),
            Macro::Bl { .. } => self.format_bl_blocks(macro_node),

            // Special block macro ta formatting
            Macro::Ta => self.format_ta(),

            // Block full-implicit
            Macro::It { head} => self.format_it_block(head, macro_node),
            Macro::Nd => self.format_nd(macro_node),
            Macro::Nm => self.format_nm(macro_node),
            Macro::Sh { title } => self.format_sh_block(title, macro_node),
            Macro::Ss { title } => self.format_ss_block(title, macro_node),

            // Block partial-explicit.
            Macro::Ao => self.format_a_block(macro_node),
            Macro::Bo => self.format_b_block(macro_node),
            Macro::Bro => self.format_br_block(macro_node),
            Macro::Do => self.format_d_block(macro_node),
            Macro::Oo => self.format_o_block(macro_node),
            Macro::Po => self.format_p_block(macro_node),
            Macro::Qo => self.format_q_block(macro_node),
            Macro::Rs => self.format_rs_block(macro_node),
            Macro::So => self.format_s_block(macro_node),
            Macro::Xo => self.format_x_block(macro_node),
            Macro::Eo {
                opening_delimiter,
                closing_delimiter,
            } => self.format_e_block(opening_delimiter, closing_delimiter, macro_node),
            Macro::Fo { ref funcname } => {
                let funcname_copy = funcname.clone();
                self.format_f_block(funcname_copy, macro_node)
            }

            // Block partial-implicit.
            Macro::Aq => self.format_aq(macro_node),
            Macro::Bq => self.format_bq(macro_node),
            Macro::Brq => self.format_brq(macro_node),
            Macro::D1 => self.format_d1(macro_node),
            Macro::Dl => self.format_dl(macro_node),
            Macro::Dq => self.format_dq(macro_node),
            Macro::En => self.format_en(macro_node),
            Macro::Op => self.format_op(macro_node),
            Macro::Pq => self.format_pq(macro_node),
            Macro::Ql => self.format_ql(macro_node),
            Macro::Qq => self.format_qq(macro_node),
            Macro::Sq => self.format_sq(macro_node),
            Macro::Vt => self.format_vt(macro_node),

            // In-line.
            // Rs block macros which can appears outside Rs-Re block.
            Macro::B => self.format_b(macro_node),
            Macro::T => self.format_t(macro_node),
            Macro::U => self.format_u(macro_node),

            // Text production macros.
            Macro::At => self.format_at(macro_node),
            Macro::Bsx => self.format_bsx(macro_node),
            Macro::Bx => self.format_bx(macro_node),
            Macro::Dx => self.format_dx(macro_node),
            Macro::Ad => self.format_ad(macro_node),
            Macro::Ap => self.format_ap(macro_node),
            Macro::Ar => self.format_ar(macro_node),
            Macro::Bt => self.format_bt(),
            Macro::Cd => self.format_cd(macro_node),
            Macro::Cm => self.format_cm(macro_node),
            Macro::Db => self.format_db(),
            Macro::Dv => self.format_dv(macro_node),
            Macro::Em => self.format_em(macro_node),
            Macro::An { author_name_type } => self.format_an(author_name_type, macro_node),
            Macro::Dd => {
                match macro_node.nodes.is_empty() {
                    true => self.formatting_state.date = self.format_dd(""),
                    false => {
                        match &macro_node.nodes[0] {
                            Element::Text(l) => self.formatting_state.date = self.format_dd(l.as_str()),
                            _ => unreachable!()
                        }
                    }
                };

                String::new()
            },
            Macro::Dt {
                title,
                section,
                arch,
            } => self.format_dt(title.clone(), section.as_str(), arch.clone()),

            Macro::Er => self.format_er(macro_node),
            Macro::Es {
                opening_delimiter,
                closing_delimiter,
            } => self.format_es(opening_delimiter, closing_delimiter, macro_node),
            Macro::Ev => self.format_ev(macro_node),
            Macro::Ex => self.format_ex(macro_node),
            Macro::Fa => self.format_fa(macro_node),
            Macro::Fd {
                directive,
                arguments,
            } => self.format_fd(directive.as_str(), &arguments),
            Macro::Fl => self.format_fl(macro_node),
            Macro::Fn { funcname } => self.format_fn(funcname.as_str(), macro_node),
            Macro::Fr => self.format_fr(macro_node),
            Macro::Ft => self.format_ft(macro_node),
            Macro::Fx => self.format_fx(macro_node),
            Macro::Hf => self.format_hf(macro_node),
            Macro::Ic => self.format_ic(macro_node),
            Macro::In { filename } => self.format_in(filename.as_str(), macro_node),
            Macro::Lb { lib_name } => self.format_lb(lib_name.as_str(), macro_node),
            Macro::Li => self.format_li(macro_node),
            Macro::Lk { ref uri } => self.format_lk(uri.as_str(), macro_node),
            Macro::Lp => self.format_lp(),
            Macro::Ms => self.format_ms(macro_node),
            Macro::Mt => self.format_mt(macro_node),
            Macro::No => self.format_no(macro_node),
            Macro::Ns => self.format_ns(macro_node),
            Macro::Nx => self.format_nx(macro_node),
            Macro::Os => self.format_os(macro_node),
            Macro::Ox => self.format_ox(macro_node),
            Macro::Pa => self.format_pa(macro_node),
            Macro::Pf { prefix } => self.format_pf(prefix.as_str(), macro_node),
            Macro::Pp => self.format_pp(macro_node),
            Macro::Rv => self.format_rv(macro_node),
            Macro::Sm(sm_mode) => self.format_sm(sm_mode, macro_node),
            Macro::St(st_type) => self.format_st(st_type, macro_node),
            Macro::Sx => self.format_sx(macro_node),
            Macro::Sy => self.format_sy(macro_node),
            Macro::Tg { term } => self.format_tg(term),
            Macro::Tn => self.format_tn(macro_node),
            Macro::Ud => self.format_ud(),
            Macro::Ux => self.format_ux(macro_node),
            Macro::Va => self.format_va(macro_node),
            Macro::Xr { name, section } => self.format_xr(
                name.as_str(), 
                section.as_str(), 
                macro_node
            ),

            _ => String::new(),
        }
    }

    fn format_text_node(&self, text: &str) -> String {
        let replacements: HashMap<&str, &str> = [
            // Spaces:
            (r"\ ", " "), // unpaddable space
            (r"\~", " "), // paddable space
            (r"\0", " "), // digit-width space
            (r"\|", " "), // one-sixth \(em narrow space
            (r"\^", " "), // one-twelfth \(em half-narrow space
            (r"\&", ""),  // zero-width space
            (r"\)", ""),  // zero-width space (transparent to end-of-sentence detection)
            (r"\%", ""),  // zero-width space allowing hyphenation
            (r"\:", ""),  // zero-width space allowing line break
            // Lines:
            (r"\(ba", r"|"), // bar
            (r"\(br", r"│"), // box rule
            (r"\(ul", r"_"), // underscore
            (r"\(ru", r"_"), // underscore (width 0.5m)
            (r"\(rn", r"‾"), // overline
            (r"\(bb", r"¦"), // broken bar
            (r"\(sl", r"/"), // forward slash
            (r"\(rs", r"\"), // backward slash
            // Text markers:
            (r"\(ci", r"○"), // circle
            (r"\(bu", r"•"), // bullet
            (r"\(dd", r"‡"), // double dagger
            (r"\(dg", r"†"), // dagger
            (r"\(lz", r"◊"), // lozenge
            (r"\(sq", r"□"), // white square
            (r"\(ps", r"¶"), // paragraph
            (r"\(sc", r"§"), // section
            (r"\(lh", r"☜"), // left hand
            (r"\(rh", r"☞"), // right hand
            (r"\(at", r"@"), // at
            (r"\(sh", r"#"), // hash (pound)
            (r"\(CR", r"↵"), // carriage return
            (r"\(OK", r"✓"), // check mark
            (r"\(CL", r"♣"), // club suit
            (r"\(SP", r"♠"), // spade suit
            (r"\(HE", r"♥"), // heart suit
            (r"\(DI", r"♦"), // diamond suit
            // Legal symbols:
            (r"\(co", r"©"), // copyright
            (r"\(rg", r"®"), // registered
            (r"\(tm", r"™"), // trademarked
            // Punctuation:
            (r"\(em", r"—"), // em-dash
            (r"\(en", r"–"), // en-dash
            (r"\(hy", r"‐"), // hyphen
            (r"\e", r"\\"),  // back-slash
            (r"\(r!", r"¡"), // upside-down exclamation
            (r"\(r?", r"¿"), // upside-down question
            // Quotes:
            (r"\(Bq", r"„"), // right low double-quote
            (r"\(bq", r"‚"), // right low single-quote
            (r"\(lq", r"“"), // left double-quote
            (r"\(rq", r"”"), // right double-quote
            (r"\(oq", r"‘"), // left single-quote
            (r"\(cq", r"’"), // right single-quote
            (r"\(aq", r"'"), // apostrophe quote (ASCII character)
            (r"\(dq", "\""), // double quote (ASCII character)
            (r"\(Fo", r"«"), // left guillemet
            (r"\(Fc", r"»"), // right guillemet
            (r"\(fo", r"‹"), // left single guillemet
            (r"\(fc", r"›"), // right single guillemet
            // Brackets:
            (r"\(lB", r"["),              // left bracket
            (r"\(rB", r"]"),              // right bracket
            (r"\(lC", r"{"),              // left brace
            (r"\(rC", r"}"),              // right brace
            (r"\(la", r"⟨"),              // left angle
            (r"\(ra", r"⟩"),              // right angle
            (r"\(bv", r"⎪"),              // brace extension (special font)
            (r"\[braceex]", r"⎪"),        // brace extension
            (r"\[bracketlefttp]", r"⎡"),  // top-left hooked bracket
            (r"\[bracketleftbt]", r"⎣"),  // bottom-left hooked bracket
            (r"\[bracketleftex]", r"⎢"),  // left hooked bracket extension
            (r"\[bracketrighttp]", r"⎤"), // top-right hooked bracket
            (r"\[bracketrightbt]", r"⎦"), // bottom-right hooked bracket
            (r"\[bracketrightex]", r"⎥"), // right hooked bracket extension
            (r"\(lt", r"⎧"),              // top-left hooked brace
            (r"\[bracelefttp]", r"⎧"),    // top-left hooked brace
            (r"\(lk", r"⎨"),              // mid-left hooked brace
            (r"\[braceleftmid]", r"⎨"),   // mid-left hooked brace
            (r"\(lb", r"⎩"),              // bottom-left hooked brace
            (r"\[braceleftbt]", r"⎩"),    // bottom-left hooked brace
            (r"\[braceleftex]", r"⎪"),    // left hooked brace extension
            (r"\(rt", r"⎫"),              // top-right hooked brace
            (r"\[bracerighttp]", r"⎫"),   // top-right hooked brace
            (r"\(rk", r"⎬"),              // mid-right hooked brace
            (r"\[bracerightmid]", r"⎬"),  // mid-right hooked brace
            (r"\(rb", r"⎭"),              // bottom-right hooked brace
            (r"\[bracerightbt]", r"⎭"),   // bottom-right hooked brace
            (r"\[bracerightex]", r"⎪"),   // right hooked brace extension
            (r"\[parenlefttp]", r"⎛"),    // top-left hooked parenthesis
            (r"\[parenleftbt]", r"⎝"),    // bottom-left hooked parenthesis
            (r"\[parenleftex]", r"⎜"),    // left hooked parenthesis extension
            (r"\[parenrighttp]", r"⎞"),   // top-right hooked parenthesis
            (r"\[parenrightbt]", r"⎠"),   // bottom-right hooked parenthesis
            (r"\[parenrightex]", r"⎟"),   // right hooked parenthesis extension
            // Arrows:
            (r"\(<-", r"←"), // left arrow
            (r"\(->", r"→"), // right arrow
            (r"\(<>", r"↔"), // left-right arrow
            (r"\(da", r"↓"), // down arrow
            (r"\(ua", r"↑"), // up arrow
            (r"\(va", r"↕"), // up-down arrow
            (r"\(lA", r"⇐"), // left double-arrow
            (r"\(rA", r"⇒"), // right double-arrow
            (r"\(hA", r"⇔"), // left-right double-arrow
            (r"\(uA", r"⇑"), // up double-arrow
            (r"\(dA", r"⇓"), // down double-arrow
            (r"\(vA", r"⇕"), // up-down double-arrow
            (r"\(an", r"⎯"), // horizontal arrow extension
            // Logical:
            (r"\(AN", r"∧"),   // logical and
            (r"\(OR", r"∨"),   // logical or
            (r"\[tno]", r"¬"), // logical not (text font)
            (r"\(no", r"¬"),   // logical not (special font)
            (r"\(te", r"∃"),   // existential quantifier
            (r"\(fa", r"∀"),   // universal quantifier
            (r"\(st", r"∋"),   // such that
            (r"\(tf", r"∴"),   // therefore
            (r"\(3d", r"∴"),   // therefore
            (r"\(or", r"|"),   // bitwise or
            // Mathematical:
            (r"\-", r"-"),           // minus (text font)
            (r"\(mi", r"−"),         // minus (special font)
            (r"\+", r"+"),           // plus (text font)
            (r"\(pl", r"+"),         // plus (special font)
            (r"\(-+", r"∓"),         // minus-plus
            (r"\[t+-]", r"±"),       // plus-minus (text font)
            (r"\(+-", r"±"),         // plus-minus (special font)
            (r"\(pc", r"·"),         // center-dot
            (r"\[tmu]", r"×"),       // multiply (text font)
            (r"\(mu", r"×"),         // multiply (special font)
            (r"\(c*", r"⊗"),         // circle-multiply
            (r"\(c+", r"⊕"),         // circle-plus
            (r"\[tdi]", r"÷"),       // divide (text font)
            (r"\(di", r"÷"),         // divide (special font)
            (r"\(f/", r"⁄"),         // fraction
            (r"\(**", r"∗"),         // asterisk
            (r"\(<=", r"≤"),         // less-than-equal
            (r"\(>=", r"≥"),         // greater-than-equal
            (r"\(<<", r"≪"),         // much less
            (r"\(>>", r"≫"),         // much greater
            (r"\(eq", r"="),         // equal
            (r"\(!=", r"≠"),         // not equal
            (r"\(==", r"≡"),         // equivalent
            (r"\(ne", r"≢"),         // not equivalent
            (r"\(ap", r"∼"),         // tilde operator
            (r"\(|=", r"≃"),         // asymptotically equal
            (r"\(=~", r"≅"),         // approximately equal
            (r"\(~~", r"≈"),         // almost equal
            (r"\(~=", r"≈"),         // almost equal
            (r"\(pt", r"∝"),         // proportionate
            (r"\(es", r"∅"),         // empty set
            (r"\(mo", r"∈"),         // element
            (r"\(nm", r"∉"),         // not element
            (r"\(sb", r"⊂"),         // proper subset
            (r"\(nb", r"⊄"),         // not subset
            (r"\(sp", r"⊃"),         // proper superset
            (r"\(nc", r"⊅"),         // not superset
            (r"\(ib", r"⊆"),         // reflexive subset
            (r"\(ip", r"⊇"),         // reflexive superset
            (r"\(ca", r"∩"),         // intersection
            (r"\(cu", r"∪"),         // union
            (r"\(/_", r"∠"),         // angle
            (r"\(pp", r"⊥"),         // perpendicular
            (r"\(is", r"∫"),         // integral
            (r"\[integral]", r"∫"),  // integral
            (r"\[sum]", r"∑"),       // summation
            (r"\[product]", r"∏"),   // product
            (r"\[coproduct]", r"∐"), // coproduct
            (r"\(gr", r"∇"),         // gradient
            (r"\(sr", r"√"),         // square root
            (r"\[sqrt]", r"√"),      // square root
            (r"\(lc", r"⌈"),         // left-ceiling
            (r"\(rc", r"⌉"),         // right-ceiling
            (r"\(lf", r"⌊"),         // left-floor
            (r"\(rf", r"⌋"),         // right-floor
            (r"\(if", r"∞"),         // infinity
            (r"\(Ah", r"ℵ"),         // aleph
            (r"\(Im", r"ℑ"),         // imaginary
            (r"\(Re", r"ℜ"),         // real
            (r"\(wp", r"℘"),         // Weierstrass p
            (r"\(pd", r"∂"),         // partial differential
            (r"\(-h", r"ℏ"),         // Planck constant over 2π
            (r"\[hbar]", r"ℏ"),      // Planck constant over 2π
            (r"\(12", r"½"),         // one-half
            (r"\(14", r"¼"),         // one-fourth
            (r"\(34", r"¾"),         // three-fourths
            (r"\(18", r"⅛"),         // one-eighth
            (r"\(38", r"⅜"),         // three-eighths
            (r"\(58", r"⅝"),         // five-eighths
            (r"\(78", r"⅞"),         // seven-eighths
            (r"\(S1", r"¹"),         // superscript 1
            (r"\(S2", r"²"),         // superscript 2
            (r"\(S3", r"³"),         // superscript 3
            // Ligatures:
            (r"\(ff", r"ﬀ"), // ff ligature
            (r"\(fi", r"ﬁ"), // fi ligature
            (r"\(fl", r"ﬂ"), // fl ligature
            (r"\(Fi", r"ﬃ"), // ffi ligature
            (r"\(Fl", r"ﬄ"), // ffl ligature
            (r"\(AE", r"Æ"), // AE
            (r"\(ae", r"æ"), // ae
            (r"\(OE", r"Œ"), // OE
            (r"\(oe", r"œ"), // oe
            (r"\(ss", r"ß"), // German eszett
            (r"\(IJ", r"Ĳ"), // IJ ligature
            (r"\(ij", r"ĳ"), // ij ligature
            // Accents:
            ("\\(a\"", r"˝"), // Hungarian umlaut
            (r"\(a-", r"¯"),  // macron
            (r"\(a.", r"˙"),  // dotted
            (r"\(a^", r"^"),  // circumflex
            (r"\(aa", r"´"),  // acute
            (r"\'", r"´"),    // acute
            (r"\(ga", r"`"),  // grave
            (r"\`", r"`"),    // grave
            (r"\(ab", r"˘"),  // breve
            (r"\(ac", r"¸"),  // cedilla
            (r"\(ad", r"¨"),  // dieresis
            (r"\(ah", r"ˇ"),  // caron
            (r"\(ao", r"˚"),  // ring
            (r"\(a~", r"~"),  // tilde
            (r"\(ho", r"˛"),  // ogonek
            (r"\(ha", r"^"),  // hat (ASCII character)
            (r"\(ti", r"~"),  // tilde (ASCII character)
            // Accented letters:
            (r"\('A", r"Á"), // acute A
            (r"\('E", r"É"), // acute E
            (r"\('I", r"Í"), // acute I
            (r"\('O", r"Ó"), // acute O
            (r"\('U", r"Ú"), // acute U
            (r"\('Y", r"Ý"), // acute Y
            (r"\('a", r"á"), // acute a
            (r"\('e", r"é"), // acute e
            (r"\('i", r"í"), // acute i
            (r"\('o", r"ó"), // acute o
            (r"\('u", r"ú"), // acute u
            (r"\('y", r"ý"), // acute y
            (r"\(`A", r"À"), // grave A
            (r"\(`E", r"È"), // grave E
            (r"\(`I", r"Ì"), // grave I
            (r"\(`O", r"Ò"), // grave O
            (r"\(`U", r"Ù"), // grave U
            (r"\(`a", r"à"), // grave a
            (r"\(`e", r"è"), // grave e
            (r"\(`i", r"ì"), // grave i
            (r"\(`o", r"ò"), // grave o
            (r"\(`u", r"ù"), // grave u
            (r"\(~A", r"Ã"), // tilde A
            (r"\(~N", r"Ñ"), // tilde N
            (r"\(~O", r"Õ"), // tilde O
            (r"\(~a", r"ã"), // tilde a
            (r"\(~n", r"ñ"), // tilde n
            (r"\(~o", r"õ"), // tilde o
            (r"\(:A", r"Ä"), // dieresis A
            (r"\(:E", r"Ë"), // dieresis E
            (r"\(:I", r"Ï"), // dieresis I
            (r"\(:O", r"Ö"), // dieresis O
            (r"\(:U", r"Ü"), // dieresis U
            (r"\(:a", r"ä"), // dieresis a
            (r"\(:e", r"ë"), // dieresis e
            (r"\(:i", r"ï"), // dieresis i
            (r"\(:o", r"ö"), // dieresis o
            (r"\(:u", r"ü"), // dieresis u
            (r"\(:y", r"ÿ"), // dieresis y
            (r"\(^A", r"Â"), // circumflex A
            (r"\(^E", r"Ê"), // circumflex E
            (r"\(^I", r"Î"), // circumflex I
            (r"\(^O", r"Ô"), // circumflex O
            (r"\(^U", r"Û"), // circumflex U
            (r"\(^a", r"â"), // circumflex a
            (r"\(^e", r"ê"), // circumflex e
            (r"\(^i", r"î"), // circumflex i
            (r"\(^o", r"ô"), // circumflex o
            (r"\(^u", r"û"), // circumflex u
            (r"\(,C", r"Ç"), // cedilla C
            (r"\(,c", r"ç"), // cedilla c
            (r"\(/L", r"Ł"), // stroke L
            (r"\(/l", r"ł"), // stroke l
            (r"\(/O", r"Ø"), // stroke O
            (r"\(/o", r"ø"), // stroke o
            (r"\(oA", r"Å"), // ring A
            (r"\(oa", r"å"), // ring a
            // Special letters:
            (r"\(-D", r"Ð"), // Eth
            (r"\(Sd", r"ð"), // eth
            (r"\(TP", r"Þ"), // Thorn
            (r"\(Tp", r"þ"), // thorn
            (r"\(.i", r"ı"), // dotless i
            (r"\(.j", r"ȷ"), // dotless j
            // Currency:
            (r"\(Do", r"$"), // dollar
            (r"\(ct", r"¢"), // cent
            (r"\(Eu", r"€"), // Euro symbol
            (r"\(eu", r"€"), // Euro symbol
            (r"\(Ye", r"¥"), // yen
            (r"\(Po", r"£"), // pound
            (r"\(Cs", r"¤"), // Scandinavian
            (r"\(Fn", r"ƒ"), // florin
            // Units:
            (r"\(de", r"°"), // degree
            (r"\(%0", r"‰"), // per-thousand
            (r"\(fm", r"′"), // minute
            (r"\(sd", r"″"), // second
            (r"\(mc", r"µ"), // micro
            (r"\(Of", r"ª"), // Spanish female ordinal
            (r"\(Om", r"º"), // Spanish masculine ordinal
            // Greek letters:
            (r"\(*A", r"Α"), // Alpha
            (r"\(*B", r"Β"), // Beta
            (r"\(*G", r"Γ"), // Gamma
            (r"\(*D", r"Δ"), // Delta
            (r"\(*E", r"Ε"), // Epsilon
            (r"\(*Z", r"Ζ"), // Zeta
            (r"\(*Y", r"Η"), // Eta
            (r"\(*H", r"Θ"), // Theta
            (r"\(*I", r"Ι"), // Iota
            (r"\(*K", r"Κ"), // Kappa
            (r"\(*L", r"Λ"), // Lambda
            (r"\(*M", r"Μ"), // Mu
            (r"\(*N", r"Ν"), // Nu
            (r"\(*C", r"Ξ"), // Xi
            (r"\(*O", r"Ο"), // Omicron
            (r"\(*P", r"Π"), // Pi
            (r"\(*R", r"Ρ"), // Rho
            (r"\(*S", r"Σ"), // Sigma
            (r"\(*T", r"Τ"), // Tau
            (r"\(*U", r"Υ"), // Upsilon
            (r"\(*F", r"Φ"), // Phi
            (r"\(*X", r"Χ"), // Chi
            (r"\(*Q", r"Ψ"), // Psi
            (r"\(*W", r"Ω"), // Omega
            (r"\(*a", r"α"), // alpha
            (r"\(*b", r"β"), // beta
            (r"\(*g", r"γ"), // gamma
            (r"\(*d", r"δ"), // delta
            (r"\(*e", r"ε"), // epsilon
            (r"\(*z", r"ζ"), // zeta
            (r"\(*y", r"η"), // eta
            (r"\(*h", r"θ"), // theta
            (r"\(*i", r"ι"), // iota
            (r"\(*k", r"κ"), // kappa
            (r"\(*l", r"λ"), // lambda
            (r"\(*m", r"μ"), // mu
            (r"\(*n", r"ν"), // nu
            (r"\(*c", r"ξ"), // xi
            (r"\(*o", r"ο"), // omicron
            (r"\(*p", r"π"), // pi
            (r"\(*r", r"ρ"), // rho
            (r"\(*s", r"σ"), // sigma
            (r"\(*t", r"τ"), // tau
            (r"\(*u", r"υ"), // upsilon
            (r"\(*f", r"ϕ"), // phi
            (r"\(*x", r"χ"), // chi
            (r"\(*q", r"ψ"), // psi
            (r"\(*w", r"ω"), // omega
            (r"\(+h", r"ϑ"), // theta variant
            (r"\(+f", r"φ"), // phi variant
            (r"\(+p", r"ϖ"), // pi variant
            (r"\(+e", r"ϵ"), // epsilon variant
            (r"\(ts", r"ς"), // sigma terminal
            // Predefined strings:
            (r"\*(Ba", r"|"),        // vertical bar
            (r"\*(Ne", r"≠"),        // not equal
            (r"\*(Ge", r"≥"),        // greater-than-equal
            (r"\*(Le", r"≤"),        // less-than-equal
            (r"\*(Gt", r">"),        // greater-than
            (r"\*(Lt", r"<"),        // less-than
            (r"\*(Pm", r"±"),        // plus-minus
            (r"\*(If", r"infinity"), // infinity
            (r"\*(Pi", r"pi"),       // pi
            (r"\*(Na", r"NaN"),      // NaN
            (r"\*(Am", r"&"),        // ampersand
            (r"\*R", r"®"),          // restricted mark
            (r"\*(Tm", r"(Tm)"),     // trade mark
            (r"\*q", "\""),          // double-quote
            (r"\*(Rq", r"”"),        // right-double-quote
            (r"\*(Lq", r"“"),        // left-double-quote
            (r"\*(lp", r"("),        // right-parenthesis
            (r"\*(rp", r")"),        // left-parenthesis
            (r"\*(lq", r"“"),        // left double-quote
            (r"\*(rq", r"”"),        // right double-quote
            (r"\*(ua", r"↑"),        // up arrow
            (r"\*(va", r"↕"),        // up-down arrow
            (r"\*(<=", r"≤"),        // less-than-equal
            (r"\*(>=", r"≥"),        // greater-than-equal
            (r"\*(aa", r"´"),        // acute
            (r"\*(ga", r"`"),        // grave
            (r"\*(Px", r"POSIX"),    // POSIX standard name
            (r"\*(Ai", r"ANSI"),     // ANSI standard name
        ]
        .iter()
        .cloned()
        .collect();

        let mut result = String::new();

        let ac = AhoCorasick::new(replacements.keys()).expect("Build error");

        ac.replace_all_with(text, &mut result, |_, key, dst| {
            dst.push_str(replacements[key]);
            true
        });

        self.replace_unicode_escapes(&result)
    }

    /// Special block macro ta formatting
    fn format_ta(&mut self) -> String {
        String::new()
    }
}

fn split_by_width(words: Vec<String>, width: usize) -> Vec<String>{
    if width == 0{
        return words.iter()
            .map(|s|s.to_string())
            .collect::<Vec<_>>();
    }
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut i = 0; 
    while i < words.len(){
        let l = line.clone();
        if l.is_empty() && words[i].len() > width{
            lines.extend(words[i]
                .chars()
                .collect::<Vec<_>>()
                .chunks(width)
                .map(|ch| ch.iter().collect::<String>()));
            if let Some(l) = lines.pop(){
                line = l;
            }
            i += 1;
            continue;
        } else if line.len() + words[i].len() + 1 > width{
            lines.push(l);
            line.clear();
            continue;
        }
        if !line.is_empty() && line.len() < width{
            if let Some(ch) = line.chars().last(){
                if !ch.is_whitespace(){
                    line.push(' ');
                }
            }
        }
        line.push_str(&words[i]);
        i += 1;
    }
    lines.push(line);
    lines
}

fn add_indent_to_lines(lines: Vec<String>, width: usize, offset: &OffsetType) -> Vec<String>{
    lines.into_iter()
        .map(|line|{
            let mut line_indent = width.saturating_sub(line.len());
            match offset{
                OffsetType::Left => line,
                OffsetType::Right => {
                    let indent = " ".repeat(line_indent);
                    indent + &line
                }
                OffsetType::Center => {
                    line_indent = (line_indent as f32 / 2.0).floor() as usize;
                    let indent = " ".repeat(line_indent);
                    indent.clone() + &line
                },
                _ => unreachable!()
            }
        })
        .collect::<Vec<_>>()
}

fn get_symbol(last_symbol: &str, list_type: &BlType) -> String{
    match list_type{
        BlType::Bullet => "•".to_string(),
        BlType::Dash => "-".to_string(),
        BlType::Enum => {
            if last_symbol.is_empty(){
                return "0.".to_string();
            }
            let mut symbol = last_symbol.to_string();
            symbol.pop();
            let Ok(number) = symbol.parse::<usize>() else{
                return String::new();
            };
            (number + 1).to_string() + "."
        },
        _ => String::new()
    }
}

fn interleave<T: Clone + std::fmt::Debug>(v1: Vec<T>, v2: Vec<T>) -> Vec<T> {
    if v1.is_empty(){
        return v2;
    }else if v2.is_empty(){
        return v1;
    }

    let len1 = v1.len();
    let len2 = v2.len();

    let mut result = Vec::with_capacity(len1 + len2);

    let mut iter1 = v1.iter();
    let mut iter2 = v2.iter();

    for (item1, item2) in iter1.by_ref().zip(iter2.by_ref()) {
        //print!("|{:?}, {:?}|", item1, item2);
        result.push(item1.clone()); 
        result.push(item2.clone());
    }

    result.extend(iter1.cloned());
    result.extend(iter2.cloned());

    result
}

fn get_multilined(body: &[String]) -> Vec<Vec<String>> {
    body.iter()
        .filter(|el| el.lines().count() > 1)
        .map(|s| {
            s.split("\n")
            .map(|l|l.to_string())
            .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
}

fn get_onelined(
    body: &[String], 
    line_width: usize, 
    indent_str: &str, 
    offset: &OffsetType
) -> Vec<Vec<String>>{
    body.split(|el| el.lines().count() > 1)
        .collect::<Vec<_>>()
        .iter()
        .filter(|s| !s.is_empty())
        .map(|v|{
            let mut content = split_by_width(
                v.join(" ").split_whitespace()                    
                .map(|s| s.to_string())
                .collect::<Vec<_>>(), 
                line_width
            );
            content = add_indent_to_lines(content, line_width, offset);
            for line in content.iter_mut(){
                *line = indent_str.to_string() + line;
            }
            content
        })
        .collect::<Vec<_>>()
}

fn split_nested_bl(bl: MacroNode) -> Vec<Element>{
    let MacroNode{ mdoc_macro, nodes } = bl;

    let super_macros = |nodes: Vec<Element>| {
        Element::Macro(MacroNode {
            mdoc_macro: mdoc_macro.clone(),
            nodes
        })
    };

    let nested_macros = super_macros(nodes.clone());

    let Macro::Bl { list_type: super_list_type, .. } = mdoc_macro.clone() else {
        return vec![nested_macros]; 
    };
    let is_not_nested = |list_type: &BlType| {
        matches!(list_type, BlType::Item | BlType::Inset | BlType::Column | BlType::Ohang )
    };

    if !is_not_nested(&super_list_type){
        return vec![nested_macros];
    }

    let mut bl_elements = vec![];
    let mut it_elements = vec![];
    for it in nodes{
        let Element::Macro(MacroNode { mdoc_macro: Macro::It{ head }, nodes: it_nodes }) = it else{
            if !it_elements.is_empty(){
                bl_elements.push((true, super_macros(it_elements.clone())));
                it_elements.clear();
            }
            bl_elements.push((true, it));
            continue;
        };

        let mut head_elements = vec![];
        for element in head{
            if matches!(element, Element::Macro(MacroNode { mdoc_macro: Macro::Bl{ .. }, .. })){
                if !head_elements.is_empty(){
                    if !it_elements.is_empty(){
                        bl_elements.push((true, super_macros(it_elements.clone())));
                        it_elements.clear();
                    }
                    bl_elements.push((true, super_macros(vec![Element::Macro(
                        MacroNode { mdoc_macro: Macro::It{ head: head_elements.clone() }, nodes: vec![] })
                    ])));
                    head_elements.clear();
                }
                bl_elements.push((false, element));
            }else{
                head_elements.push(element);
            }
        }

        let mut body_elements = vec![];
        for element in it_nodes{
            if matches!(element, Element::Macro(MacroNode { mdoc_macro: Macro::Bl{ .. }, .. })){
                if !head_elements.is_empty() || !body_elements.is_empty(){
                    if !it_elements.is_empty(){
                        bl_elements.push((true, super_macros(it_elements.clone())));
                        it_elements.clear();
                    }
                    bl_elements.push((true, super_macros(vec![Element::Macro(
                        MacroNode { mdoc_macro: Macro::It{ head: head_elements.clone() }, nodes: body_elements.clone() }
                    )])));
                    head_elements.clear();
                    body_elements.clear();
                }
                bl_elements.push((false, element));
            }else{
                body_elements.push(element);
            }
        }

        if !head_elements.is_empty() || !body_elements.is_empty(){
            it_elements.push(Element::Macro(
                MacroNode { mdoc_macro: Macro::It{ head: head_elements }, nodes: body_elements }
            ));
        }
    }

    if !it_elements.is_empty(){
        bl_elements.push((true, super_macros(it_elements)));
    }

    return if !bl_elements.is_empty(){
        bl_elements.into_iter()
            .flat_map(|(checked, bl)|{
                if checked{
                    return vec![bl];
                }
                if let Element::Macro(ref node) = bl{
                    if let MacroNode{ mdoc_macro: Macro::Bl{ .. }, .. } = node{
                        return split_nested_bl(node.clone());
                    }
                }
                vec![]
            })
            .collect::<Vec<_>>()
    }else{
        vec![nested_macros]
    };
}

/// Trim, but leaves '\n' on ends
fn trim(string: &mut String){
    if let Some(position) = string.find(|ch| ch != '\n'){
        if let Some(s) = string.strip_prefix(&("\n".repeat(position))){
            *string = s.to_string();
        }
    }

    if let Some(position) = string.rfind(|ch| ch != '\n'){
        if let Some(s) = string.strip_suffix(&("\n".repeat(string.len().saturating_sub(2 + position)))){
            *string = s.to_string();
        }
    }
}

// Formatting block full-explicit.
impl MdocFormatter {
    fn get_indent_from_offset_type(&self, width: &Option<u8>, offset: &Option<OffsetType>) -> usize{
        if let Some(width) = width{
            return *width as usize;
        }
        let Some(offset) = offset else {
            return self.formatting_settings.indent;
        };
        match offset{
            OffsetType::Indent => 8,
            OffsetType::IndentTwo => 8 * 2,
            _ => self.formatting_settings.indent
        }
    }

    fn get_offset_from_offset_type(&self, offset: &Option<OffsetType>) -> OffsetType{
        let Some(offset) = offset else{
            return OffsetType::Left;
        };
        match offset.clone(){
            OffsetType::Indent | OffsetType::IndentTwo => OffsetType::Left,
            OffsetType::Left | OffsetType::Right | OffsetType::Center => offset.clone()
        }
    }

    fn format_bd_block(
        &mut self,
        block_type: BdType,
        offset: Option<OffsetType>,
        compact: bool,
        macro_node: MacroNode,
    ) -> String {
        let indent = self.get_indent_from_offset_type(&None, &offset);
        let mut offset = self.get_offset_from_offset_type(&offset);
        if block_type == BdType::Centered{
            offset = OffsetType::Center;
        }

        self.formatting_state.current_indent += indent;
        let indent = self.formatting_state.current_indent;
        let indent_str = " ".repeat(indent);
        let line_width = self.formatting_settings.width.saturating_sub(indent);

        let formatted_elements = macro_node.nodes
            .into_iter()
            .map(|el| {
                let is_aligned_macro = if let Element::Macro( MacroNode{ mdoc_macro, .. } ) = el.clone(){
                    matches!(mdoc_macro, Macro::Bl{ .. }) ||
                    matches!(mdoc_macro, Macro::Bd{ .. })
                }else{
                    false
                };
                let string = self.format_node(el.clone());
                let content = if is_aligned_macro{
                    vec![string]
                }else{
                    string
                        .split_whitespace()
                        .map(|s|s.to_string())
                        .collect::<Vec<_>>()
                };
                (is_aligned_macro, content)
            });

        if line_width == 0{
            let content = formatted_elements
                .flat_map(|(_, s)| s)
                .collect::<Vec<_>>()
                .join(" ");
            return content;
        }

        let mut lines = vec![];
        let mut is_last_aligned_macro = false;
        for (is_aligned_macro, content) in formatted_elements{
            if is_aligned_macro{
                lines.extend(content);
            }else{   
                let mut content = content;
                if let BdType::Centered | BdType::Filled | BdType::Ragged = block_type{
                    if !is_last_aligned_macro{
                        if let Some(current_line) = lines.last(){
                            let current_line = current_line.split_whitespace()
                                .map(|s|s.to_string())
                                .collect::<Vec<_>>();
                            content = [current_line, content].concat();
                        }
                    }
                }     
                let l = split_by_width(content, line_width);
                let l = add_indent_to_lines(l, line_width, &offset)
                    .iter()
                    .map(|line|{
                        indent_str.clone() + line
                    })
                    .collect::<Vec<_>>();
                lines.extend(l);
            }
            is_last_aligned_macro = is_aligned_macro;
        }

        let mut content = lines.join("\n");

        self.formatting_state.current_indent = self.formatting_state.current_indent.saturating_sub(indent);

        if !compact{
            content = "\n".to_string() + &content;
        }

        while let Some('\n') = content.chars().last(){
            content.pop();
        }

        content += "\n\n";
        content
    }

    fn format_bf_block(&mut self, bf_type: BfType, macro_node: MacroNode) -> String {
        let font_change = match bf_type{
            BfType::Emphasis => {
                if self.supports_italic() {
                    "\x1b[3m".to_string()
                } else if self.supports_underline() {
                    "\x1b[4m".to_string()
                }else{
                    String::new()
                }
            },
            BfType::Literal => {
                String::new()
            },
            BfType::Symbolic => {
                if self.supports_bold(){
                    "\x1b[1m".to_string()
                }else{
                    String::new()
                }
            }
        };

        let content = macro_node
            .nodes
            .into_iter()
            .map(|node| {
                let mut content = self.format_node(node);
                if !content.ends_with('\n') && !content.is_empty() {
                    content.push_str(&self.formatting_state.spacing);
                }
                content
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>()
            .join("");

        let normal_font = if !font_change.is_empty() {
            "\x1b[0m"
        }else{
            ""
        };

        font_change + &content + normal_font
    }

    fn format_bk_block(&mut self, macro_node: MacroNode) -> String {
        let content = macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing);

        content.replace("\n", " ").replace("\r", "")
    }

    fn add_missing_indent(&self, content: &mut String){
        *content = content.split("\n")
            .map(|line|{
                let indent_is_small = line.chars()
                    .take_while(|ch| ch.is_whitespace())
                    .count() < self.formatting_settings.indent;

                let is_not_empty = !(line.chars().all(|ch| ch.is_whitespace()) || line.is_empty()); 
                let line = if indent_is_small && is_not_empty{
                    " ".repeat(self.formatting_settings.indent) + line.trim_start()
                }else{
                    line.to_string()
                };

                line
            })
            .collect::<Vec<_>>()
            .join("\n");
    }

    fn format_bl_symbol_block(
        &self, 
        items: Vec<(String, Vec<String>)>,
        width: Option<u8>,
        offset: Option<OffsetType>,
        list_type: BlType,
        compact: bool
    ) -> String{
        let indent = self.get_indent_from_offset_type(&width, &offset);
        let offset = self.get_offset_from_offset_type(&offset);
        let origin_indent = self.formatting_state.current_indent;
        let width = self.formatting_settings.width;
        let symbol_range = if let BlType::Enum = list_type{
            items.len().to_string().len() + 1
        }else{
            1
        };
        let full_indent = origin_indent + indent;
        let line_width = width.saturating_sub(full_indent);
        let indent_str = " ".repeat(full_indent);

        let mut symbol = get_symbol("", &list_type);
        let mut content = String::new();
        for (_, body) in items{
            let mut body = body;
            body.retain(|s| !s.is_empty());
            let multilined = get_multilined(&body);
            let onelined = get_onelined(&body, line_width, &indent_str, &offset);   
            body = interleave(onelined, multilined)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();  
            
            if let Some(first_line) = body.get_mut(0){
                if !first_line.chars().all(|ch| ch.is_whitespace()){
                    symbol = get_symbol(symbol.as_str(), &list_type);
                }
                if first_line.len() > origin_indent + symbol_range{
                    first_line.replace_range(origin_indent..(origin_indent + symbol_range), &symbol);
                }
            }
            content.push_str(&(body.join("\n") + "\n"));
            if !compact{
                content.push('\n');
            }
        }  

        trim(&mut content);

        content
    }

    fn format_bl_item_block(
        &self, 
        items: Vec<(String, Vec<String>)>,
        width: Option<u8>,
        offset: Option<OffsetType>, 
        compact: bool
    ) -> String{
        let indent = self.get_indent_from_offset_type(&width, &offset);
        let offset = self.get_offset_from_offset_type(&offset);
        let origin_indent = self.formatting_state.current_indent;
        let width = self.formatting_settings.width;
        let line_width = width.saturating_sub(origin_indent + indent);

        let mut content = String::new();
        for (_, body) in items{
            let body = body.join(" ");
            let mut body = split_by_width(
                body.split_whitespace()                    
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
                line_width + indent
            );
            body = add_indent_to_lines(body, line_width + indent, &offset);
            content.push_str(&body.join("\n"));
            content.push('\n');
            if !compact{
                content.push('\n');
            }
        } 

        trim(&mut content);

        content
    }
    
    fn format_bl_ohang_block(
        &self, 
        items: Vec<(String, Vec<String>)>,
        width: Option<u8>,
        offset: Option<OffsetType>, 
        compact: bool
    ) -> String{
        let indent = self.get_indent_from_offset_type(&width, &offset);
        let offset = self.get_offset_from_offset_type(&offset);
        let origin_indent = self.formatting_state.current_indent;
        let width = self.formatting_settings.width;
        let line_width = width.saturating_sub(origin_indent + indent);
        let origin_indent_str = " ".repeat(origin_indent);

        let items = items.into_iter()
            .map(|(head, body)|{
                (head, body.join(" "))    
            })
            .collect::<Vec<_>>();

        let mut content = String::new();
        for (head, body) in items{
            let mut h = split_by_width(
                head.split_whitespace()                    
                .map(|s| s.to_string())
                .collect::<Vec<_>>(), 
                line_width + indent
            );
            let mut body = split_by_width(
                body.split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(), 
                line_width + indent
            );
            h.extend(body);
            body = h;
            body = add_indent_to_lines(body, line_width + indent, &offset);
            for line in body.iter_mut(){
                *line = origin_indent_str.clone() + line;
            }
            content.push_str(&(body.join("\n") + "\n"));
            if !compact{
                content.push('\n');
            }
        }

        trim(&mut content);

        content
    }

    fn format_bl_inset_block(
        &self, 
        items: Vec<(String, Vec<String>)>,
        width: Option<u8>,
        offset: Option<OffsetType>, 
        compact: bool,
        list_type: BlType
    ) -> String{
        let head_space = match list_type{
            BlType::Inset => " ", 
            BlType::Diag => "  ", 
            _ => " "
        };
        let indent = self.get_indent_from_offset_type(&width, &offset);
        let offset = self.get_offset_from_offset_type(&offset);
        let origin_indent = self.formatting_state.current_indent;
        let width = self.formatting_settings.width;
        let line_width = width.saturating_sub(origin_indent + indent);
        let origin_indent_str = " ".repeat(origin_indent);

        let items = items.into_iter()
            .map(|(head, body)|{
                (head, body.join(" "))
            })
            .collect::<Vec<_>>();

        let get_words = |s: &str| s.split_whitespace()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        let mut content = String::new();
        for (head, body) in items{
            let mut head = get_words(&head);
            let mut body = get_words(&body);
            if let Some(word) = head.last_mut(){
                *word += head_space;
            }

            body = split_by_width(
                [head, body].concat(), 
                line_width + indent
            );
            
            body = add_indent_to_lines(body, line_width + indent, &offset);
            for line in body.iter_mut(){
                *line = origin_indent_str.clone() + line;
            }
            content.push_str(&(body.join("\n") + "\n"));
            if !compact{
                content.push('\n');
            }
        }

        trim(&mut content);

        content
    }

    fn format_bl_column_block(
        &self, 
        items: Vec<Vec<String>>,
        columns: Vec<String>,
        _compact: bool
    ) -> String{
        fn format_table(table: Vec<Vec<String>>, col_count: usize, max_line_width: usize) -> String {
            if table.is_empty() {
                return String::new();
            }
            
            let mut col_widths = vec![0; col_count];
            
            for row in &table {
                for (i, cell) in row.iter().enumerate() {
                    if i >= col_widths.len(){
                        break;
                    }
                    col_widths[i] = col_widths[i].max(cell.len());
                }
            }
            
            let mut result = String::new();
            for row in table {
                let total_width: usize = col_widths.iter().sum::<usize>() + 2 * (col_count - 1);
                let mut offset = 0;
                let indent_step = 8;
                
                if total_width > max_line_width {
                    for (i, cell) in row.iter().take(col_widths.len()).enumerate() {
                        result.push_str(&" ".repeat(offset));
                        result.push_str(&format!("{:<width$}\n", cell, width = col_widths[i]));
                        offset += indent_step;
                    }
                } else {
                    let mut line_width = 0;
                    for (i, cell) in row.iter().enumerate() {
                        let cell_width = col_widths[i] + 1;
                        if line_width + cell_width > max_line_width {
                            result.push('\n');
                            offset += indent_step;
                            line_width = offset;
                            result.push_str(&" ".repeat(offset));
                        }
                        result.push_str(&format!("{:<width$}  ", cell, width = col_widths[i]));
                        line_width += cell_width;
                    }
                    result.push('\n');
                }
            }
            result
        }

        let origin_indent = self.formatting_state.current_indent;
        let width = self.formatting_settings.width;
        let line_width = width.saturating_sub(origin_indent);

        let mut content = format_table(items, columns.len(), line_width);
        
        content = content.lines()
            .map(|line|{
                " ".repeat(origin_indent) + line
            })
            .collect::<Vec<_>>()
            .join("\n");

        trim(&mut content);

        if !content.ends_with("\n"){
            content.push('\n');
        }

        content
    }

    fn format_bl_tag_block(
        &self, 
        items: Vec<(String, Vec<String>)>,
        width: Option<u8>,
        offset: Option<OffsetType>, 
        compact: bool
    ) -> String{
        let indent = self.get_indent_from_offset_type(&width, &offset);
        let offset = self.get_offset_from_offset_type(&offset);
        let origin_indent = self.formatting_state.current_indent;
        let width = self.formatting_settings.width;
        let line_width = width.saturating_sub(origin_indent + indent);
        let indent_str = " ".repeat(origin_indent + indent);
        let origin_indent_str = " ".repeat(origin_indent);

        let mut content = String::new();
        for (head, body) in items{
            let multilined = get_multilined(&body);
            let onelined = get_onelined(&body, line_width, &indent_str, &offset);   
            let mut body = interleave(onelined, multilined)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(); 

            let space = if head.len() < indent.saturating_sub(2){
                if let Some(line) = body.first_mut(){
                    *line = line.trim_start().to_string();
                }
                " ".repeat(indent - head.len())
            }else{
                "\n".to_string()
            };
            content.push_str(
                &(origin_indent_str.clone() + &head + &space + &body.join("\n") + "\n")
            );
            if !compact{
                content.push('\n');
            }
        } 

        trim(&mut content);

        content
    }

    fn format_bl_hang_block(
        &self, 
        items: Vec<(String, Vec<String>)>,
        is_first_block: Vec<bool>,
        width: Option<u8>,
        offset: Option<OffsetType>, 
        compact: bool
    ) -> String{
        let indent = self.get_indent_from_offset_type(&width, &offset);
        let offset = self.get_offset_from_offset_type(&offset);
        let origin_indent = self.formatting_state.current_indent;
        let width = self.formatting_settings.width;
        let line_width = width.saturating_sub(origin_indent + indent);
        let indent_str = " ".repeat(origin_indent + indent);
        let origin_indent_str = " ".repeat(origin_indent);
        let mut content = String::new();

        for (i, (head, body)) in items.into_iter().enumerate(){
            let mut body = body;
            let mut head = head.clone(); 

            if !is_first_block[i]{
                let first_line = body.get(0)
                    .cloned()
                    .unwrap_or_default()
                    .split_whitespace()
                    .map(|s|s.to_string())
                    .collect::<Vec<_>>();
                let mut i = 0;
                let mut j = 0;
                if head.len() > indent.saturating_sub(1){
                    while head.len() < line_width + indent && i < first_line.len() {
                        if head.len() + first_line[i].len() >= line_width + indent{
                            break;
                        }
                        head.push_str(&(" ".to_string() + &first_line[i]));
                        j += first_line[i].len() + 1;
                        i += 1;
                    }
                }
                if let Some(line) = body.get_mut(0){
                    line.replace_range(0..j, "");
                }
            }

            let multilined = get_multilined(&body);
            let onelined = get_onelined(&body, line_width, &indent_str, &offset);   
            body = interleave(onelined, multilined)
                 .into_iter()
                 .flatten()
                 .collect::<Vec<_>>(); 

            body.retain(|s| !s.is_empty());

            if head.len() < indent.saturating_sub(1){
                if let Some(line) = body.first_mut(){
                    *line = line.trim_start().to_string(); 
                }
                let space = if is_first_block[i]{
                    "\n".to_string() + &origin_indent_str.clone() + &" ".repeat(indent)
                }else{
                    " ".repeat(indent - head.len())
                };
                content.push_str(&(origin_indent_str.clone() + &head + &space + body.join("\n").trim_end() + "\n"));
            }else{
                content.push_str(&(origin_indent_str.clone() + head.trim_end() + "\n" + body.join("\n").trim_end() + "\n"));
            }
            if !compact{
                content.push('\n');
            }
        } 

        trim(&mut content);

        content
    }

    fn get_heads(&mut self, macro_node: MacroNode, list_type: &BlType) -> Vec<String>{
        macro_node.nodes
            .into_iter()
            .filter_map(|el|{
                let Element::Macro(MacroNode{ mdoc_macro: Macro::It{ head }, .. }) = el else{
                    return None;
                }; 

                if list_type == &BlType::Column{
                    None
                }else {
                    let content = head.iter()
                        .map(|element| self.format_node(element.clone()))
                        .collect::<Vec<_>>()
                        .join(&self.formatting_state.spacing);

                    if !content.is_empty(){
                        Some(content)
                    }else{
                        None
                    }
                }
            })
            .collect::<Vec<_>>()
    }

    fn prepare_rows(&mut self, elements: Vec<Element>) -> Vec<String>{
        elements.split(|el| 
                matches!(el, Element::Macro(MacroNode{ mdoc_macro: Macro::Ta, .. }))
            ).map(|elements|{
                elements.iter()
                    .map(|el| self.format_node(el.clone()))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
    }

    fn get_bodies(&mut self, macro_node: MacroNode, list_type: &BlType) -> Vec<Vec<String>>{        
        macro_node.nodes
            .into_iter()
            .filter_map(|el|{
                let Element::Macro(MacroNode{ mdoc_macro: Macro::It{head}, nodes }) = el else{
                    return None;
                }; 

                if list_type == &BlType::Column{
                    Some(self.prepare_rows([head, nodes].concat()))
                }else{
                    Some(nodes.iter()
                    .filter(|el| 
                        !matches!(el, Element::Macro(MacroNode{ mdoc_macro: Macro::Ta, .. }))
                    )
                    .map(|element| self.format_node(element.clone()))
                    .collect::<Vec<_>>())
                }
            })
            .collect::<Vec<_>>()
    }

    fn format_bl_block(
        &mut self,
        list_type: BlType,
        width: Option<u8>,
        offset: Option<OffsetType>,
        compact: bool,
        columns: Vec<String>,
        macro_node: MacroNode,
    ) -> String {
        let heads = self.get_heads(macro_node.clone(), &list_type);
        self.formatting_state.current_indent += self.get_indent_from_offset_type(&width, &offset);
        let bodies = self.get_bodies(macro_node.clone(), &list_type);
        self.formatting_state.current_indent = 
            self.formatting_state.current_indent.saturating_sub(self.get_indent_from_offset_type(&width, &offset));
        let items = heads.into_iter()
            .zip(bodies.clone())
            .collect::<Vec<_>>();

        let content = match list_type {
            BlType::Bullet | BlType::Dash | BlType::Enum => self.format_bl_symbol_block(items, width, offset, list_type, compact),
            BlType::Item => self.format_bl_item_block(items, width, offset, compact),
            BlType::Ohang => self.format_bl_ohang_block(items, width, offset, compact),
            BlType::Inset | BlType::Diag => self.format_bl_inset_block(items, width, offset, compact, list_type),
            BlType::Column => self.format_bl_column_block(bodies, columns, compact),
            BlType::Tag => self.format_bl_tag_block(items, width, offset, compact),
            BlType::Hang => {
                let MacroNode{ nodes, .. } = macro_node;
                let is_first_block = nodes.iter()
                    .map(|el|{
                        if let Element::Macro(MacroNode { nodes, .. }) = el{
                            if let Some(Element::Macro(MacroNode { mdoc_macro, .. })) = nodes.get(0){
                                return matches!(mdoc_macro, &Macro::Bl{ .. } | &Macro::Bd{ .. } );
                            }
                        }
                        false
                    })
                    .collect::<Vec<_>>();
                self.format_bl_hang_block(items, is_first_block, width, offset, compact)
            }
        };

        "\n".to_string() + &content
    }

    fn format_bl_blocks(
        &mut self,
        macro_node: MacroNode,
    ) -> String {
        split_nested_bl(macro_node)
            .into_iter()
            .map(|element|{
                let Element::Macro(ref macro_node) = element else{
                    return self.format_node(element);
                };
                let MacroNode { mdoc_macro, .. } = macro_node.clone();
                let Macro::Bl { 
                    list_type, 
                    width,
                    offset, 
                    compact, 
                    columns 
                } = mdoc_macro.clone() else {
                    return self.format_node(element);
                };
                self.format_bl_block(list_type, width, offset, compact, columns, macro_node.clone())
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// Formatting block full-implicit.
impl MdocFormatter {
    fn format_it_block(&mut self, _head: Vec<Element>, _macro_node: MacroNode) -> String {
        String::new()
    }

    fn format_nd(&mut self, macro_node: MacroNode) -> String {
        let content = macro_node
            .nodes
            .into_iter()
            .map(|node| {
                let mut content = self.format_node(node);
                if !content.ends_with('\n') && !content.is_empty() {
                    content.push_str(&self.formatting_state.spacing);
                }
                content
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>()
            .join("");

        format!("– {}", content)
    }

    fn format_nm(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_nm_synopsis(&mut self, macro_node: MacroNode) -> String {
        let content = macro_node
            .nodes
            .into_iter()
            .map(|node| {
                let mut content = self.format_node(node);
                if !content.ends_with('\n') && !content.is_empty() {
                    content.push_str(&self.formatting_state.spacing);
                }
                content
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>()
            .join("");
        
        if !content.is_empty() {
            self.formatting_state.first_name = Some(content.clone());
        }

        content
    }

    fn format_sh_block(&mut self, title: String, macro_node: MacroNode) -> String {
        let mut ss_lines_positions = vec![];
        let mut current_lines_count = 0;
        let mut prev_node = Macro::Soi;
        self.formatting_state.current_indent += self.formatting_settings.indent;
        let mut content = macro_node
            .nodes
            .into_iter()
            .map(|node| {
                let mut content = match node {
                    Element::Macro(ref macro_node) => {
                        if title.eq_ignore_ascii_case("SYNOPSIS") {
                            let formatted = match &macro_node.mdoc_macro {
                                Macro::In { 
                                    ref filename 
                                } => self.format_in_synopsis(filename.as_str(), macro_node.clone(), &prev_node),
                                Macro::Vt => self.format_vt_synopsis(macro_node.clone()),
                                Macro::Nm => self.format_nm_synopsis(macro_node.clone()),
                                Macro::Ft => self.format_ft_synopsis(macro_node.clone()),
                                Macro::Fn { funcname } => self.format_fn_synopsis(&funcname, macro_node.clone()),
                                _ => self.format_macro_node(macro_node.clone())
                            };

                            prev_node = macro_node.mdoc_macro.clone();
                            println!("Loop: Fromated In macro {}", formatted.replace("\n", "NEWLINE"));

                            return formatted;
                        } 
                        // else if title.eq_ignore_ascii_case("AUTHORS") {
                            // match &macro_node.mdoc_macro {
                            //     Macro::An { author_name_type } => self.format_an_authors(author_name_type.clone(), macro_node.clone()),
                            //     _ => self.format_node(node)
                            // }
                        // } 
                        else {
                            self.format_macro_node(macro_node.clone())
                        }

                    },
                    Element::Text(ref text) => self.format_text_node(&text),
                    Element::Eoi => String::new()
                };

                if matches!(node, Element::Macro(MacroNode { mdoc_macro: Macro::Ss{ .. }, .. })){
                    ss_lines_positions.push(current_lines_count);
                }

                if !content.ends_with('\n') && !content.is_empty() {
                    content.push_str(&self.formatting_state.spacing);
                }

                current_lines_count += content.lines().count();

                println!("Sh block: Fromated In macro {}", content.replace("\n", "NEWLINE"));

                content
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("");

        self.add_missing_indent(&mut content);

        self.formatting_state.current_indent = 0;

        format!(
            "\n{}\n{}", 
            title.to_uppercase(), 
            content
        )
    }

    fn format_ss_block(&mut self, title: String, macro_node: MacroNode) -> String {        
        if self.formatting_state.current_indent == 0{
            self.formatting_state.current_indent += self.formatting_settings.indent;
        }
        let mut content = macro_node
            .nodes
            .into_iter()
            .map(|node| {
                let mut content = self.format_node(node);
                if content.chars().last() != Some('\n') && !content.is_empty() {
                    content.push_str(&self.formatting_state.spacing);
                }
                content
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("");

        self.add_missing_indent(&mut content);
        self.formatting_state.current_indent = 0;

        let mut title_ident = self.formatting_settings.indent.saturating_sub(2);
        if title_ident == 0 {
            title_ident = 1;
        }
        
        format!(
            "{}{title}\n{content}",
            " ".repeat(title_ident),
        )
    }
}

// Formatting block partial-explicit.
impl MdocFormatter {
    fn format_partial_explicit_block(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| {
                let mut content = self.format_node(node);
                if !content.ends_with('\n') && !content.is_empty() {
                    content.push_str(&self.formatting_state.spacing);
                }
                content
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("")
    }

    fn format_a_block(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_explicit_block(macro_node);

        format!("⟨{}⟩", formatted_block.trim())
    }

    fn format_b_block(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_explicit_block(macro_node);

        format!("[{}]", formatted_block)
    }

    fn format_br_block(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_explicit_block(macro_node);

        format!("{{{}}}", formatted_block)
    }

    fn format_d_block(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_explicit_block(macro_node);

        format!("“{}”", formatted_block)
    }

    fn format_e_block(
        &mut self,
        opening_delimiter: Option<char>,
        closing_delimiter: Option<char>,
        macro_node: MacroNode,
    ) -> String {
        let formatted_block = self.format_partial_explicit_block(macro_node);

        match (opening_delimiter, closing_delimiter) {
            (Some(open), Some(close)) => {
                format!("{}{}{}", open, formatted_block, close)
            }
            (Some(open), None) => {
                format!("{}{}", open, formatted_block)
            }
            (None, Some(close)) => {
                format!("{}{}", formatted_block, close)
            }
            (None, None) => formatted_block,
        }
    }

    fn format_f_block(&mut self, funcname: String, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_explicit_block(macro_node);

        format!("{}({})", funcname, formatted_block)
    }

    fn format_o_block(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_explicit_block(macro_node);

        format!("[{}]", formatted_block)
    }

    fn format_p_block(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_explicit_block(macro_node);

        format!("({})", formatted_block)
    }

    fn format_q_block(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_explicit_block(macro_node);

        format!("\"{}\"", formatted_block)
    }

    fn format_s_block(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_explicit_block(macro_node);

        format!("'{}'", formatted_block)
    }

    fn format_x_block(&mut self, macro_node: MacroNode) -> String {
        self.format_partial_explicit_block(macro_node)
    }
}

// Formatting Rs-Re bloock. Can contain only %* macros
impl MdocFormatter {
    fn format_rs_block(&self, macro_node: MacroNode) -> String {
        let mut iter = macro_node.nodes.into_iter().peekable();

        let mut items = Vec::new();
        while let Some(el) = iter.peek() {
            if let Element::Macro(node) = el {
                if node.mdoc_macro == Macro::A {
                    let el = iter.next().unwrap();
                    if let Element::Macro(node) = el {
                        items.push(self.format_a(node));
                    }
                } else {
                    break;
                }
            } else {
                unreachable!("Unexpected rule!");
            }
        }

        let formatted_a = match items.len() {
            0 => "".to_string(),
            1 => items[0].clone(),
            2 => format!("{} and {}", items[0], items[1]),
            _ => {
                let last = items.last().unwrap();
                let all_but_last = &items[..items.len() - 1];
                format!("{}, and {}", all_but_last.join(", "), last)
            }
        };

        let formatted_all = iter
            .map(|el| match el {
                Element::Macro(node) => match node.mdoc_macro {
                    Macro::B => self.format_b(node),
                    Macro::C => self.format_c(node),
                    Macro::D => self.format_d(node),
                    Macro::I => self.format_i(node),
                    Macro::J => self.format_j(node),
                    Macro::N => self.format_n(node),
                    Macro::O => self.format_o(node),
                    Macro::P => self.format_p(node),
                    Macro::Q => self.format_q(node),
                    Macro::R => self.format_r(node),
                    Macro::T => self.format_t(node),
                    Macro::U => self.format_u(node),
                    Macro::V => self.format_v(node),
                    _ => unreachable!("Rs can not contain macro: {:?}", node),
                },
                _ => unreachable!("Unexpected element type!"),
            })
            .collect::<Vec<_>>()
            .join(", ");

        match (formatted_a.is_empty(), formatted_all.is_empty()) {
            (true, true) => "".to_string(),
            (true, false) => format!("{}.", formatted_all),
            (false, true) => format!("{}.", formatted_a),
            (false, false) => format!("{}, {}.", formatted_a, formatted_all),
        }
    }

    fn format_a(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_b(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_c(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_d(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_i(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_j(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_n(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_o(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_p(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_q(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_r(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_t(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_u(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_v(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }
}

// Formatting block partial-implicit.
impl MdocFormatter {
    fn format_partial_implicit_block(&mut self, macro_node: MacroNode) -> String {
        let mut result = String::new();
        let mut prev_was_open = false;
        let mut is_first_node = true;
    
        for node in macro_node.nodes {
            let content = match node {
                Element::Text(text) => {
                    match text.as_str() {
                        "(" | "[" => {
                            prev_was_open = true;
                            text.clone()
                        }
                        ")" | "]" | "." | "," | ":" | ";" | "!" | "?" => {
                            prev_was_open = false;
                            text.clone()
                        }
                        _ => {
                            let formatted_text = self.format_text_node(&text);
                            let offset = if is_first_node || prev_was_open {
                                ""
                            } else {
                                self.formatting_state.spacing.as_str()
                            };
                            prev_was_open = false;
                            format!("{}{}", offset, formatted_text)
                        }
                    }
                }
                other => {
                    let mut s = self.format_node(other);
                    if !s.is_empty() && !s.ends_with('\n') {
                        s.push_str(&self.formatting_state.spacing);
                    }
                    s
                }
            };
    
            if !content.is_empty() {
                result.push_str(&content);
            }
            if is_first_node {
                is_first_node = false;
            }
        }
        result
    }

    fn format_aq(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_implicit_block(macro_node);

        format!("⟨{}⟩", formatted_block.trim())
    }

    fn format_bq(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_implicit_block(macro_node);

        format!("[{}]", formatted_block.trim())
    }

    fn format_brq(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_implicit_block(macro_node);

        format!("{{{}}}", formatted_block.trim())
    }

    fn format_d1(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_implicit_block(macro_node);

        let spaces = " ".repeat(self.formatting_settings.indent);

        format!("{}{}", spaces, formatted_block.trim())
    }

    fn format_dl(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_implicit_block(macro_node);

        let spaces = " ".repeat(self.formatting_settings.indent);

        format!("{}{}", spaces, formatted_block.trim())
    }

    fn format_dq(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_implicit_block(macro_node);

        format!("“{}”", formatted_block.trim())
    }

    fn format_en(&mut self, macro_node: MacroNode) -> String {
        self.format_partial_implicit_block(macro_node)
            .trim()
            .to_string()
    }

    fn format_op(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_implicit_block(macro_node);

        format!("[{}]", formatted_block.trim())
    }

    fn format_pq(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_implicit_block(macro_node);

        format!("({})", formatted_block.trim())
    }

    fn format_ql(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_implicit_block(macro_node);

        format!("‘{}’", formatted_block.trim())
    }

    fn format_qq(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_implicit_block(macro_node);

        format!("\"{}\"", formatted_block.trim())
    }

    fn format_sq(&mut self, macro_node: MacroNode) -> String {
        let formatted_block = self.format_partial_implicit_block(macro_node);

        format!("'{}'", formatted_block.trim())
    }

    fn format_vt(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_vt_synopsis(&mut self, macro_node: MacroNode) -> String {
        self.format_partial_implicit_block(macro_node)
            .trim()
            .to_string()
    }
}

// Format other in-line macros.
impl MdocFormatter {
    fn format_inline_macro(&self, macro_node: MacroNode) -> String {
        let mut result = String::new();
        let mut prev_was_open = false;
        let mut is_first_node = true;

        for node in macro_node.nodes {
            match node {
                Element::Text(text) => match text.as_str() {
                    "(" | "[" => {
                        result.push_str(&text);
                        prev_was_open = true;
                    }
                    ")" | "]" | "." | "," | ":" | ";" | "!" | "?" => {
                        result.push_str(&text);
                        prev_was_open = false;
                    }
                    _ => {
                        match prev_was_open {
                            true => result.push_str(&self.format_text_node(&text)),
                            false => {
                                let offset = if is_first_node { "" } else { self.formatting_state.spacing.as_str() };
                                let formatted_node =
                                    format!("{}{}", offset, self.format_text_node(&text));
                                result.push_str(&formatted_node);
                            }
                        }
                        prev_was_open = false;
                    }
                },
                _ => unreachable!("macro can't contain macro node or EOI!"),
            }

            if is_first_node {
                is_first_node = false;
            }
        }

        result
    }

    fn format_ad(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_ap(&self, macro_node: MacroNode) -> String {
        let content = self.format_inline_macro(macro_node);
        format!("'{}", content)
    }

    fn format_an(&mut self, an_type: AnType, macro_node: MacroNode) -> String {
        match an_type {
            AnType::NoSplit => {
                self.formatting_state.split_mod = false;
                String::new()
            }
            AnType::Split => {
                self.formatting_state.split_mod = true;
                String::new()
            }
            AnType::Name => {
                let content = self.format_inline_macro(macro_node);

                match self.formatting_state.split_mod {
                    true => format!("\n{}", content),
                    false => content,
                }
            }
        }
    }

    fn format_an_authors(&mut self, an_type: AnType, macro_node: MacroNode) -> String {
        unimplemented!()
    }

    fn format_ar(&self, macro_node: MacroNode) -> String {
        if macro_node.nodes.is_empty() {
            return "file ...".to_string();
        }

        self.format_inline_macro(macro_node)
    }

    fn format_bt(&self) -> String {
        "is currently in beta test.".to_string()
    }

    fn format_cd(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_cm(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_db(&self) -> String {
        "".to_string()
    }

    fn format_dv(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_em(&self, macro_node: MacroNode) -> String {
        let line = self.format_inline_macro(macro_node);

        if self.supports_italic() {
            format!("\x1b[3m{line}\x1b[0m")
        } else if self.supports_underline() {
            format!("\x1b[4m{line}\x1b[0m")
        } else {
            line
        }
    }

    fn format_dt(&mut self, title: Option<String>, section: &str, arch: Option<String>) -> String {
        let title = match title {
            Some(name) => format!("{name}({section})"),
            None if section.is_empty() => "UNTITLED".to_string(),
            _ => format!("UNTITLED({section})"),
        };

        let section = match section {
            "1" => "General Commands Manual",
            "2" => "System Calls Manual",
            "3" => "Library Functions Manual",
            "4" => "Device Drivers Manual",
            "5" => "File Formats Manual",
            "6" => "Games Manual",
            "7" => "Miscellaneous Information Manual",
            "8" => "System Manager's Manual",
            "9" => "Kernel Developer's Manual",
            _ if section.is_empty() => "LOCAL",
            _ => section,
        };

        let section = if let Some(val) = arch {
            format!("{section} ({val})")
        } else {
            section.to_string()
        };

        let side_len = title.len();
        let center_len = section.len();

        let center_start = (self.formatting_settings.width / 2).saturating_sub(center_len / 2);

        let right_start = self.formatting_settings.width.saturating_sub(side_len);

        let mut line = String::with_capacity(self.formatting_settings.width);

        line.push_str(&title);

        if center_start > side_len {
            line.push_str(&" ".repeat(center_start - side_len));
        }
        line.push_str(&section);

        let current_len = line.len();
        if right_start > current_len {
            line.push_str(&" ".repeat(right_start - current_len));
        }
        line.push_str(&title);

        let final_len = line.len();
        if final_len < self.formatting_settings.width {
            line.push_str(&" ".repeat(self.formatting_settings.width - final_len));
        }

        self.formatting_state.header_text = Some(line + "\n");
        // self.formatting_state.header_text = Some(line);
        String::new()
    }

    fn format_dx(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_dd(&mut self, line: &str) -> String {
        fn parse_month_name(month: &str) -> Option<u32> {
            let mut months = HashMap::new();
            months.insert("january", 1);
            months.insert("february", 2);
            months.insert("march", 3);
            months.insert("april", 4);
            months.insert("may", 5);
            months.insert("june", 6);
            months.insert("july", 7);
            months.insert("august", 8);
            months.insert("september", 9);
            months.insert("october", 10);
            months.insert("november", 11);
            months.insert("december", 12);
        
            months.get(&month.to_lowercase()[..]).copied()
        }

        let trimmed = line.trim();

        if trimmed == "$Mdocdate$" {
            return chrono::Utc::now().format("%B %-d, %Y").to_string();
        }

        let prefix = "$Mdocdate: ";
        if let Some(remainder) = trimmed.strip_prefix(prefix) {
            let mut parts = remainder.split_whitespace();

            let Some(month_str) = parts.next() else {
                return line.to_string();
            };
            let Some(day_str) = parts.next() else {
                return line.to_string();
            };
            let Some(year_str) = parts.next() else {
                return line.to_string();
            };

            let Some(month_num) = parse_month_name(month_str) else {
                return line.to_string();
            };

            let Ok(day_num) = day_str.parse::<u32>() else {
                return line.to_string();
            };
            let Ok(year_num) = year_str.parse::<i32>() else {
                return line.to_string();
            };

            let Some(date) = chrono::NaiveDate::from_ymd_opt(year_num, month_num, day_num) else {
                return line.to_string();
            };

            return date.format("%B %-d, %Y").to_string();
        }

        line.to_string()
    }

    fn format_bx(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_bsx(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_at(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_er(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_es(&self, opening_delimiter: char, closing_delimiter: char, macro_node: MacroNode) -> String {
        let c = self.format_inline_macro(macro_node);

        format!("{}{} {}", opening_delimiter, closing_delimiter, c)
    }

    fn format_ev(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_ex(&mut self, macro_node: MacroNode) -> String {
        let mut content = macro_node
            .nodes
            .clone()
            .into_iter()
            .map(|node| self.format_node(node))
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>()
            .join(", ");

        if macro_node.nodes.is_empty() {
            content = self.formatting_state.first_name.clone().unwrap_or_default();
        }

        if let Some(pos) = content.rfind(",") {
            content.replace_range(pos..(pos + 1), " and");
        }

        let ending = if macro_node.nodes.len() <= 1 {
            "y"
        } else {
            "ies"
        };

        if !content.is_empty() {
            format!("The {content} utilit{ending} exits 0 on success, and >0 if an error occurs.")
        } else {
            String::new()
        }
    }

    fn format_fa(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_fd(&self, directive: &str, arguments: &[String]) -> String {
        format!(
            "{directive} {}",
            arguments.join(&self.formatting_state.spacing)
        )
    }

    fn format_fl(&mut self, macro_node: MacroNode) -> String {
        let mut result = String::new();
        let mut prev_was_open = false;
        let mut is_first_node = true;

        for node in macro_node.nodes {
            match node {
                Element::Text(text) => match text.as_str() {
                    "(" | "[" => {
                        result.push_str(&text);
                        prev_was_open = true;
                    }
                    ")" | "]" | "." | "," | ":" | ";" | "!" | "?" => {
                        result.push_str(&text);
                        prev_was_open = false;
                    }
                    _ => {
                        let fmtd = self.format_text_node(&text);
                        let fmtd = match is_first_char_alnum(&fmtd) {
                            true  => format!("-{}", fmtd),
                            false => fmtd
                        };

                        match prev_was_open {
                            true => result.push_str(&fmtd),
                            false => {
                                let offset = if is_first_node { "" } else { self.formatting_state.spacing.as_str() };
                                result.push_str(&format!("{}{}", offset, fmtd));
                            }
                        }
                        prev_was_open = false;
                    }
                },
                _ => unreachable!("macro can't contain macro node or EOI!"),
            }

            if is_first_node {
                is_first_node = false;
            }
        }

        result
    }

    fn format_fn(&mut self, funcname: &str, macro_node: MacroNode) -> String {
        let mut result = format!("{funcname}(");
        let mut prev_was_open = false;
        let mut is_first_node = true;

        for node in macro_node.nodes {
            match node {
                Element::Text(text) => match text.as_str() {
                    "(" | "[" => {
                        result.push_str(&text);
                        prev_was_open = true;
                    }
                    ")" | "]" | "." | "," | ":" | ";" | "!" | "?" => {
                        result.push_str(&text);
                        prev_was_open = false;
                    }
                    _ => {
                        match prev_was_open {
                            true => result.push_str(&self.format_text_node(&text)),
                            false => {
                                let offset = if is_first_node { "" } else { self.formatting_state.spacing.as_str() };
                                let formatted_node = format!("{}{},", offset, self.format_text_node(&text));
                                result.push_str(&formatted_node);
                            }
                        }
                        prev_was_open = false;
                    }
                },
                _ => unreachable!("macro can't contain macro node or EOI!"),
            }

            if is_first_node {
                is_first_node = false;
            }
        }

        if result.ends_with(",") {
            result.pop();
        }

        result.push(')');

        result
    }

    fn format_fn_synopsis(&mut self, funcname: &str, macro_node: MacroNode) -> String {
        let mut result = format!("{funcname}(");
        let mut prev_was_open = false;
        let mut is_first_node = true;

        for node in macro_node.nodes {
            match node {
                Element::Text(text) => match text.as_str() {
                    "(" | "[" => {
                        result.push_str(&text);
                        prev_was_open = true;
                    }
                    ")" | "]" | "." | "," | ":" | ";" | "!" | "?" => {
                        result.push_str(&text);
                        prev_was_open = false;
                    }
                    _ => {
                        match prev_was_open {
                            true => result.push_str(&self.format_text_node(&text)),
                            false => {
                                let offset = if is_first_node { "" } else { self.formatting_state.spacing.as_str() };
                                let formatted_node = format!("{}{},", offset, self.format_text_node(&text));
                                result.push_str(&formatted_node);
                            }
                        }
                        prev_was_open = false;
                    }
                },
                _ => unreachable!("macro can't contain macro node or EOI!"),
            }

            if is_first_node {
                is_first_node = false;
            }   
        }

        if result.ends_with(",") {
            result.pop();
        }

        result.push_str(");\n");

        result
    }

    fn format_fr(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)

    }

    fn format_ft(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_ft_synopsis(&mut self, macro_node: MacroNode) -> String {
        let content = self.format_inline_macro(macro_node);
        
        format!("\n{}\n", content)
    }

    fn format_fx(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_hf(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)

    }

    fn format_ic(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)

    }

    fn format_in(&self, filename: &str, macro_node: MacroNode) -> String {
        println!("nodes: {:?}", macro_node.nodes.clone());

        let mut result = String::new();
        let mut iter = macro_node.nodes.into_iter();
        
        if let Some(node) = iter.next() {
            match node {
                Element::Text(open_del) => result.push_str(open_del.as_str()),
                _=> unreachable!()
            }
        }

        result.push_str(&format!("<{filename}>"));

        if let Some(node) = iter.next() {
            match node {
                Element::Text(close_del) => result.push_str(close_del.as_str()),
                _ => unreachable!()
            }
        }

        result
    }

    fn format_in_synopsis(&self, filename: &str, macro_node: MacroNode, prev_node: &Macro) -> String {
        let in_formatted = self.format_in(filename, macro_node);
        let start = match prev_node {
            Macro::Fn { .. } => "\n#include".to_string(),
            _ => "#include".to_string()
        };
    
        format!("{} {}\n", start, in_formatted)
    }

    fn format_lb(&self, lib_name: &str, macro_node: MacroNode) -> String {
        let mut result = String::new();
        let mut iter = macro_node.nodes.into_iter();
        
        if let Some(node) = iter.next() {
            match node {
                Element::Text(open_del) => result.push_str(open_del.as_str()),
                _=> unreachable!()
            }
        }

        result.push_str(&format!("library “{lib_name}”"));

        if let Some(node) = iter.next() {
            match node {
                Element::Text(close_del) => result.push_str(close_del.as_str()),
                _ => unreachable!()
            }
        }

        result
    }

    fn format_li(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)

    }

    fn format_lk(&mut self, uri: &str, macro_node: MacroNode) -> String {
        let content = macro_node
            .nodes
            .clone()
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing);

        format!("{content}: {uri}")
    }

    fn format_lp(&self) -> String {
        "\n \n".to_string()
    }

    fn format_ms(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_mt(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_no(&mut self, macro_node: MacroNode) -> String {
        self.formatting_state.suppress_space = false;
        self.format_inline_macro(macro_node)
    }

    fn format_ns(&mut self, macro_node: MacroNode) -> String {
        self.formatting_state.suppress_space = true;
        "\x08".to_string() + &self.format_inline_macro(macro_node)
    }

    fn format_nx(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_os(&mut self, macro_node: MacroNode) -> String {
        let content = macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing);

        if !content.is_empty() {
            self.formatting_state.footer_text = Some(content);
        }
        String::new()
    }

    fn format_ox(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_pa(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_pf(&mut self, prefix: &str, macro_node: MacroNode) -> String {
        // self.formatting_state.suppress_space = true;
        let c = self.format_inline_macro(macro_node);
        
        println!("Pf formatted nodes: {}", c);

        format!("{}{}", prefix, c)
        
    }

    fn format_pp(&self, _macro_node: MacroNode) -> String {
        "\n \n".to_string()
    }

    fn format_rv(&mut self, macro_node: MacroNode) -> String {
        let mut content = macro_node
            .nodes
            .clone()
            .into_iter()
            .take(macro_node.nodes.len().saturating_sub(1))
            .map(|node| self.format_node(node))
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>()
            .join("(), ");

        if macro_node.nodes.is_empty() {
            content = self.formatting_state.first_name.clone().unwrap_or_default();
        } else if let Some(formatted_node) = macro_node.nodes.iter().last() {
            let formatted_node = self.format_node(formatted_node.clone());
            if macro_node.nodes.len() == 1 {
                content = format!("{formatted_node}()");
            } else {
                content.push_str(&format!("(), and {formatted_node}()"));
            }
        }

        let ending_1 = if macro_node.nodes.len() <= 1 { "" } else { "s" };

        let ending_2 = if macro_node.nodes.len() <= 1 { "s" } else { "" };

        if !content.is_empty() {
            format!("The {content} function{ending_1} return{ending_2} the value 0 if successful; otherwise the value -1 is returned and the global variable errno is set to indicate the error.")
        } else {
            String::new()
        }
    }

    fn format_sm(&mut self, sm_mode: Option<SmMode>, macro_node: MacroNode) -> String {
        self.formatting_state.spacing = match sm_mode {
            Some(SmMode::On) => " ".to_string(),
            Some(SmMode::Off) => "".to_string(),
            None => match self.formatting_state.spacing.as_str() {
                "" => " ".to_string(),
                " " => "".to_string(),
                _ => " ".to_string(),
            },
        };

        let c= self.format_inline_macro(macro_node);

        format!("{}{}", self.formatting_state.spacing, c)
    }

    fn format_st(&self, st_type: StType, macro_node: MacroNode) -> String {
        let content = self.format_inline_macro(macro_node);

        format!("{} {}", st_type, content)
    }

    fn format_sx(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_sy(&mut self, macro_node: MacroNode) -> String {
        let line = self.format_inline_macro(macro_node);

        if self.supports_bold() {
            format!("\x1b[1m{line}\x1b[0m")
        } else {
            line
        }
    }

    fn format_tg(&self, _term: Option<String>) -> String {
        String::new()
    }

    fn format_tn(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_ud(&self) -> String {
        "currently under development.".to_string()
    }

    fn format_ux(&self, macro_node: MacroNode) -> String {
        let content = self.format_inline_macro(macro_node);

        format!("UNIX {content}")
    }

    fn format_va(&mut self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_xr(&self, name: &str, section: &str, macro_node: MacroNode) -> String {
        let content = self.format_inline_macro(macro_node);

        format!("{name}({section}){content}")
    }
}

fn is_first_char_alnum(s: &str) -> bool {
    s.chars().next().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use crate::{man_util::formatter::MdocDocument, FormattingSettings, MdocFormatter, MdocParser};

    const FORMATTING_SETTINGS: FormattingSettings = FormattingSettings {
        width: 78,
        indent: 5,
    };

    fn get_ast(input: &str) -> MdocDocument {
        MdocParser::parse_mdoc(input).unwrap()
    }

    fn test_formatting(input: &str, output: &str) {
        let ast = get_ast(input);

        let mut formatter = MdocFormatter::new(FORMATTING_SETTINGS);
        //println!("{:?}", formatter);
        let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
        println!("Formatted document:\nTarget:\n{}\n{}\nReal:\n{}\n", 
            output, 
            vec!['-';formatter.formatting_settings.width].iter().collect::<String>(), 
            result
        );
        assert_eq!(output, result)
    }

    mod special_chars {
        use crate::man_util::formatter::tests::test_formatting;

        #[test]
        fn spaces() {
            let input = r".Dd January 1, 1970
.Os footer text
\ \~\0\|\^\&\)\%\:";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

   

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn lines() {
            let input = r".Dd January 1, 1970
.Os footer text
\(ba \(br \(ul \(ru \(rn \(bb \(sl \(rs";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

| │ _ _ ‾ ¦ / \

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn text_markers() {
            let input = r".Dd January 1, 1970
.Os footer text
\(ci \(bu \(dd \(dg \(lz \(sq \(ps \(sc \(lh \(rh \(at \(sh \(CR \(OK \(CL \(SP \(HE \(DI";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

○ • ‡ † ◊ □ ¶ § ☜ ☞ @ # ↵ ✓ ♣ ♠ ♥ ♦

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn legal_symbols() {
            let input = r".Dd January 1, 1970
.Os footer text
\(co \(rg \(tm";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

© ® ™

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn punctuation() {
            let input = r".Dd January 1, 1970
.Os footer text
\(em \(en \(hy \e \(r! \(r?";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

— – ‐ \\ ¡ ¿

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn quotes() {
            let input = r".Dd January 1, 1970
.Os footer text
\(Bq \(bq \(lq \(rq \(oq \(cq \(aq \(dq \(Fo \(Fc \(fo \(fc";
            let output = "UNTITLED                             LOCAL                            UNTITLED

„ ‚ “ ” ‘ ’ ' \" « » ‹ ›

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn brackets() {
            let input = r".Dd January 1, 1970
.Os footer text
\(lB \(rB \(lC \(rC \(la \(ra \(bv \[braceex] \[bracketlefttp] \[bracketleftbt]
\[bracketleftex] \[bracketrighttp] \[bracketrightbt] \[bracketrightex]
\(lt \[bracelefttp] \(lk \[braceleftmid] \(lb \[braceleftbt] \[braceleftex]
\(rt \[bracerighttp] \(rk \[bracerightmid] \(rb \[bracerightbt] \[bracerightex]
\[parenlefttp] \[parenleftbt] \[parenleftex] \[parenrighttp] \[parenrightbt] \[parenrightex]
";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

[ ] { } ⟨ ⟩ ⎪ ⎪ ⎡ ⎣ ⎢ ⎤ ⎦ ⎥ ⎧ ⎧ ⎨ ⎨ ⎩ ⎩ ⎪ ⎫ ⎫ ⎬ ⎬ ⎭ ⎭ ⎪ ⎛ ⎝ ⎜ ⎞ ⎠ ⎟

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn arrows() {
            let input = r".Dd January 1, 1970
.Os footer text
\(<- \(-> \(<> \(da \(ua \(va \(lA \(rA \(hA \(uA \(dA \(vA \(an";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

← → ↔ ↓ ↑ ↕ ⇐ ⇒ ⇔ ⇑ ⇓ ⇕ ⎯

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn logical() {
            let input = r".Dd January 1, 1970
.Os footer text
\(AN \(OR \[tno] \(no \(te \(fa \(st \(tf \(3d \(or";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

∧ ∨ ¬ ¬ ∃ ∀ ∋ ∴ ∴ |

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn mathematical() {
            let input = r".Dd January 1, 1970
.Os footer text
\- \(mi \+ \(pl \(-+ \[t+-] \(+- \(pc \[tmu]
\(mu \(c* \(c+ \[tdi] \(di \(f/ \(** \(<= \(>= \(<< \(>> \(eq \(!= \(==
\(ne \(ap \(|= \(=~ \(~~ \(~= \(pt \(es \(mo \(nm \(sb \(nb \(sp
\(nc \(ib \(ip \(ca \(cu \(/_ \(pp \(is \[integral] \[sum] \[product]
\[coproduct] \(gr \(sr \[sqrt] \(lc \(rc \(lf \(rf \(if \(Ah \(Im \(Re
\(wp \(pd \(-h \[hbar] \(12 \(14 \(34 \(18 \(38 \(58 \(78 \(S1 \(S2 \(S3
";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

- − + + ∓ ± ± · × × ⊗ ⊕ ÷ ÷ ⁄ ∗ ≤ ≥ ≪ ≫ = ≠ ≡ ≢ ∼ ≃ ≅ ≈ ≈ ∝ ∅ ∈ ∉ ⊂ ⊄ ⊃ ⊅ ⊆ ⊇
∩ ∪ ∠ ⊥ ∫ ∫ ∑ ∏ ∐ ∇ √ √ ⌈ ⌉ ⌊ ⌋ ∞ ℵ ℑ ℜ ℘ ∂ ℏ ℏ ½ ¼ ¾ ⅛ ⅜ ⅝ ⅞ ¹ ² ³

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn ligatures() {
            let input = r".Dd January 1, 1970
.Os footer text
\(ff \(fi \(fl \(Fi \(Fl \(AE \(ae \(OE \(oe \(ss \(IJ \(ij";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

ﬀ ﬁ ﬂ ﬃ ﬄ Æ æ Œ œ ß Ĳ ĳ

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn accents() {
            let input = ".Dd January 1, 1970
.Os footer text
\\(a\" \\(a- \\(a. \\(a^ \\(aa \\\' \\(ga \\` \\(ab \\(ac \\(ad \\(ah \\(ao \\(a~ \\(ho \\(ha \\(ti";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

˝ ¯ ˙ ^ ´ ´ ` ` ˘ ¸ ¨ ˇ ˚ ~ ˛ ^ ~

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn accented_letters() {
            let input = r".Dd January 1, 1970
.Os footer text
\('A \('E \('I \('O \('U \('Y \('a \('e
\('i \('o \('u \('y \(`A \(`E \(`I \(`O \(`U \(`a \(`e \(`i \(`o \(`u
\(~A \(~N \(~O \(~a \(~n \(~o \(:A \(:E \(:I \(:O \(:U \(:a \(:e \(:i
\(:o \(:u \(:y \(^A \(^E \(^I \(^O \(^U \(^a \(^e \(^i \(^o \(^u \(,C
\(,c \(/L \(/l \(/O \(/o \(oA \(oa
";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

Á É Í Ó Ú Ý á é í ó ú ý À È Ì Ò Ù à è ì ò ù Ã Ñ Õ ã ñ õ Ä Ë Ï Ö Ü ä ë ï ö ü ÿ
Â Ê Î Ô Û â ê î ô û Ç ç Ł ł Ø ø Å å

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn special_letters() {
            let input = r".Dd January 1, 1970
.Os footer text
\(-D \(Sd \(TP \(Tp \(.i \(.j";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

Ð ð Þ þ ı ȷ

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn currency() {
            let input = r".Dd January 1, 1970
.Os footer text
\(Do \(ct \(Eu \(eu \(Ye \(Po \(Cs \(Fn";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

$ ¢ € € ¥ £ ¤ ƒ

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn units() {
            let input = r".Dd January 1, 1970
.Os footer text
\(de \(%0 \(fm \(sd \(mc \(Of \(Om";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

° ‰ ′ ″ µ ª º

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn greek_leters() {
            let input = r".Dd January 1, 1970
.Os footer text
\(*A \(*B \(*G \(*D \(*E \(*Z
\(*Y \(*H \(*I \(*K \(*L \(*M \(*N \(*C \(*O \(*P \(*R \(*S
\(*T \(*U \(*F \(*X \(*Q \(*W \(*a \(*b \(*g \(*d \(*e \(*z
\(*y \(*h \(*i \(*k \(*l \(*m \(*n \(*c \(*o \(*p \(*r \(*s
\(*t \(*u \(*f \(*x \(*q \(*w \(+h \(+f \(+p \(+e \(ts
";
            let output = r"UNTITLED                             LOCAL                            UNTITLED

Α Β Γ Δ Ε Ζ Η Θ Ι Κ Λ Μ Ν Ξ Ο Π Ρ Σ Τ Υ Φ Χ Ψ Ω α β γ δ ε ζ η θ ι κ λ μ ν ξ ο
π ρ σ τ υ ϕ χ ψ ω ϑ φ ϖ ϵ ς

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn predefined_strings() {
            let input = r".Dd January 1, 1970
.Os footer text
\*(Ba \*(Ne \*(Ge \*(Le \*(Gt \*(Lt \*(Pm \*(If \*(Pi \*(Na \*(Am \*R \*(Tm \*q \*(Rq \*(Lq \*(lp \*(rp \*(lq \*(rq \*(ua \*(va \*(<= \*(>= \*(aa \*(ga \*(Px \*(Ai";
            let output = "UNTITLED                             LOCAL                            UNTITLED

| ≠ ≥ ≤ > < ± infinity pi NaN & ® (Tm) \" ” “ ( ) “ ” ↑ ↕ ≤ ≥ ´ ` POSIX ANSI

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn unicode() {
            let input = r".Dd January 1, 1970
.Os footer text
\[u0100] \C'u01230' \[u025600]";
            let output = "UNTITLED                             LOCAL                            UNTITLED

Ā ሰ 𥘀

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn numbered() {
            let input = r".Dd January 1, 1970
.Os footer text
\N'34' \[char43]";
            let output = "UNTITLED                             LOCAL                            UNTITLED

\" +

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }
    }

    mod full_explicit {
        use crate::man_util::formatter::tests::test_formatting;

        mod bd{
            use crate::man_util::formatter::tests::test_formatting;

            #[test]
            fn bd_filled() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bd -filled -offset indent
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
.Ed";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

      Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod
      tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim
      veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea
      commodo consequat.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }
        
            #[test]
            fn bd_unfilled() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bd -unfilled -offset indent
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
.Ed";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

      Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod
      tempor incididunt ut labore et dolore magna aliqua.
      Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi
      ut aliquip ex ea commodo consequat.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bd_centered() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bd -centered -offset indent
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
.Ed";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

      Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod
        tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim
      veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea
                                 commodo consequat.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bd_offset_right() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bd -filled -offset right
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
.Ed";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

       Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod
          tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim
       veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea
                                                            commodo consequat.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bd_compact() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bd -literal -offset indent -compact
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
.Ed";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

      Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod
      tempor incididunt ut labore et dolore magna aliqua.
      Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi
      ut aliquip ex ea commodo consequat.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bd_nested_blocks() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bd -unfilled -offset indent
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
.Ed
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>
.Sh DESCRIPTION
.Ss SUBSECTION
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bd -unfilled -offset indent
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
.Bd -unfilled -offset indent
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
.Bd -unfilled -offset indent
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
.Bd -unfilled -offset indent
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
.Ed
.Ed
.Ed
.Ed
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>

      Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.
      Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg
g wefwefwer werwe rwe r wer <alpha>

DESCRIPTION
   SUBSECTION
     Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>

           Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.
           Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.

                 Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.
                 Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.

                       Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.
                       Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.

                             Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.
                             Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
     Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df
     gdfg dfg g wefwefwer werwe rwe r wer <alpha>

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }
        }

        #[test]
        fn bf() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bf -emphasis
Line 1
Line 2
.Ef";
            let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

\u{1b}[3mLine 1 Line 2 \u{1b}[0m

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn bf_macro() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bf Em
Line 1
Line 2
.Ef";
            let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

\u{1b}[3mLine 1 Line 2 \u{1b}[0m

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn bk() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bk -words
Line 1
Line 2
.Ek";
            let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Line 1 Line 2

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        mod bl{
            use crate::man_util::formatter::tests::test_formatting;

            #[test]
            fn bl_bullet() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bl -bullet -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

•       Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
        eiusmod tempor incididunt ut labore et dolore magna aliqua.
•       Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris
        nisi ut aliquip ex ea commodo consequat.
•       Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
        dolore eu fugiat nulla pariatur.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_column() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1  
.Os footer text
.Bl -column -width 8 -compact col1 col2 col3
.It Cell 1 Ta Cell 2 Ta Cell 3
Line 1
.It Cell 4 Ta Cell 5 Ta Cell 6
Line 2
.It Cell 7 Ta Cell 8 Ta Cell 9
Line 3
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Cell 1  Cell 2  Cell 3 Line 1
Cell 4  Cell 5  Cell 6 Line 2
Cell 7  Cell 8  Cell 9 Line 3

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_column_long_content() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bl -column -width 8 -compact col1 col2 col3
.It AAAAAA AAAAAAAAAAAA AAAAA Ta BBBBBB BBBBBBBBB BBBBBB Ta CCCCCC CCCCCCCCCC CCCCCCC
Line 1
.It DDDDDD DDDDDDDDDDDD DDDDD Ta EEEEEE EEEEEEEEE EEEEEE Ta FFFFFF FFFFFFFFFF FFFFFFF
Line 2
.It RRRRRR RRRRRRRRRRRR RRRRR Ta VVVVVV VVVVVVVVV VVVVVV Ta WWWWWW WWWWWWWWWW WWWWWWW
Line 3
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

AAAAAA AAAAAAAAAAAA AAAAA
        BBBBBB BBBBBBBBB BBBBBB
                CCCCCC CCCCCCCCCC CCCCCCC Line 1
DDDDDD DDDDDDDDDDDD DDDDD
        EEEEEE EEEEEEEEE EEEEEE
                FFFFFF FFFFFFFFFF FFFFFFF Line 2
RRRRRR RRRRRRRRRRRR RRRRR
        VVVVVV VVVVVVVVV VVVVVV
                WWWWWW WWWWWWWWWW WWWWWWW Line 3

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }
    
            #[test]
            fn bl_dash() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bl -dash -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

-       Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
        eiusmod tempor incididunt ut labore et dolore magna aliqua.
-       Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris
        nisi ut aliquip ex ea commodo consequat.
-       Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
        dolore eu fugiat nulla pariatur.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }
    
            #[test]
            fn bl_diag() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bl -diag -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

head1  Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
eiusmod tempor incididunt ut labore et dolore magna aliqua.
head2  Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris
nisi ut aliquip ex ea commodo consequat.
head3  Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
dolore eu fugiat nulla pariatur.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_enum() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bl -enum -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

1.      Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
        eiusmod tempor incididunt ut labore et dolore magna aliqua.
2.      Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris
        nisi ut aliquip ex ea commodo consequat.
3.      Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
        dolore eu fugiat nulla pariatur.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_item() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bl -item -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor
incididunt ut labore et dolore magna aliqua.
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut
aliquip ex ea commodo consequat.
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore
eu fugiat nulla pariatur.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_hang() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bl -hang -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

head1   Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
        eiusmod tempor incididunt ut labore et dolore magna aliqua.
head2   Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris
        nisi ut aliquip ex ea commodo consequat.
head3   Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
        dolore eu fugiat nulla pariatur.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }
    
            #[test]
            fn bl_inset() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bl -inset -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

head1 Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod
tempor incididunt ut labore et dolore magna aliqua.
head2 Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi
ut aliquip ex ea commodo consequat.
head3 Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
dolore eu fugiat nulla pariatur.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }
    
            #[test]
            fn bl_ohang() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bl -ohang -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor
incididunt ut labore et dolore magna aliqua.
head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut
aliquip ex ea commodo consequat.
head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore
eu fugiat nulla pariatur.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }
    
            #[test]
            fn bl_tag() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bl -tag -width 12 -compact
.It head1 
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

head1       Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
            eiusmod tempor incididunt ut labore et dolore magna aliqua.
head2       Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris
            nisi ut aliquip ex ea commodo consequat.
head3       Duis aute irure dolor in reprehenderit in voluptate velit esse
            cillum dolore eu fugiat nulla pariatur.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_hang_long_head() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bl -hang -width 8 -compact
.It Item head title1 
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It Item head title2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It Item head title3 
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Item head title1 Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed
        do eiusmod tempor incididunt ut labore et dolore magna aliqua.
Item head title2 Ut enim ad minim veniam, quis nostrud exercitation ullamco
        laboris nisi ut aliquip ex ea commodo consequat.
Item head title3 Duis aute irure dolor in reprehenderit in voluptate velit
        esse cillum dolore eu fugiat nulla pariatur.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_inset_long_head() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bl -inset -width 8 -compact
.It Item head title1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It Item head title2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It Item head title3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Item head title1 Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed
do eiusmod tempor incididunt ut labore et dolore magna aliqua.
Item head title2 Ut enim ad minim veniam, quis nostrud exercitation ullamco
laboris nisi ut aliquip ex ea commodo consequat.
Item head title3 Duis aute irure dolor in reprehenderit in voluptate velit
esse cillum dolore eu fugiat nulla pariatur.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_ohang_long_head() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bl -ohang -width 8 -compact
.It Item head title1 
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It Item head title2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It Item head title3 
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Item head title1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor
incididunt ut labore et dolore magna aliqua.
Item head title2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut
aliquip ex ea commodo consequat.
Item head title3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore
eu fugiat nulla pariatur.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_tag_long_head() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
.Bl -tag -width 8 -compact
.It Item head title1 
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It Item head title2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It Item head title3 
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Item head title1
        Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
        eiusmod tempor incididunt ut labore et dolore magna aliqua.
Item head title2
        Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris
        nisi ut aliquip ex ea commodo consequat.
Item head title3
        Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
        dolore eu fugiat nulla pariatur.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_symbol_nested_lists() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -bullet -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>
.Sh DESCRIPTION
.Ss SUBSECTION
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -bullet -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.It head4
.Bl -bullet -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.It head4
.Bl -bullet -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El
.El
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
•       Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
        eiusmod tempor incididunt ut labore et dolore magna aliqua.
•       Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris
        nisi ut aliquip ex ea commodo consequat.
•       Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
        dolore eu fugiat nulla pariatur.
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg
g wefwefwer werwe rwe r wer <alpha>

DESCRIPTION
   SUBSECTION
     Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
     •       Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
             eiusmod tempor incididunt ut labore et dolore magna aliqua.
     •       Ut enim ad minim veniam, quis nostrud exercitation ullamco
             laboris nisi ut aliquip ex ea commodo consequat.
     •       Duis aute irure dolor in reprehenderit in voluptate velit esse
             cillum dolore eu fugiat nulla pariatur.
     •
             •       Lorem ipsum dolor sit amet, consectetur adipiscing elit,
                     sed do eiusmod tempor incididunt ut labore et dolore
                     magna aliqua.
             •       Ut enim ad minim veniam, quis nostrud exercitation
                     ullamco laboris nisi ut aliquip ex ea commodo consequat.
             •       Duis aute irure dolor in reprehenderit in voluptate velit
                     esse cillum dolore eu fugiat nulla pariatur.
             •
                     •       Lorem ipsum dolor sit amet, consectetur
                             adipiscing elit, sed do eiusmod tempor incididunt
                             ut labore et dolore magna aliqua.
                     •       Ut enim ad minim veniam, quis nostrud
                             exercitation ullamco laboris nisi ut aliquip ex
                             ea commodo consequat.
                     •       Duis aute irure dolor in reprehenderit in
                             voluptate velit esse cillum dolore eu fugiat
                             nulla pariatur.
     Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df
     gdfg dfg g wefwefwer werwe rwe r wer <alpha>

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_item_nested_lists() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -item -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>
.Sh DESCRIPTION
.Ss SUBSECTION
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -item -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.It head4
.Bl -item -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.It head4
.Bl -item -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El
.El
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor
incididunt ut labore et dolore magna aliqua.
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut
aliquip ex ea commodo consequat.
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore
eu fugiat nulla pariatur.
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg
g wefwefwer werwe rwe r wer <alpha>

DESCRIPTION
   SUBSECTION
     Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
     Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod
     tempor incididunt ut labore et dolore magna aliqua.
     Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi
     ut aliquip ex ea commodo consequat.
     Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
     dolore eu fugiat nulla pariatur.
     Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod
     tempor incididunt ut labore et dolore magna aliqua.
     Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi
     ut aliquip ex ea commodo consequat.
     Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
     dolore eu fugiat nulla pariatur.
     Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod
     tempor incididunt ut labore et dolore magna aliqua.
     Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi
     ut aliquip ex ea commodo consequat.
     Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
     dolore eu fugiat nulla pariatur.
     Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df
     gdfg dfg g wefwefwer werwe rwe r wer <alpha>

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_ohang_nested_lists() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -ohang -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>
.Sh DESCRIPTION
.Ss SUBSECTION
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -ohang -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.It head4
.Bl -ohang -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.It head4
.Bl -ohang -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El
.El
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor
incididunt ut labore et dolore magna aliqua.
head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut
aliquip ex ea commodo consequat.
head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore
eu fugiat nulla pariatur.
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg
g wefwefwer werwe rwe r wer <alpha>

DESCRIPTION
   SUBSECTION
     Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
     head1
     Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod
     tempor incididunt ut labore et dolore magna aliqua.
     head2
     Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi
     ut aliquip ex ea commodo consequat.
     head3
     Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
     dolore eu fugiat nulla pariatur.
     head4
     head1
     Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod
     tempor incididunt ut labore et dolore magna aliqua.
     head2
     Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi
     ut aliquip ex ea commodo consequat.
     head3
     Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
     dolore eu fugiat nulla pariatur.
     head4
     head1
     Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod
     tempor incididunt ut labore et dolore magna aliqua.
     head2
     Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi
     ut aliquip ex ea commodo consequat.
     head3
     Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
     dolore eu fugiat nulla pariatur.
     Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df
     gdfg dfg g wefwefwer werwe rwe r wer <alpha>

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_inset_nested_lists() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -inset -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>
.Sh DESCRIPTION
.Ss SUBSECTION
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -inset -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.It head4
.Bl -inset -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.It head4
.Bl -inset -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El
.El
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
head1 Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod
tempor incididunt ut labore et dolore magna aliqua.
head2 Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi
ut aliquip ex ea commodo consequat.
head3 Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
dolore eu fugiat nulla pariatur.
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg
g wefwefwer werwe rwe r wer <alpha>

DESCRIPTION
   SUBSECTION
     Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
     head1 Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
     eiusmod tempor incididunt ut labore et dolore magna aliqua.
     head2 Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris
     nisi ut aliquip ex ea commodo consequat.
     head3 Duis aute irure dolor in reprehenderit in voluptate velit esse
     cillum dolore eu fugiat nulla pariatur.
     head4 
     head1 Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
     eiusmod tempor incididunt ut labore et dolore magna aliqua.
     head2 Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris
     nisi ut aliquip ex ea commodo consequat.
     head3 Duis aute irure dolor in reprehenderit in voluptate velit esse
     cillum dolore eu fugiat nulla pariatur.
     head4 
     head1 Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
     eiusmod tempor incididunt ut labore et dolore magna aliqua.
     head2 Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris
     nisi ut aliquip ex ea commodo consequat.
     head3 Duis aute irure dolor in reprehenderit in voluptate velit esse
     cillum dolore eu fugiat nulla pariatur.
     Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df
     gdfg dfg g wefwefwer werwe rwe r wer <alpha>

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_column_nested_lists() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -column -width 8 -compact col1 col2 col3 col4
.It head1 Ta Lorem ipsum dolor sit amet, Ta consectetur adipiscing elit, Ta sed do eiusmod tempor incididunt ut Ta labore et dolore magna aliqua. 
.It head2 Ta Ut enim ad minim veniam, Ta quis nostrud exercitation ullamco Ta laboris nisi ut aliquip ex Ta ea commodo consequat. 
.It head3 Ta Duis aute irure dolor in Ta reprehenderit in voluptate velit Ta esse cillum dolore eu Ta fugiat nulla pariatur. 
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>
.Sh DESCRIPTION
.Ss SUBSECTION
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -column -width 8 -compact col1 col2 col3 col4
.It head1 Ta Lorem ipsum dolor sit amet, Ta consectetur adipiscing elit, Ta sed do eiusmod tempor incididunt ut Ta labore et dolore magna aliqua. 
.It head2 Ta Ut enim ad minim veniam, Ta quis nostrud exercitation ullamco Ta laboris nisi ut aliquip ex Ta ea commodo consequat. 
.It head3 Ta Duis aute irure dolor in Ta reprehenderit in voluptate velit Ta esse cillum dolore eu Ta fugiat nulla pariatur. 
.It head4
.Bl -column -width 8 -compact col1 col2 col3 col4
.It head1 Ta Lorem ipsum dolor sit amet, Ta consectetur adipiscing elit, Ta sed do eiusmod tempor incididunt ut Ta labore et dolore magna aliqua. 
.It head2 Ta Ut enim ad minim veniam, Ta quis nostrud exercitation ullamco Ta laboris nisi ut aliquip ex Ta ea commodo consequat. 
.It head3 Ta Duis aute irure dolor in Ta reprehenderit in voluptate velit Ta esse cillum dolore eu Ta fugiat nulla pariatur. 
.It head4
.Bl -column -width 8 -compact col1 col2 col3 col4
.It head1 Ta Lorem ipsum dolor sit amet, Ta consectetur adipiscing elit, Ta sed do eiusmod tempor incididunt ut Ta labore et dolore magna aliqua. 
.It head2 Ta Ut enim ad minim veniam, Ta quis nostrud exercitation ullamco Ta laboris nisi ut aliquip ex Ta ea commodo consequat. 
.It head3 Ta Duis aute irure dolor in Ta reprehenderit in voluptate velit Ta esse cillum dolore eu Ta fugiat nulla pariatur. 
.El
.El
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
head1   Lorem ipsum dolor sit amet,
                consectetur adipiscing elit,
                        sed do eiusmod tempor incididunt ut
                                labore et dolore magna aliqua.
head2   Ut enim ad minim veniam,
                quis nostrud exercitation ullamco
                        laboris nisi ut aliquip ex
                                ea commodo consequat.
head3   Duis aute irure dolor in
                reprehenderit in voluptate velit
                        esse cillum dolore eu
                                fugiat nulla pariatur.
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg
g wefwefwer werwe rwe r wer <alpha>

DESCRIPTION
   SUBSECTION
     Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
     head1   Lorem ipsum dolor sit amet,
                     consectetur adipiscing elit,
                             sed do eiusmod tempor incididunt ut
                                     labore et dolore magna aliqua.
     head2   Ut enim ad minim veniam,
                     quis nostrud exercitation ullamco
                             laboris nisi ut aliquip ex
                                     ea commodo consequat.
     head3   Duis aute irure dolor in
                     reprehenderit in voluptate velit
                             esse cillum dolore eu
                                     fugiat nulla pariatur.
     head4
     head1   Lorem ipsum dolor sit amet,
                     consectetur adipiscing elit,
                             sed do eiusmod tempor incididunt ut
                                     labore et dolore magna aliqua.
     head2   Ut enim ad minim veniam,
                     quis nostrud exercitation ullamco
                             laboris nisi ut aliquip ex
                                     ea commodo consequat.
     head3   Duis aute irure dolor in
                     reprehenderit in voluptate velit
                             esse cillum dolore eu
                                     fugiat nulla pariatur.
     head4
     head1   Lorem ipsum dolor sit amet,
                     consectetur adipiscing elit,
                             sed do eiusmod tempor incididunt ut
                                     labore et dolore magna aliqua.
     head2   Ut enim ad minim veniam,
                     quis nostrud exercitation ullamco
                             laboris nisi ut aliquip ex
                                     ea commodo consequat.
     head3   Duis aute irure dolor in
                     reprehenderit in voluptate velit
                             esse cillum dolore eu
                                     fugiat nulla pariatur.


     Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df
     gdfg dfg g wefwefwer werwe rwe r wer <alpha>

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_tag_nested_lists() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -tag -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>
.Sh DESCRIPTION
.Ss SUBSECTION
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -tag -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.It head4
.Bl -tag -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.It head4
.Bl -tag -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El
.El
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
head1   Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
        eiusmod tempor incididunt ut labore et dolore magna aliqua.
head2   Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris
        nisi ut aliquip ex ea commodo consequat.
head3   Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
        dolore eu fugiat nulla pariatur.
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg
g wefwefwer werwe rwe r wer <alpha>

DESCRIPTION
   SUBSECTION
     Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
     head1   Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
             eiusmod tempor incididunt ut labore et dolore magna aliqua.
     head2   Ut enim ad minim veniam, quis nostrud exercitation ullamco
             laboris nisi ut aliquip ex ea commodo consequat.
     head3   Duis aute irure dolor in reprehenderit in voluptate velit esse
             cillum dolore eu fugiat nulla pariatur.
     head4
             head1   Lorem ipsum dolor sit amet, consectetur adipiscing elit,
                     sed do eiusmod tempor incididunt ut labore et dolore
                     magna aliqua.
             head2   Ut enim ad minim veniam, quis nostrud exercitation
                     ullamco laboris nisi ut aliquip ex ea commodo consequat.
             head3   Duis aute irure dolor in reprehenderit in voluptate velit
                     esse cillum dolore eu fugiat nulla pariatur.
             head4
                     head1   Lorem ipsum dolor sit amet, consectetur
                             adipiscing elit, sed do eiusmod tempor incididunt
                             ut labore et dolore magna aliqua.
                     head2   Ut enim ad minim veniam, quis nostrud
                             exercitation ullamco laboris nisi ut aliquip ex
                             ea commodo consequat.
                     head3   Duis aute irure dolor in reprehenderit in
                             voluptate velit esse cillum dolore eu fugiat
                             nulla pariatur.
     Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df
     gdfg dfg g wefwefwer werwe rwe r wer <alpha>

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_hang_nested_lists() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -hang -width 8 -compact
.It Item head title1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It Item head title2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It Item head title3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>
.Sh DESCRIPTION
.Ss SUBSECTION
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -hang -width 8 -compact
.It Item head title1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It Item head title2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It Item head title3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.It Item head title4
.Bl -hang -width 8 -compact
.It Item head title1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It Item head title2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It Item head title3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.It Item head title4
.Bl -hang -width 8 -compact
.It Item head title1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It Item head title2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It Item head title3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El
.El
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
Item head title1 Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed
        do eiusmod tempor incididunt ut labore et dolore magna aliqua.
Item head title2 Ut enim ad minim veniam, quis nostrud exercitation ullamco
        laboris nisi ut aliquip ex ea commodo consequat.
Item head title3 Duis aute irure dolor in reprehenderit in voluptate velit
        esse cillum dolore eu fugiat nulla pariatur.
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg
g wefwefwer werwe rwe r wer <alpha>

DESCRIPTION
   SUBSECTION
     Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
     Item head title1 Lorem ipsum dolor sit amet, consectetur adipiscing elit,
             sed do eiusmod tempor incididunt ut labore et dolore magna
             aliqua.
     Item head title2 Ut enim ad minim veniam, quis nostrud exercitation
             ullamco laboris nisi ut aliquip ex ea commodo consequat.
     Item head title3 Duis aute irure dolor in reprehenderit in voluptate
             velit esse cillum dolore eu fugiat nulla pariatur.
     Item head title4
             Item head title1 Lorem ipsum dolor sit amet, consectetur
                     adipiscing elit, sed do eiusmod tempor incididunt ut
                     labore et dolore magna aliqua.
             Item head title2 Ut enim ad minim veniam, quis nostrud
                     exercitation ullamco laboris nisi ut aliquip ex ea
                     commodo consequat.
             Item head title3 Duis aute irure dolor in reprehenderit in
                     voluptate velit esse cillum dolore eu fugiat nulla
                     pariatur.
             Item head title4
                     Item head title1 Lorem ipsum dolor sit amet, consectetur
                             adipiscing elit, sed do eiusmod tempor incididunt
                             ut labore et dolore magna aliqua.
                     Item head title2 Ut enim ad minim veniam, quis nostrud
                             exercitation ullamco laboris nisi ut aliquip ex
                             ea commodo consequat.
                     Item head title3 Duis aute irure dolor in reprehenderit
                             in voluptate velit esse cillum dolore eu fugiat
                             nulla pariatur.
     Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df
     gdfg dfg g wefwefwer werwe rwe r wer <alpha>

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn bl_mixed_nested_lists() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME 1
.Os footer text
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -bullet -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>
.Sh DESCRIPTION
.Ss SUBSECTION
Adssdf sdfmsdpf  sdfm sdfmsdpf
.Ms <alpha>
.Bl -bullet -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.It head4
.Bl -hang -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.It head4
.Bl -tag -width 8 -compact
.It head1
Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. 
.It head2
Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. 
.It head3
Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
.El
.El
.El
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg g wefwefwer werwe rwe r wer 
.Ms <alpha>";
                let output = "PROGNAME(1)                 General Commands Manual                PROGNAME(1)

Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
•       Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
        eiusmod tempor incididunt ut labore et dolore magna aliqua.
•       Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris
        nisi ut aliquip ex ea commodo consequat.
•       Duis aute irure dolor in reprehenderit in voluptate velit esse cillum
        dolore eu fugiat nulla pariatur.
Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df gdfg dfg
g wefwefwer werwe rwe r wer <alpha>

DESCRIPTION
   SUBSECTION
     Adssdf sdfmsdpf  sdfm sdfmsdpf <alpha>
     •       Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do
             eiusmod tempor incididunt ut labore et dolore magna aliqua.
     •       Ut enim ad minim veniam, quis nostrud exercitation ullamco
             laboris nisi ut aliquip ex ea commodo consequat.
     •       Duis aute irure dolor in reprehenderit in voluptate velit esse
             cillum dolore eu fugiat nulla pariatur.
     •
             head1   Lorem ipsum dolor sit amet, consectetur adipiscing elit,
                     sed do eiusmod tempor incididunt ut labore et dolore
                     magna aliqua.
             head2   Ut enim ad minim veniam, quis nostrud exercitation
                     ullamco laboris nisi ut aliquip ex ea commodo consequat.
             head3   Duis aute irure dolor in reprehenderit in voluptate velit
                     esse cillum dolore eu fugiat nulla pariatur.
             head4
                     head1   Lorem ipsum dolor sit amet, consectetur
                             adipiscing elit, sed do eiusmod tempor incididunt
                             ut labore et dolore magna aliqua.
                     head2   Ut enim ad minim veniam, quis nostrud
                             exercitation ullamco laboris nisi ut aliquip ex
                             ea commodo consequat.
                     head3   Duis aute irure dolor in reprehenderit in
                             voluptate velit esse cillum dolore eu fugiat
                             nulla pariatur.
     Adssdf sdfmsdpf  sdfm sdfmsdpf sgsdgsdg sdfg sdfg sdfg fdsg d gdfg df
     gdfg dfg g wefwefwer werwe rwe r wer <alpha>

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }
        }
    }

    mod full_implicit {
        use crate::man_util::formatter::tests::test_formatting;

        #[test]
        fn it() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Bl -bullet
.It 
Line 1
.It 
Line 2
.El";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)


footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn nd() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Nd short description of the manual";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

– short description of the manual

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn nm() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Nm command_name";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

command_name

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn sh() {
            let input = 
".Dd $Mdocdate: October 28 2016 $
.Dt REV 1
.Os
.Sh NAME
.Nm rev
.Nd reverse lines of a file
.Sh SYNOPSIS
.Nm rev
.Op Ar
.Sh DESCRIPTION
The
.Nm rev
utility copies the specified files to the standard output, reversing the
order of characters in every line.
If no files are specified, the standard input is read.";
            let output =
"REV(1)                      General Commands Manual                     REV(1)

NAME
     rev – reverse lines of a file

SYNOPSIS
     rev [file ...]

DESCRIPTION
     The rev utility copies the specified files to the standard output,
     reversing the order of characters in every line.  If no files are
     specified, the standard input is read.

Debian                         October 28, 2016                         Debian";
            test_formatting(input, output);
        }

        #[test]
        fn ss() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ss Options
These are the available options.";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

   Options
     These are the available options.

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }
    }

    #[test]
    fn ta() {
        let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Bl -column \"A col\" \"B col\"
.It item1 Ta item2
.It item1 Ta item2
.El";
        let output = "PROGNAME(section)                   section                  PROGNAME(section)

item1  item2
item1  item2

footer text                     January 1, 1970                    footer text";
        test_formatting(input, output);
    }

    mod inline {
        use crate::man_util::formatter::tests::test_formatting;

        mod rs_submacro {
            use super::*;

            #[test]
            fn a() {
                let input = r".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%A author name
.Re
.Rs
.%A author name1
.%A author name2
.Re
.Rs
.%A author name1
.%A author name2
.%A author name3
.Re
.Rs
.%A ( author ) name1
.%A author , name2
.%A author name3 !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

author name. author name1 and author name2. author name1, author name2, and
author name3. (author) name1, author, name2, and author name3!.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn b() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%B book title
.Re
.Rs
.%B book title
.%B book title
.Re
.Rs
.%B ( book ) title
.%B book , title
.%B book title !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

book title. book title, book title. (book) title, book, title, book title!.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn c() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%C Publication city
.Re
.Rs
.%C Publication city
.%C Publication city
.Re
.Rs
.%C ( Publication ) city
.%C Publication , city
.%C Publication city !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

Publication city. Publication city, Publication city. (Publication) city,
Publication, city, Publication city!.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn d() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%D January 1, 1970
.Re
.Rs
.%D January 1 1970
.%D first january 1970
.Re
.Rs
.%D ( March ) 1189
.%D 12 , 1900
.%D 12 of March, 1970 !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

January 1, 1970. January 1 1970, first january 1970. (March) 1189, 12, 1900,
12 of March, 1970!.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn i() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%I issuer name
.Re
.Rs
.%I issuer name
.%I issuer name
.Re
.Rs
.%I ( issuer ) name
.%I issuer , name
.%I issuer name !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

issuer name. issuer name, issuer name. (issuer) name, issuer, name, issuer
name!.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn j() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%J Journal name
.Re
.Rs
.%J Journal name
.%J Journal name
.Re
.Rs
.%J ( Journal ) name
.%J Journal , name
.%J Journal name !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

Journal name. Journal name, Journal name. (Journal) name, Journal, name,
Journal name!.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn n() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%N Issue number
.Re
.Rs
.%N Issue number
.%N Issue number
.Re
.Rs
.%N ( Issue ) number
.%N Issue , number
.%N Issue number !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

Issue number. Issue number, Issue number. (Issue) number, Issue, number,
Issue number!.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn o() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%O Optional information
.Re
.Rs
.%O Optional information
.%O Optional information
.Re
.Rs
.%O ( Optional ) information
.%O Optional , information
.%O Optional information !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

Optional information. Optional information, Optional information. (Optional)
information, Optional, information, Optional information!.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn p() {
                let input = r".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%P pp. 42\(en47
.Re
.Rs
.%P pp. 42\(en47
.%P p. 42
.Re
.Rs
.%P ( p. 42 ) p. 43
.%P pp. 42 , 47
.%P pp. 42\(en47 !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

pp. 42–47. pp. 42–47, p. 42. (p. 42) p. 43, pp. 42, 47, pp. 42–47!.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn q() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%Q Institutional author
.Re
.Rs
.%Q Institutional author
.%Q Institutional author
.Re
.Rs
.%Q ( Institutional ) author
.%Q Institutional , author
.%Q Institutional author !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

Institutional author. Institutional author, Institutional author.
(Institutional) author, Institutional, author, Institutional author!.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn r() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%R Technical report
.Re
.Rs
.%R Technical report
.%R Technical report
.Re
.Rs
.%R ( Technical report ) Technical report
.%R Technical report , Technical report
.%R Technical report !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

Technical report. Technical report, Technical report. (Technical report)
Technical report, Technical report, Technical report, Technical report!.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn t() {
                let input = r".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%T Article title
.Re
.Rs
.%T Article title
.%T Article title
.Re
.Rs
.%T ( Article title ) Article title
.%T Article title , Article title
.%T Article title !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

Article title. Article title, Article title. (Article title) Article title,
Article title, Article title, Article title!.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn u() {
                let input = r".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%U Article title
.Re
.Rs
.%U Article title
.%U Article title
.Re
.Rs
.%U ( Article title ) Article title
.%U Article title , Article title
.%U Article title !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

Article title. Article title, Article title. (Article title) Article title,
Article title, Article title, Article title!.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn v() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%V Volume number
.Re
.Rs
.%V Volume number
.%V Volume number
.Re
.Rs
.%V ( Volume number ) Volume number
.%V Volume number , Volume number
.%V Volume number !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

Volume number. Volume number, Volume number. (Volume number) Volume number,
Volume number, Volume number, Volume number!.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }
        }

        #[test]
        fn ad() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ad [0,$]
.Ad 0x00000000
.Ad [ 0,$ ]";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

[0,$] 0x00000000 [0,$]

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn an() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os Debian
.An Kristaps
.An Kristaps
.An Kristaps
.An -split
.An Kristaps
.An Kristaps
.An -nosplit
.An Kristaps
.An Kristaps";
            let output = "PROGNAME(section)                   section                  PROGNAME(section)

Kristaps Kristaps Kristaps
Kristaps
Kristaps Kristaps Kristaps

Debian                          January 1, 1970                         Debian";
            test_formatting(input, output);
        }

        #[test]
        fn ap() {
            let input = 
".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ap Text Line Ns addr";
            let output =
"PROGNAME(section)                   section                  PROGNAME(section)

'Text Lineaddr

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn ar() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ar
.Ar arg1 , arg2 .";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

file ... arg1, arg2.

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn at() {
            let input = 
".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.At
.At III
.At V.1
.At ( V.1 )
.At ( V.1 ) subnode Ad ( addr )";
            let output =
"PROGNAME(section)                   section                  PROGNAME(section)

AT&T UNIX AT&T System III UNIX AT&T System V Release 1 UNIX (AT&T System V
Release 1 UNIX) (AT&T System V Release 1 UNIX) subnode (addr)

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn bsx() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Bsx 1.0
.Bsx
.Bsx ( 1.0 )";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

BSD/OS 1.0 BSD/OS (BSD/OS 1.0)

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn bt() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Bt";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

is currently in beta test.

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn bx() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Bx 4.3 Tahoe
.Bx 4.4
.Bx
.Bx ( 4.3 Tahoe )";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

4.3BSD-Tahoe 4.4BSD BSD (4.3BSD-Tahoe)

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn cd() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Cd device le0 at scode?";

            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

device le0 at scode?

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn cm() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Cm file bind";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

file bind

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn db() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Db
";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)


footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn dd() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)


footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn dt() {
            let input = ".Dd January 1, 1970
.Dt TITLE 7 arch
.Os footer text";
            let output =
                "TITLE(7)            Miscellaneous Information Manual (arch)           TITLE(7)


footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn dv() {
            let input = ".Dd January 1, 1970
.Dt TITLE 7 arch
.Os footer text
.Dv NULL
.Dv BUFSIZ
.Dv STDOUT_FILEnmo";
            let output =
                "TITLE(7)            Miscellaneous Information Manual (arch)           TITLE(7)

NULL BUFSIZ STDOUT_FILEnmo

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn dx() {
            let input = ".Dd January 1, 1970
.Dt TITLE 7 arch
.Os footer text
.Dx 2.4.1
.Dx ( 2.4.1 )
";
            let output =
                "TITLE(7)            Miscellaneous Information Manual (arch)           TITLE(7)

DragonFly 2.4.1 (DragonFly 2.4.1)

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn em() {
            let input = ".Dd January 1, 1970
.Dt TITLE 7 arch
.Os footer text
Selected lines are those
.Em not
matching any of the specified patterns.
Some of the functions use a
.Em hold space
to save the pattern space for subsequent retrieval.";
            let output =
                "TITLE(7)            Miscellaneous Information Manual (arch)           TITLE(7)

Selected lines are those \u{1b}[3mnot\u{1b}[0m matching any of the specified patterns.
Some of the functions use a \u{1b}[3mhold space\u{1b}[0m to save the pattern space for
subsequent retrieval.

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn er() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Er ERROR ERROR2";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

ERROR ERROR2

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn es() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Es ( )";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

()

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn ev() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ev DISPLAY";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

DISPLAY

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn ex() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ex -std grep";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

The grep utility exits 0 on success, and >0 if an error occurs.

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn fa() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Fa funcname Ft const char *";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

funcname const char *

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn fd() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Fd #define sa_handler __sigaction_u.__sa_handler";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

#define sa_handler __sigaction_u.__sa_handler

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn fl() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Fl H | L | P inet";
            let output =
"PROGNAME(section)                   section                  PROGNAME(section)

-H | -L | -P -inet

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[allow(non_snake_case)]
        #[test]
        fn Fn() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Fn funcname arg arg2 arg3";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

funcname(arg arg2 arg3)

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn fr() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Fr 32";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

32

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn ft() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ft int32 void";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

int32 void

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn fx() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Fx 1.0";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

FreeBSD 1.0

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn hf() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Hf file/path file2/path";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

file/path file2/path

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn ic() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ic :wq";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

:wq

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[allow(non_snake_case)]
        #[test]
        fn In() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.In stdatomic.h";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

<stdatomic.h>

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn lb() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Lb libname";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

library “libname”

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn li() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Li Book Antiqua";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

Book Antiqua

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn lk() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Lk https://bsd.lv The BSD.lv Project";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

The BSD.lv Project: https://bsd.lv

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn lp() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Hf file/path file2/path
.Lp
.Lk https://bsd.lv The BSD.lv Project";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

file/path file2/path
 
The BSD.lv Project: https://bsd.lv

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn ms() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ms alpha beta";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

alpha beta

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn mt() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Mt abc@gmail.com abc@gmail.com";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

abc@gmail.com abc@gmail.com

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn nm() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Nm command_name";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

command_name

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn no() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.No a b c";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

a b c

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn ns() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ar name Ns = Ns Ar value
.Cm :M Ns Ar pattern
.Fl o Ns Ar output
.No a b c
.Ns
.No a b c";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

name=value :Mpattern -ooutput a b ca b c

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn nx() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Nx Version 1.0";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

NetBSD Version 1.0

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn os() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)


footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn ot() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ot functype";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

functype

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn ox() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ox Version 1.0";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

OpenBSD Version 1.0

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn pa() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Pa name1 name2";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

name1 name2

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn pf() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ar value Pf $ Ar variable_name";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

value $ variable_name

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn pp() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ad name1 name2 
.Pp
.Ad name1 name2";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

name1 name2
 
name1 name2

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn rv() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rv -std f1 f2 Ar value";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

The f1(), f2(), Ar(), and value() functions return the value 0 if successful;
otherwise the value -1 is returned and the global variable errno is set to
indicate the error.

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn sm() {
            let input = 
".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Sm off A B C D
.Sm on A B C D";
            let output =
"PROGNAME(section)                   section                  PROGNAME(section)

ABCD A B C D

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn st() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Sh SECTION
.St -ansiC word
.St -iso9945-1-96";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

ANSI X3.159-1989 (“ANSI C89”) word ISO/IEC 9945-1:1996 (“POSIX.1”)

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn sx() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Sx MANUAL STRUCTURE";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

MANUAL STRUCTURE

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn sy() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Sy word1 word2";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

\u{1b}[1mword1 word2\u{1b}[0m

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn tg() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Tg term";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)


footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn tn() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Tn word1 word2";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

word1 word2

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn ud() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ud";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

currently under development.

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn ux() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ux";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

UNIX

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn va() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Va const char *bar";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

const char *bar

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn xr() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Xr mandoc 1";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

mandoc(1)

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }
    }

    mod partial_implicit {
        use crate::man_util::formatter::tests::test_formatting;

        #[test]
        fn block_empty() {
            let input = r#".Dd January 1, 1970
.Os footer text
.Aq"#;
            let output = "UNTITLED                             LOCAL                            UNTITLED

⟨⟩

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn block_single_line() {
            let input = r#".Dd January 1, 1970
.Os footer text
.Aq Ad addr addr Ad addr Ad addr"#;
            let output = "UNTITLED                             LOCAL                            UNTITLED

⟨addr addr addr addr⟩

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }
    }

    mod partial_explicit {
        use crate::man_util::formatter::tests::test_formatting;

        #[test]
        fn block_empty() {
            let input = r#".Dd January 1, 1970
.Os footer text
.Ao
.Ac"#;
            let output = "UNTITLED                             LOCAL                            UNTITLED

⟨⟩

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn block_single_line() {
            let input = r#".Dd January 1, 1970
.Os footer text
.Ao
.Ad addr addr
.Ad addr 
.Ad addr 
.Ac"#;
            let output = "UNTITLED                             LOCAL                            UNTITLED

⟨addr addr addr addr⟩

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn multi_line() {
            let input = r#".Dd January 1, 1970
.Os footer text
.Ao
.Ad addr 
.Ad addr 
.Ad addr 
Text loooooooong line
Text loooooooong line
Text loooooooong line
Text loooooooong line
Text loooooooong line
Text loooooooong line
.Ac"#;
            let output = r#"UNTITLED                             LOCAL                            UNTITLED

⟨addr addr addr Text loooooooong line Text loooooooong line Text loooooooong
line Text loooooooong line Text loooooooong line Text loooooooong line⟩

footer text                     January 1, 1970                    footer text"#;
            test_formatting(input, output);
        }

        #[test]
        fn block_overlong_line() {
            let input = r#".Dd January 1, 1970
.Os Debian
.Aq Ad addr Ad addr Ad addr Text looooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong line"#;
            let output = r#"UNTITLED                             LOCAL                            UNTITLED

⟨addr addr addr Text
looooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong
line⟩

Debian                          January 1, 1970                         Debian"#;
            test_formatting(input, output);
        }

        #[test]
        fn rs_block() {
            let input = ".Dd January 1, 1970
.Dt TITLE 7 arch
.Os footer text
.Rs
.%A J. E. Hopcroft
.%A J. D. Ullman
.%B Introduction to Automata Theory, Languages, and Computation
.%I Addison-Wesley
.%C Reading, Massachusetts
.%D 1979
.Re";
            let output =
                "TITLE(7)            Miscellaneous Information Manual (arch)           TITLE(7)

J. E. Hopcroft and J. D. Ullman, Introduction to Automata Theory, Languages,
and Computation, Addison-Wesley, Reading, Massachusetts, 1979.

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }
    }

    #[test]
    fn zero_width() {
        let input = r".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Xr mandoc 1 \&Ns \&( s \&) behaviour
Text Line \&Ns \&( s \&) behaviour";
        let output = 
"PROGNAME(section)                   section                  PROGNAME(section)

mandoc(1) Ns ( s ) behaviour Text Line Ns ( s ) behaviour

footer text                     January 1, 1970                    footer text";
        test_formatting(input, output);
    }

    mod delimiters {
        use super::*;

        #[test]
        fn delimiters_rs_submacros() {
            fn test(macro_str: &str) {
                let input = vec![
                    format!(".Dd January 1, 1970\n.Dt PROGNAME section\n.Os footer text"),
                    format!(".Rs\n{} {} text {}\n.Re", macro_str, "(", ")"),
                    format!(".Rs\n{} {} text {}\n.Re", macro_str, "[", "]"),
                    format!(".Rs\n{} text {}\n.Re",    macro_str, "."),
                    format!(".Rs\n{} text {}\n.Re",    macro_str, ","),
                    format!(".Rs\n{} text {}\n.Re",    macro_str, "?"),
                    format!(".Rs\n{} text {}\n.Re",    macro_str, "!"),
                    format!(".Rs\n{} text {}\n.Re",    macro_str, ":"),
                    format!(".Rs\n{} text {}\n.Re",    macro_str, ";")
                ].join("\n");
    
                let output = 
"PROGNAME(section)                   section                  PROGNAME(section)

(text). [text]. text.. text,. text?. text!. text:. text;.

footer text                     January 1, 1970                    footer text";
        
                test_formatting(&input, &output);
            }
            
            let macros = vec![
                "%A", "%B", "%C", "%D", "%I", "%J", "%N",
                "%O", "%P", "%Q", "%R", "%T", "%U", "%V",
            ];
    
            for macro_str in macros {
                test(macro_str);
            }
        }

        #[test]
        fn delimiters_inline_common() {
            fn test(macro_str: &str) {
                let input = vec![
                    format!(".Dd January 1, 1970\n.Dt PROGNAME section\n.Os footer text"),
                    format!(".{} {} text {}", macro_str, "(", ")"),
                    format!(".{} {} text {}", macro_str, "[", "]"),
                    format!(".{} text {}",    macro_str, "."),
                    format!(".{} text {}",    macro_str, ","),
                    format!(".{} text {}",    macro_str, "?"),
                    format!(".{} text {}",    macro_str, "!"),
                    format!(".{} text {}",    macro_str, ":"),
                    format!(".{} text {}",    macro_str, ";")
                ].join("\n");
    
                let output = 
"PROGNAME(section)                   section                  PROGNAME(section)

(text) [text] text. text, text? text! text: text;

footer text                     January 1, 1970                    footer text";
        
                test_formatting(&input, &output);
            }

            let inline_macros = vec![
                "Ad", "An", "Ar",
                "Cd", "Cm",
                "Dv",
                "Er", "Ev",
                "Fa", "Fr", "Ft",
                "Hf",
                "Ic",
                "Li",
                "Ms", "Mt",
                "No",
                "Ot",
                "Pa",
                "Sx",
                "Tn",
                "Va"
            ];
    
            for macro_str in inline_macros {
                println!("Macro: {macro_str}");

                test(macro_str);
            }
        }

        #[test]
        fn delimiters_text_production() {
            fn test(macro_str: &str) {
                let placeholder = match macro_str {
                    "At"  => "AT&T UNIX",
                    "Bsx" => "BSD/OS",
                    "Dx"  => "DragonFly",
                    "Fx"  => "FreeBSD",
                    "Nx"  => "NetBSD",
                    "Ox"  => "OpenBSD",
                    _ => unreachable!()
                };

                let input = vec![
                    format!(".Dd January 1, 1970\n.Dt PROGNAME section\n.Os footer text"),
                    format!(".{} {} text {}", macro_str, "(", ")"),
                    format!(".{} {} text {}", macro_str, "[", "]"),
                    format!(".{} text {}",    macro_str, ".")
                ].join("\n");

                let output = format!(
"PROGNAME(section)                   section                  PROGNAME(section)

({placeholder} text) [{placeholder} text] {placeholder} text.

footer text                     January 1, 1970                    footer text",
);
                test_formatting(&input, &output);
            }

            let macros = vec!["At", "Bsx", "Ox", "Dx", "Fx", "Nx"];

            for macro_str in macros {
                println!("Macro: {}", macro_str);

                test(macro_str)
            }

        }

        #[test]
        fn delimiters_bx() {
            let input = 
".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Bx ( random )
.Bx random !";
            let output = 
"PROGNAME(section)                   section                  PROGNAME(section)

(randomBSD) randomBSD!

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn delimiters_em() {
            let input = 
".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Em ( random ) text !";
            let output = 
"PROGNAME(section)                   section                  PROGNAME(section)

\u{1b}[3m(random) text!\u{1b}[0m

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn delimiters_fn() {
            let input = 
".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Fn ( random ) text !";
            let output = 
"PROGNAME(section)                   section                  PROGNAME(section)

(random() text!)

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn delimiters_sy() {
            let input = 
".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Sy ( random ) text !";
            let output = 
"PROGNAME(section)                   section                  PROGNAME(section)

\u{1b}[1m( random ) text !\u{1b}[0m

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn delimiters_fl() {
            let input = 
".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Fl ( random ) text !";
            let output = 
"PROGNAME(section)                   section                  PROGNAME(section)

(-random) -text!

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn delimiters_in() {
            let input = 
".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.In ( random )";
            let output = 
"PROGNAME(section)                   section                  PROGNAME(section)

(<random>)

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn delimiters_lb() {
            let input = 
".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Lb ( random )";
            let output = 
"PROGNAME(section)                   section                  PROGNAME(section)

(library “random”)

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn delimiters_vt() {
            let input = 
".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Vt ( random ) text !";
            let output = 
"PROGNAME(section)                   section                  PROGNAME(section)

(random) text!

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }
    }

    
    mod mdoc {
        use crate::man_util::formatter::tests::test_formatting;
        use std::{path::{Path, PathBuf}, process::Command, str::FromStr};
        use rstest::rstest;

        /// Recursively finds all file paths within a directory and its subdirectories.
        ///
        /// # Arguments
        ///
        /// * `dir_path` - The starting directory `PathBuf`.
        ///
        /// # Returns
        ///
        /// A `Vec<String>` containing the full paths of all files found. Returns an
        /// empty vector if the initial path is not a directory or if an error occurs
        /// when reading the top-level directory. Paths that are not valid UTF-8 will be skipped.
        fn get_dir_files_paths(dir_path: PathBuf) -> Vec<String> {
            let mut results = Vec::new();
            let _ = recursive_find_files(&dir_path, &mut results);
            results
        }

        /// Helper function to perform the recursive search.
        ///
        /// Modifies the `results` vector directly. Returns `io::Result<()>` to indicate
        /// if the read operation *on the current directory* succeeded, allowing the
        /// caller (or the initial call) to handle top-level errors.
        fn recursive_find_files(current_path: &Path, results: &mut Vec<String>) -> std::io::Result<()> {
            if !current_path.is_dir() {
                return Ok(());
            }

            let entries = std::fs::read_dir(current_path)?;

            for entry_result in entries {
                let Ok(entry) =  entry_result else{
                    continue;
                };
                let path = entry.path();
                if path.is_dir() {
                    let _ = recursive_find_files(&path, results);
                } else if path.is_file() && !path.to_str().unwrap().ends_with(":Zone.Identifier") {
                    if let Some(path_str) = path.to_str() {
                        results.push(path_str.to_string());
                    }
                }
            }

            Ok(())
        }

        //#[test]
        fn format_mdoc() {
            let test_dir = PathBuf::from_str("./test_files/mdoc").unwrap();
            assert!(test_dir.exists());
            
            let dir_paths = get_dir_files_paths(test_dir);
            
            for path in dir_paths{
                format_mdoc_file(&path);
            }
        }

        #[rstest]
        #[case("./test_files/mdoc/access.2")]
        // #[case("./test_files/mdoc/cvs.1")]
        #[case("./test_files/mdoc/getfh.2")]
        #[case("./test_files/mdoc/ioctl.2")]
        #[case("./test_files/mdoc/munmap.2")]
        #[case("./test_files/mdoc/rev.1")]
        #[case("./test_files/mdoc/shutdown.2")]
        #[case("./test_files/mdoc/talk.1")]
        #[case("./test_files/mdoc/adjfreq.2")]
        #[case("./test_files/mdoc/dc.1")]
        #[case("./test_files/mdoc/getgroups.2")]
        #[case("./test_files/mdoc/ipcs.1")]
        #[case("./test_files/mdoc/mv.1")]
        //#[case("./test_files/mdoc/rlog.1")]
        #[case("./test_files/mdoc/signify.1")]
        #[case("./test_files/mdoc/tmux.1")]
        #[case("./test_files/mdoc/atq.1")]
        #[case("./test_files/mdoc/diff.1")]
        #[case("./test_files/mdoc/getitimer.2")]
        #[case("./test_files/mdoc/ktrace.2")]
        #[case("./test_files/mdoc/nl.1")]
        #[case("./test_files/mdoc/rup.1")]
        #[case("./test_files/mdoc/sigreturn.2")]
        #[case("./test_files/mdoc/top.1")]
        #[case("./test_files/mdoc/bc.1")]
        #[case("./test_files/mdoc/dup.2")]
        #[case("./test_files/mdoc/getpeername.2")]
        #[case("./test_files/mdoc/lpq.1")]
        #[case("./test_files/mdoc/nm.1")]
        #[case("./test_files/mdoc/sched_yield.2")]
        #[case("./test_files/mdoc/sigsuspend.2")]
        #[case("./test_files/mdoc/truncate.2")]
        #[case("./test_files/mdoc/brk.2")]
        #[case("./test_files/mdoc/execve.2")]
        #[case("./test_files/mdoc/getpriority.2")]
        #[case("./test_files/mdoc/mg.1")]
        #[case("./test_files/mdoc/open.2")]
        #[case("./test_files/mdoc/scp.1")]
        #[case("./test_files/mdoc/size.1")]
        #[case("./test_files/mdoc/umask.2")]
        #[case("./test_files/mdoc/cal.1")]
        #[case("./test_files/mdoc/fgen.1")]
        #[case("./test_files/mdoc/getrtable.2")]
        #[case("./test_files/mdoc/minherit.2")]
        #[case("./test_files/mdoc/poll.2")]
        #[case("./test_files/mdoc/select.2")]
        #[case("./test_files/mdoc/snmp.1")]
        #[case("./test_files/mdoc/w.1")]
        #[case("./test_files/mdoc/cat.1")]
        #[case("./test_files/mdoc/file.1")]
        #[case("./test_files/mdoc/getrusage.2")]
        #[case("./test_files/mdoc/mkdir.1")]
        #[case("./test_files/mdoc/profil.2")]
        #[case("./test_files/mdoc/semget.2")]
        #[case("./test_files/mdoc/socket.2")]
        #[case("./test_files/mdoc/wall.1")]
        #[case("./test_files/mdoc/chdir.2")]
        #[case("./test_files/mdoc/flex.1")]
        #[case("./test_files/mdoc/getsid.2")]
        #[case("./test_files/mdoc/mkfifo.2")]
        #[case("./test_files/mdoc/quotactl.2")]
        #[case("./test_files/mdoc/send.2")]
        #[case("./test_files/mdoc/socketpair.2")]
        #[case("./test_files/mdoc/write.2")]
        #[case("./test_files/mdoc/chflags.2")]
        #[case("./test_files/mdoc/flock.2")]
        #[case("./test_files/mdoc/getsockname.2")]
        #[case("./test_files/mdoc/mlockall.2")]
        //#[case("./test_files/mdoc/rcs.1")]
        #[case("./test_files/mdoc/setuid.2")]
        #[case("./test_files/mdoc/statfs.2")]
        #[case("./test_files/mdoc/ypconnect.2")]
        #[case("./test_files/mdoc/chmod.2")]
        #[case("./test_files/mdoc/fork.2")]
        #[case("./test_files/mdoc/getsockopt.2")]
        #[case("./test_files/mdoc/mopa.out.1")]
        //#[case("./test_files/mdoc/rdist.1")]
        //#[case("./test_files/mdoc/sftp.1")]
        #[case("./test_files/mdoc/symlink.2")]
        #[case("./test_files/mdoc/closefrom.2")]
        #[case("./test_files/mdoc/fsync.2")]
        #[case("./test_files/mdoc/gettimeofday.2")]
        #[case("./test_files/mdoc/moptrace.1")]
        #[case("./test_files/mdoc/read.2")]
        #[case("./test_files/mdoc/shar.1")]
        #[case("./test_files/mdoc/sync.2")]
        #[case("./test_files/mdoc/cu.1")]
        #[case("./test_files/mdoc/futex.2")]
        //#[case("./test_files/mdoc/grep.1")]
        #[case("./test_files/mdoc/msgrcv.2")]
        #[case("./test_files/mdoc/reboot.2")]
        #[case("./test_files/mdoc/shmctl.2")]
        #[case("./test_files/mdoc/sysarch.2")]
        #[case("./test_files/mdoc/cut.1")]
        #[case("./test_files/mdoc/getdents.2")]
        #[case("./test_files/mdoc/id.1")]
        #[case("./test_files/mdoc/msgsnd.2")]
        #[case("./test_files/mdoc/rename.2")]
        #[case("./test_files/mdoc/shmget.2")]
        #[case("./test_files/mdoc/t11.2")]
        fn format_mdoc_file(#[case] path: &str){
            let input = std::fs::read_to_string(path).unwrap();
            let output = Command::new("mandoc")
                .args(["-T", "locale", path])
                .output()
                .unwrap()
                .stdout;
            let output = String::from_utf8(output).unwrap();
            println!("Current path: {}", path);
            test_formatting(&input, &output);
        }
        
    }
}