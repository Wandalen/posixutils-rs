use crate::FormattingSettings;
use aho_corasick::AhoCorasick;
use std::collections::HashMap;
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
    split_mod: bool,
}

impl Default for FormattingState {
    fn default() -> Self {
        Self {
            first_name: None,
            suppress_space: false,
            header_text: None,
            footer_text: None,
            spacing: " ".to_string(),
            split_mod: false,
        }
    }
}

#[derive(Debug)]
pub struct MdocFormatter {
    formatting_settings: FormattingSettings,
    formatting_state: FormattingState,
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
                        if codepoint < 0x10FFFF && !(0xD800 <= codepoint && codepoint <= 0xDFFF) {
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
    pub fn format_mdoc(&mut self, ast: MdocDocument) -> Vec<u8> {
        let max_width = self.formatting_settings.width;
        let mut lines = Vec::new();
        let mut current_line = String::new();

        for node in ast.elements {
            let formatted_node = self.format_node(node);

            if current_line.chars().count() + formatted_node.chars().count() > max_width {
                for word in formatted_node.split_whitespace() {
                    if current_line.chars().count() + word.chars().count() >= max_width {
                        lines.push(current_line.trim_end().to_string());
                        current_line.clear();
                    }

                    current_line.push_str(word);
                    current_line.push(' ');
                }
            } else {
                let is_all_control = formatted_node.chars().all(|ch| ch.is_ascii_control());
                if is_all_control {
                    if let Some(' ') = current_line.chars().last() {
                        current_line.pop();
                    }
                }
                current_line.push_str(&formatted_node);
                if !formatted_node.is_empty() && !is_all_control {
                    current_line.push(' ');
                }
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line.trim_end().to_string());
        }

        lines.insert(
            0,
            self.formatting_state
                .header_text
                .clone()
                .unwrap_or(self.format_default_header()),
        );
        lines.push(self.format_footer());

        lines.join("\n").into_bytes()
    }

    fn format_default_header(&mut self) -> String {
        self.format_dt(None, "", None)
    }

    fn get_default_footer_text() -> String {
        String::new()
    }

    fn format_footer(&self) -> String {
        let footer_text = self
            .formatting_state
            .footer_text
            .clone()
            .unwrap_or(Self::get_default_footer_text());

        let date = self.format_dd(chrono::Local::now().date_naive().into());

        let mut space_size = self
            .formatting_settings
            .width
            .saturating_sub(2 * footer_text.len() + date.len())
            / 2;

        let mut left_footer_text = footer_text.clone();
        let mut right_footer_text = footer_text.clone();

        if space_size <= 1 {
            space_size = self.formatting_settings.width.saturating_sub(date.len()) / 2;

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

        let space = vec![" "; space_size].into_iter().collect::<String>();

        let mut content = format!(
            "\n{}{}{}{}{}",
            left_footer_text,
            space.clone(),
            date,
            space,
            right_footer_text
        );

        let missing_space = self
            .formatting_settings
            .width
            .saturating_sub(content.len() - 1);

        content.insert_str(
            left_footer_text.len() + 1,
            &vec![" "; missing_space].join(""),
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
            Macro::Ap => self.format_ap(),
            Macro::Ar => self.format_ar(macro_node),
            Macro::Bt => self.format_bt(),
            Macro::Cd => self.format_cd(macro_node),
            Macro::Cm => self.format_cm(macro_node),
            Macro::Db => self.format_db(),
            Macro::Dv => self.format_dv(macro_node),
            Macro::Em => self.format_em(macro_node),
            Macro::An { author_name_type } => self.format_an(author_name_type, macro_node),
            Macro::Dd { date } => self.format_dd(date),
            Macro::Dt {
                title,
                section,
                arch,
            } => self.format_dt(title.clone(), section.as_str(), arch.clone()),

            Macro::Er => self.format_er(macro_node),
            Macro::Es {
                opening_delimiter,
                closing_delimiter,
            } => self.format_es(opening_delimiter, closing_delimiter),
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
            Macro::In { filename } => self.format_in(filename.as_str()),
            Macro::Lb { lib_name } => self.format_lb(lib_name.as_str()),
            Macro::Li => self.format_li(macro_node),
            Macro::Lk { ref uri } => self.format_lk(uri.as_str(), macro_node),
            Macro::Lp => self.format_lp(),
            Macro::Ms => self.format_ms(macro_node),
            Macro::Mt => self.format_mt(macro_node),
            Macro::Nm => self.format_nm(macro_node),
            Macro::No => self.format_no(macro_node),
            Macro::Ns => self.format_ns(),
            Macro::Nx => self.format_nx(macro_node),
            Macro::Os => self.format_os(macro_node),
            Macro::Ot => self.format_ot(macro_node),
            Macro::Ox => self.format_ox(macro_node),
            Macro::Pa => self.format_pa(macro_node),
            Macro::Pf { prefix } => self.format_pf(prefix.as_str()),
            Macro::Pp => self.format_pp(macro_node),
            Macro::Rv => self.format_rv(macro_node),
            Macro::Sm(sm_mode) => self.format_sm(sm_mode),
            Macro::St(st_type) => self.format_st(st_type),
            Macro::Sx => self.format_sx(macro_node),
            Macro::Sy => self.format_sy(macro_node),
            Macro::Tg { term } => self.format_tg(term),
            Macro::Tn => self.format_tn(macro_node),
            Macro::Ud => self.format_ud(),
            Macro::Ux => self.format_ux(),
            Macro::Va => self.format_va(macro_node),
            Macro::Xr { name, section } => self.format_xr(name.as_str(), section.as_str()),

            _ => unreachable!(),
        }
    }

    fn format_text_node(&self, text: &str) -> String {
        let replacements: HashMap<&str, &str> = [
            // Spaces:
            (r"\ ", " "), // unpaddable space
            (r"\~", " "), // paddable space
            (r"\0", " "), // digit-width space
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
    fn format_ta(&mut self, macro_node: MacroNode) -> String {
        unimplemented!()
    }
}

// Formatting block full-explicit.
impl MdocFormatter {
    fn format_bd_block(&mut self, macro_node: MacroNode) -> String {
        /*let Macro::Bd { block_type, offset, compact } = macro_node.mdoc_macro else{
            unreachable!()
        };

        let (offset, align) = if let Some(offset) = offset{
            match offset{
                OffsetType::Indent => (self.formatting_settings.indent, OffsetType::Left),
                OffsetType::IndentTwo => (self.formatting_settings.indent * 2, OffsetType::Left),
                OffsetType::Left => (0, OffsetType::Left),
                OffsetType::Right => (0, OffsetType::Right),
                OffsetType::Center => (0, OffsetType::Center)
            }
        } else {
            (self.formatting_settings.indent as isize, OffsetType::Left)
        };

        let mut content = String::new();
        for element in macro_node.nodes{
            let formatted_element = self.format_node(element);
            let mut content = match block_type{
                BdType::Centered => {
                    if self.formatting_settings.width >= formatted_element.len(){

                    }else{
                        content += &formatted_element;
                    }
                },
                _ if matches!(align, OffsetType::Center) => {

                },
                BdType::Filled => {
                    content += match align{
                        OffsetType::Left => formatted_element,
                        OffsetType::Right => formatted_element,
                        _ => unreachable!()
                    }
                },
                BdType::Literal | BdType::Unfilled => {
                    content += formatted_element.as_str() + " ";
                    if let Some(c) = content.strip_suffix(" "){
                        content = c.to_string();
                    }
                    match align{
                        OffsetType::Left => ,
                        OffsetType::Right => ,
                        _ => unreachable!()
                    }
                },
                BdType::Ragged => {

                }
            };
        }

        if !compact{
            let vertical_space = "\n\n".to_string();
            content = vertical_space.clone() + &content + &vertical_space;
        }

        content*/
        String::new()
    }

    fn format_bf_block(&mut self, macro_node: MacroNode) -> String {
        let content = macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing);

        content
    }

    fn format_bk_block(&mut self, macro_node: MacroNode) -> String {
        let mut content = macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing);

        content.replace("\n", " ").replace("\r", "")
    }

    fn format_bl_block(&mut self, macro_node: MacroNode) -> String {
        unimplemented!()
    }
}

// Formatting block full-implicit.
impl MdocFormatter {
    fn format_it_block(&mut self, macro_node: MacroNode) -> String {
        unimplemented!()
    }

    fn format_nd(&mut self, macro_node: MacroNode) -> String {
        let content = macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing);

        content
    }

    fn format_nm(&mut self, macro_node: MacroNode) -> String {
        let content = macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing);

        if !content.is_empty() {
            self.formatting_state.first_name = Some(content.clone());
        }

        content
    }

    fn format_sh_block(&mut self, macro_node: MacroNode) -> String {
        unimplemented!()
    }

    fn format_ss_block(&mut self, macro_node: MacroNode) -> String {
        unimplemented!()
    }
}

// Formatting block partial-explicit.
impl MdocFormatter {
    fn format_partial_explicit_block(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
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
        let mut iter = macro_node.nodes.into_iter();

        let is_a = |el: &Element| match el {
            Element::Macro(node) => node.mdoc_macro == Macro::A,
            _ => unreachable!("Unexpected rule!"),
        };

        let items: Vec<String> = iter
            .by_ref()
            .take_while(|el| is_a(el))
            .map(|el| match el {
                Element::Macro(node) => self.format_a(node),
                _ => unreachable!("Unexcpected rule!"),
            })
            .collect();

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
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
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
                                let offset = if is_first_node { "" } else { " " };
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

    fn format_ap(&self) -> String {
        "'".to_string()
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
                    true => format!("{}\n", content),
                    false => content,
                }
            }
        }
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
            None if section.is_empty() => format!("UNTITLED"),
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
        String::new()
    }

    fn format_dx(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_dd(&self, date: DdDate) -> String {
        match date {
            DdDate::MDYFormat(dd_date) => format!(
                "{} {}, {}",
                dd_date.month_day.0, dd_date.month_day.1, dd_date.year
            ),
            DdDate::StrFormat(string) => string,
        }
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
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_es(&self, opening_delimiter: char, closing_delimiter: char) -> String {
        format!("{}{}", opening_delimiter, closing_delimiter)
    }

    fn format_ev(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
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
        let content = macro_node
            .nodes
            .iter()
            .filter_map(|el| {
                let Element::Text(text) = el else {
                    let string = self.format_node(el.clone());
                    if string.is_empty() {
                        return None;
                    } else {
                        return Some(string);
                    }
                };
                let mut text = text.clone();
                for ch in &["'", "\"", "`"] {
                    if let Some(t) = text.strip_prefix(ch) {
                        text = t.to_string()
                    }
                    if let Some(t) = text.strip_suffix(ch) {
                        text = t.to_string()
                    }
                }
                Some(text)
            })
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing);

        content
    }

    fn format_fd(&self, directive: &str, arguments: &Vec<String>) -> String {
        format!(
            "{directive} {}",
            arguments.join(&self.formatting_state.spacing)
        )
    }

    fn format_fl(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_fn(&mut self, funcname: &str, macro_node: MacroNode) -> String {
        let content = macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>()
            .join(", ");

        format!("{funcname}({content})")
    }

    fn format_fr(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_ft(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_fx(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_hf(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_ic(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_in(&self, filename: &str) -> String {
        format!("<{filename}>")
    }

    fn format_lb(&self, lib_name: &str) -> String {
        format!("library “{lib_name}”")
    }

    fn format_li(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
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
        format!("\n\n")
    }

    fn format_ms(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_mt(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_no(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_ns(&mut self) -> String {
        self.formatting_state.suppress_space = true;
        String::new()
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

    fn format_ot(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_ox(&self, macro_node: MacroNode) -> String {
        self.format_inline_macro(macro_node)
    }

    fn format_pa(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_pf(&mut self, prefix: &str) -> String {
        self.formatting_state.suppress_space = true;
        prefix.to_string()
    }

    fn format_pp(&self, _macro_node: MacroNode) -> String {
        format!("\n\n")
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

    fn format_sm(&mut self, sm_mode: Option<SmMode>) -> String {
        self.formatting_state.spacing = match sm_mode {
            Some(SmMode::On) => " ".to_string(),
            Some(SmMode::Off) => "".to_string(),
            None => match self.formatting_state.spacing.as_str() {
                "" => " ".to_string(),
                " " => "".to_string(),
                _ => " ".to_string(),
            },
        };
        String::new()
    }

    fn format_st(&self, st_type: StType) -> String {
        st_type.to_string()
    }

    fn format_sx(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_sy(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_tg(&self, _term: Option<String>) -> String {
        String::new()
    }

    fn format_tn(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_ud(&self) -> String {
        "currently under development.".to_string()
    }

    fn format_ux(&self) -> String {
        "UNIX".to_string()
    }

    fn format_va(&mut self, macro_node: MacroNode) -> String {
        macro_node
            .nodes
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<Vec<String>>()
            .join(&self.formatting_state.spacing)
    }

    fn format_xr(&self, name: &str, section: &str) -> String {
        format!("{name}({section})")
    }
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

        println!("{:?}", formatter);

        let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
        assert_eq!(output, result)
    }

    mod special_chars {
        use crate::man_util::formatter::tests::test_formatting;

        #[test]
        fn spaces() {
            let input = r"\ \~\0\|\^\&\)\%\:";
            let output = r"


                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn lines() {
            let input = r"\(ba \(br \(ul \(ru \(rn \(bb \(sl \(rs";
            let output = r"
| │ _ _ ‾ ¦ / \

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn text_markers() {
            let input = r"\(ci \(bu \(dd \(dg \(lz \(sq \(ps \(sc \(lh \(rh \(at \(sh \(CR \(OK \(CL \(SP \(HE \(DI";
            let output = r"
○ • ‡ † ◊ □ ¶ § ☜ ☞ @ # ↵ ✓ ♣ ♠ ♥ ♦

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn legal_symbols() {
            let input = r"\(co \(rg \(tm";
            let output = r"
© ® ™

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn punctuation() {
            let input = r"\(em \(en \(hy \e \(r! \(r?";
            let output = r"
— – ‐ \\ ¡ ¿

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn quotes() {
            let input = r"\(Bq \(bq \(lq \(rq \(oq \(cq \(aq \(dq \(Fo \(Fc \(fo \(fc";
            let output = "
„ ‚ “ ” ‘ ’ ' \" « » ‹ ›

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn brackets() {
            let input = r"\(lB \(rB \(lC \(rC \(la \(ra \(bv \[braceex] \[bracketlefttp] \[bracketleftbt]
\[bracketleftex] \[bracketrighttp] \[bracketrightbt] \[bracketrightex]
\(lt \[bracelefttp] \(lk \[braceleftmid] \(lb \[braceleftbt] \[braceleftex]
\(rt \[bracerighttp] \(rk \[bracerightmid] \(rb \[bracerightbt] \[bracerightex]
\[parenlefttp] \[parenleftbt] \[parenleftex] \[parenrighttp] \[parenrightbt] \[parenrightex]
";
            let output = r"
[ ] { } ⟨ ⟩ ⎪ ⎪ ⎡ ⎣ ⎢ ⎤ ⎦ ⎥ ⎧ ⎧ ⎨ ⎨ ⎩ ⎩ ⎪ ⎫ ⎫ ⎬ ⎬ ⎭ ⎭ ⎪ ⎛ ⎝ ⎜ ⎞ ⎠ ⎟

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn arrows() {
            let input = r"\(<- \(-> \(<> \(da \(ua \(va \(lA \(rA \(hA \(uA \(dA \(vA \(an";
            let output = r"
← → ↔ ↓ ↑ ↕ ⇐ ⇒ ⇔ ⇑ ⇓ ⇕ ⎯

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn logical() {
            let input = r"\(AN \(OR \[tno] \(no \(te \(fa \(st \(tf \(3d \(or";
            let output = r"
∧ ∨ ¬ ¬ ∃ ∀ ∋ ∴ ∴ |

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn mathematical() {
            let input = r"\- \(mi \+ \(pl \(-+ \[t+-] \(+- \(pc \[tmu]
\(mu \(c* \(c+ \[tdi] \(di \(f/ \(** \(<= \(>= \(<< \(>> \(eq \(!= \(==
\(ne \(ap \(|= \(=~ \(~~ \(~= \(pt \(es \(mo \(nm \(sb \(nb \(sp
\(nc \(ib \(ip \(ca \(cu \(/_ \(pp \(is \[integral] \[sum] \[product]
\[coproduct] \(gr \(sr \[sqrt] \(lc \(rc \(lf \(rf \(if \(Ah \(Im \(Re
\(wp \(pd \(-h \[hbar] \(12 \(14 \(34 \(18 \(38 \(58 \(78 \(S1 \(S2 \(S3
";
            let output = r"
- − + + ∓ ± ± · × × ⊗ ⊕ ÷ ÷ ⁄ ∗ ≤ ≥ ≪ ≫ = ≠ ≡ ≢ ∼ ≃ ≅ ≈ ≈ ∝ ∅ ∈ ∉ ⊂ ⊄ ⊃ ⊅ ⊆ ⊇
∩ ∪ ∠ ⊥ ∫ ∫ ∑ ∏ ∐ ∇ √ √ ⌈ ⌉ ⌊ ⌋ ∞ ℵ ℑ ℜ ℘ ∂ ℏ ℏ ½ ¼ ¾ ⅛ ⅜ ⅝ ⅞ ¹ ² ³

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn ligatures() {
            let input = r"\(ff \(fi \(fl \(Fi \(Fl \(AE \(ae \(OE \(oe \(ss \(IJ \(ij";
            let output = r"
ﬀ ﬁ ﬂ ﬃ ﬄ Æ æ Œ œ ß Ĳ ĳ

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn accents() {
            let input = "\\(a\" \\(a- \\(a. \\(a^ \\(aa \\\' \\(ga \\` \\(ab \\(ac \\(ad \\(ah \\(ao \\(a~ \\(ho \\(ha \\(ti";
            let output = r"
˝ ¯ ˙ ^ ´ ´ ` ` ˘ ¸ ¨ ˇ ˚ ~ ˛ ^ ~

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn accented_letters() {
            let input = r"\('A \('E \('I \('O \('U \('Y \('a \('e
\('i \('o \('u \('y \(`A \(`E \(`I \(`O \(`U \(`a \(`e \(`i \(`o \(`u
\(~A \(~N \(~O \(~a \(~n \(~o \(:A \(:E \(:I \(:O \(:U \(:a \(:e \(:i
\(:o \(:u \(:y \(^A \(^E \(^I \(^O \(^U \(^a \(^e \(^i \(^o \(^u \(,C
\(,c \(/L \(/l \(/O \(/o \(oA \(oa
";
            let output = r"
Á É Í Ó Ú Ý á é í ó ú ý À È Ì Ò Ù à è ì ò ù Ã Ñ Õ ã ñ õ Ä Ë Ï Ö Ü ä ë ï ö ü ÿ
Â Ê Î Ô Û â ê î ô û Ç ç Ł ł Ø ø Å å

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn special_letters() {
            let input = r"\(-D \(Sd \(TP \(Tp \(.i \(.j";
            let output = r"
Ð ð Þ þ ı ȷ

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn currency() {
            let input = r"\(Do \(ct \(Eu \(eu \(Ye \(Po \(Cs \(Fn";
            let output = r"
$ ¢ € € ¥ £ ¤ ƒ

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn units() {
            let input = r"\(de \(%0 \(fm \(sd \(mc \(Of \(Om";
            let output = r"
° ‰ ′ ″ µ ª º

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn greek_leters() {
            let input = r"\(*A \(*B \(*G \(*D \(*E \(*Z
\(*Y \(*H \(*I \(*K \(*L \(*M \(*N \(*C \(*O \(*P \(*R \(*S
\(*T \(*U \(*F \(*X \(*Q \(*W \(*a \(*b \(*g \(*d \(*e \(*z
\(*y \(*h \(*i \(*k \(*l \(*m \(*n \(*c \(*o \(*p \(*r \(*s
\(*t \(*u \(*f \(*x \(*q \(*w \(+h \(+f \(+p \(+e \(ts
";
            let output = r"
Α Β Γ Δ Ε Ζ Η Θ Ι Κ Λ Μ Ν Ξ Ο Π Ρ Σ Τ Υ Φ Χ Ψ Ω α β γ δ ε ζ η θ ι κ λ μ ν ξ ο
π ρ σ τ υ ϕ χ ψ ω ϑ φ ϖ ϵ ς

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn predefined_strings() {
            let input = r"\*(Ba \*(Ne \*(Ge \*(Le \*(Gt \*(Lt \*(Pm \*(If \*(Pi \*(Na \*(Am \*R \*(Tm \*q \*(Rq \*(Lq \*(lp \*(rp \*(lq \*(rq \*(ua \*(va \*(<= \*(>= \*(aa \*(ga \*(Px \*(Ai";
            let output ="
| ≠ ≥ ≤ > < ± infinity pi NaN & ® (Tm) \" ” “ ( ) “ ” ↑ ↕ ≤ ≥ ´ ` POSIX ANSI

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn unicode() {
            let input = r"\[u0100] \C'u01230' \[u025600]";
            let output = "
Ā ሰ 𥘀

                                March 25, 2025                                ";
            test_formatting(input, output);
        }

        #[test]
        fn numbered() {
            let input = r"\N'34' \[char43]";
            let output = "
\" +

                                March 25, 2025                                ";
            test_formatting(input, output);
        }
    }

    mod full_explicit {
        use crate::man_util::formatter::tests::test_formatting;

        #[test]
        fn bd() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Bd -literal indent -compact
Line 1
Line 2
.Ed";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

      Line 1
      Line 2

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn bf() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Bf -emphasis
Line 1
Line 2
.Ed";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

Line 1 Line 2

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn bk() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Bk -words
Line 1
Line 2
.Ek";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

Line 1 Line 2

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn bl() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Bl -bullet -width indent-two -compact col1 col2 col3
.It Line 1
.It Line 2
.El";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

•
•

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }
    }

    mod full_implicit {
        use crate::man_util::formatter::tests::test_formatting;

        #[test]
        fn it() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.It Line 1
.It Line 2";
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
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Sh SECTION
Line 1
Line 2
Line 3";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

SECTION
     Line 1 Line 2 Line 3

footer text                     January 1, 1970                    footer text";
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
.Bl -bullet -width indent-two -compact col1 col2 col3
.It Line 1
.It Line 2
.It Line 3
.Ta
.It Line 4
.It Line 5
.It Line 6
.Ta
.El";
        let output =
            "PROGNAME(section)                   section                  PROGNAME(section)

•
•
•
•
•
•

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
.R
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

Technical report, Technical report. (Technical report) Technical report,
Technical report, Technical report, Technical report!. Technical report.

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
.R
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

Article title, Article title. (Article title) Article title, Article title,
Article title, Article title!. Article title.

footer text                     January 1, 1970                    footer text";
                test_formatting(input, output);
            }

            #[test]
            fn u() {
                let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Rs
.%U  protocol://path
.R
.Rs
.%U  protocol://path
.%U  protocol://path
.Re
.Rs
.%U (  protocol://path )  protocol://path
.%U  protocol://path ,  protocol://path
.%U  protocol://path !
.Re";
                let output =
                    "PROGNAME(section)                   section                  PROGNAME(section)

Article title, Article title. (Article title) Article title, Article title,
Article title, Article title!. Article title.

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
.R
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

Volume number, Volume number. (Volume number) Volume number, Volume number,
Volume number, Volume number!. Volume number.

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
.Dt TITLE 7 arch
.Os footer text
.An -split
.An Kristaps Dzonsons Aq Mt kristaps@bsd.lv
.An Kristaps Dzonsons Aq Mt kristaps@bsd.lv
.An Kristaps Dzonsons Aq Mt kristaps@bsd.lv
.An Kristaps Dzonsons Aq Mt kristaps@bsd.lv
.An -nosplit
.An Kristaps Dzonsons Aq Mt kristaps@bsd.lv
.An Kristaps Dzonsons Aq Mt kristaps@bsd.lv
.An Kristaps Dzonsons Aq Mt kristaps@bsd.lv
.An Kristaps Dzonsons Aq Mt kristaps@bsd.lv";
            let output =
                "TITLE(7)            Miscellaneous Information Manual (arch)           TITLE(7)

Kristaps Dzonsons <kristaps@bsd.lv>
Kristaps Dzonsons <kristaps@bsd.lv>
Kristaps Dzonsons <kristaps@bsd.lv>
Kristaps Dzonsons <kristaps@bsd.lv> Kristaps Dzonsons <kristaps@bsd.lv>
Kristaps Dzonsons <kristaps@bsd.lv> Kristaps Dzonsons <kristaps@bsd.lv>
Kristaps Dzonsons <kristaps@bsd.lv>

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn ap() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Ap";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

'

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
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.At
.At III
.At V.1
.At ( V.1 )";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

AT&T UNIX AT&T System III UNIX AT&T System V Release 1 UNIX (AT&T System V
Release 1 UNIX)

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
.Db";
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

Selected lines are those not matching any of the specified patterns.  Some of
the functions use a hold space to save the pattern space for subsequent
retrieval.

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn er() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Er ERROR ERROR2
.Er";
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

funcname(arg, arg2, arg3)

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
.No a b c
.Ns
.No a b c";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

a b c a b c

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
.Pa name1 name2
.Pp
.Pa name1 name2";
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
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
.Sm on
A B C
.Sm off
F G H
.Sm
R T Y";
            let output =
                "PROGNAME(section)                   section                  PROGNAME(section)

A B C F G H R T Y

footer text                     January 1, 1970                    footer text";
            test_formatting(input, output);
        }

        #[test]
        fn st() {
            let input = ".Dd January 1, 1970
.Dt PROGNAME section
.Os footer text
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

word1 word2

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
            let input = r#".Aq"#;
            let output = "⟨⟩";
            test_formatting(input, output);
        }

        #[test]
        fn block_single_line() {
            let input = r#".Aq Ad addr addr Ad addr Ad addr"#;
            let output = "⟨addr addr addr addr⟩";
            test_formatting(input, output);
        }
    }

    mod partial_explicit {
        use crate::man_util::formatter::tests::test_formatting;

        #[test]
        fn block_empty() {
            let input = r#".Ao
.Ac"#;
            let output = "⟨⟩";
            test_formatting(input, output);
        }

        #[test]
        fn block_single_line() {
            let input = r#".Ao
.Ad addr addr
.Ad addr 
.Ad addr 
.Ac"#;
            let output = "⟨addr addr addr⟩";
            test_formatting(input, output);
        }

        #[test]
        fn multi_line() {
            let input = r#".Ao
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
            let output = r#"⟨addr addr addr Text loooooooong line Text loooooooong line Text loooooooong
line Text loooooooong line Text loooooooong line Text loooooooong line⟩"#;
            test_formatting(input, output);
        }

        #[test]
        fn block_overlong_line() {
            let input = r#".Aq Ad addr Ad addr Ad addr Text looooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong line"#;
            let output = r#"⟨addr addr addr Text
looooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong
line⟩"#;
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
    fn test_delimiters() {
        let input = r#".Ao
.Ad ( addr ) addr
.Ad [ addr ] 
.Ad [ addr Ad addr ] 
.Ac"#;
        let output = r"
⟨(addr) addr [addr] [addr addr]⟩

                                March 25, 2025                                ";
        test_formatting(input, output);
    }
}
