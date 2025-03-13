use std::collections::HashMap;
use aho_corasick::AhoCorasick;
use terminfo::Database;
use crate::FormattingSettings;

use super::{mdoc_macro::{types::{AnType, DdDate}, Macro}, parser::{Element, MacroNode, MdocDocument}};

static REGEX_UNICODE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
    regex::Regex::new(r"(?x)
        (?:
            (?P<unicode_bracket>\\\[u(?P<hex1>[0-9A-F]{4,6})\])      |
            (?P<unicode_c>\\C'u(?P<hex2>[0-9A-F]{4,6})')             |
            (?P<number_n>\\N'(?P<dec1>[0-9]+)')                      |
            (?P<number_char>\\\[char(?P<dec2>[0-9]+)\])
        )
    ").unwrap()
});

#[derive(Debug)]
pub struct MdocFormatter {
    formatting_settings: FormattingSettings
}

// Helper funcitons.
impl MdocFormatter {
    pub fn new(settings: FormattingSettings) -> Self {
        Self {
            formatting_settings: settings
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
        REGEX_UNICODE.replace_all(text, |caps: &regex::Captures| {
            if let Some(hex) = caps.name("hex1").or_else(|| caps.name("hex2")).map(|m| m.as_str()) {
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
            } 
            else if let Some(dec) = caps.name("dec1").or_else(|| caps.name("dec2")).map(|m| m.as_str()) {
                if let Ok(codepoint) = dec.parse::<u32>() {
                    if let Some(ch) = char::from_u32(codepoint) {
                        return ch.to_string();
                    }
                }
            }
            caps.get(0).unwrap().as_str().to_string()
        }).to_string()
    }
}

// Base formatting functions.
impl MdocFormatter {
    pub fn format_mdoc(&self, ast: MdocDocument) -> Vec<u8> {
        ast.elements
            .into_iter()
            .map(|node| self.format_node(node))
            .collect::<String>()
            .into_bytes()
    }

    fn format_node(&self, node: Element) -> String {
        match node {
            Element::Macro(macro_node) => self.format_macro_node(macro_node),
            Element::Text(text) => self.format_text_node(text.as_str()),
            Element::Eoi => "".to_string()
        }
    }

    fn format_macro_node(&self, macro_node: MacroNode) -> String {
        match macro_node.mdoc_macro {
            // Block partial-explicit.
            Macro::Ao  => unimplemented!(),
            Macro::Bo  => unimplemented!(),
            Macro::Bro => unimplemented!(),
            Macro::Do  => unimplemented!(),
            Macro::Eo { opening_delimiter, closing_delimiter } => unimplemented!(),
            Macro::Fo { funcname }  => unimplemented!(),
            Macro::Oo  => unimplemented!(),
            Macro::Po  => unimplemented!(),
            Macro::Qo  => unimplemented!(),
            Macro::Rs  => self.format_rs_block(macro_node),
            Macro::So  => unimplemented!(),
            Macro::Xo  => unimplemented!(),

            // Block partial-implicit.
            Macro::Aq  => unimplemented!(),
            Macro::Bq  => unimplemented!(),
            Macro::Brq => unimplemented!(),
            Macro::D1  => unimplemented!(),
            Macro::Dl  => unimplemented!(),
            Macro::Dq  => unimplemented!(),
            Macro::En  => unimplemented!(),
            Macro::Op  => unimplemented!(),
            Macro::Pq  => unimplemented!(),
            Macro::Ql  => unimplemented!(),
            Macro::Qq  => unimplemented!(),
            Macro::Sq  => unimplemented!(),
            Macro::Vt  => unimplemented!(),

            // In-line.
            Macro::B { book_title } => self.format_b(&book_title),
            Macro::T { article_title } => self.format_t(&article_title),
            Macro::U { uri } => self.format_u(&uri),
            Macro::Ad => self.format_ad(macro_node),
            // Macro::An { author_name_type } => unimplemented!(),
            Macro::Ap => self.format_ap(),
            Macro::Ar => self.format_ar(macro_node),
            // TODO: Fix it.
            // Macro::At()
            // Macro::Bsx()
            Macro::Bt => self.format_bt(),
            // TODO: Fix it.
            // Macro::Bx()
            Macro::Cd => self.format_cd(macro_node),
            Macro::Cm => self.format_cm(macro_node),
            Macro::Db => self.format_db(),
            // Macro::Dd { date } => unimplemented!(),
            Macro::Dt { title, section, arch } => self.format_dt(title, section, arch),
            Macro::Dv => self.format_dv(macro_node),
            // Macro::Dx() => unimplemented!(),
            Macro::Em => self.format_em(macro_node),
            _ => unreachable!()   
        }
    }

    fn format_text_node(&self, text: &str) -> String {
        let replacements: HashMap<&str, &str> = [
            // Spaces:
            (r"\ ", " "),    // unpaddable space
            (r"\~", " "),    // paddable space
            (r"\0", " "),    // digit-width space
            (r"\|", " "),    // one-sixth \(em narrow space
            (r"\^", " "),    // one-twelfth \(em half-narrow space
            (r"\&", ""),     // zero-width space
            (r"\)", ""),     // zero-width space (transparent to end-of-sentence detection)
            (r"\%", ""),     // zero-width space allowing hyphenation
            (r"\:", ""),     // zero-width space allowing line break

            // Lines:
            (r"\(ba", r"|"),  // bar
            (r"\(br", r"│"),  // box rule
            (r"\(ul", r"_"),  // underscore
            (r"\(ru", r"_"),  // underscore (width 0.5m)
            (r"\(rn", r"‾"),  // overline
            (r"\(bb", r"¦"),  // broken bar
            (r"\(sl", r"/"),  // forward slash
            (r"\(rs", r"\"), // backward slash

            // Text markers:
            (r"\(ci", r"○"),  // circle
            (r"\(bu", r"•"),  // bullet
            (r"\(dd", r"‡"),  // double dagger
            (r"\(dg", r"†"),  // dagger
            (r"\(lz", r"◊"),  // lozenge
            (r"\(sq", r"□"),  // white square
            (r"\(ps", r"¶"),  // paragraph
            (r"\(sc", r"§"),  // section
            (r"\(lh", r"☜"),  // left hand
            (r"\(rh", r"☞"),  // right hand
            (r"\(at", r"@"),  // at
            (r"\(sh", r"#"),  // hash (pound)
            (r"\(CR", r"↵"),  // carriage return
            (r"\(OK", r"✓"),  // check mark
            (r"\(CL", r"♣"),  // club suit
            (r"\(SP", r"♠"),  // spade suit
            (r"\(HE", r"♥"),  // heart suit
            (r"\(DI", r"♦"),  // diamond suit

            // Legal symbols:
            (r"\(co", r"©"),  // copyright
            (r"\(rg", r"®"),  // registered
            (r"\(tm", r"™"),  // trademarked

            // Punctuation:
            (r"\(em", r"—"),  // em-dash
            (r"\(en", r"–"),  // en-dash
            (r"\(hy", r"‐"),  // hyphen
            (r"\e", r"\\"),    // back-slash
            (r"\(r!", r"¡"),  // upside-down exclamation
            (r"\(r?", r"¿"),  // upside-down question

            // Quotes:
            (r"\(Bq", r"„"),  // right low double-quote
            (r"\(bq", r"‚"),  // right low single-quote
            (r"\(lq", r"“"),  // left double-quote
            (r"\(rq", r"”"),  // right double-quote
            (r"\(oq", r"‘"),  // left single-quote
            (r"\(cq", r"’"),  // right single-quote
            (r"\(aq", r"'"),  // apostrophe quote (ASCII character)
            (r"\(dq", "\""),  // double quote (ASCII character)
            (r"\(Fo", r"«"),  // left guillemet
            (r"\(Fc", r"»"),  // right guillemet
            (r"\(fo", r"‹"),  // left single guillemet
            (r"\(fc", r"›"),  // right single guillemet

            // Brackets:
            (r"\(lB", r"["),   // left bracket
            (r"\(rB", r"]"),   // right bracket
            (r"\(lC", r"{"),   // left brace
            (r"\(rC", r"}"),   // right brace
            (r"\(la", r"⟨"),   // left angle
            (r"\(ra", r"⟩"),   // right angle
            (r"\(bv", r"⎪"),   // brace extension (special font)
            (r"\[braceex]", r"⎪"), // brace extension
            (r"\[bracketlefttp]", r"⎡"), // top-left hooked bracket
            (r"\[bracketleftbt]", r"⎣"), // bottom-left hooked bracket
            (r"\[bracketleftex]", r"⎢"), // left hooked bracket extension
            (r"\[bracketrighttp]", r"⎤"), // top-right hooked bracket
            (r"\[bracketrightbt]", r"⎦"), // bottom-right hooked bracket
            (r"\[bracketrightex]", r"⎥"), // right hooked bracket extension
            (r"\(lt", r"⎧"),   // top-left hooked brace
            (r"\[bracelefttp]", r"⎧"), // top-left hooked brace
            (r"\(lk", r"⎨"),   // mid-left hooked brace
            (r"\[braceleftmid]", r"⎨"), // mid-left hooked brace
            (r"\(lb", r"⎩"),   // bottom-left hooked brace
            (r"\[braceleftbt]", r"⎩"), // bottom-left hooked brace
            (r"\[braceleftex]", r"⎪"), // left hooked brace extension
            (r"\(rt", r"⎫"),   // top-right hooked brace
            (r"\[bracerighttp]", r"⎫"), // top-right hooked brace
            (r"\(rk", r"⎬"),   // mid-right hooked brace
            (r"\[bracerightmid]", r"⎬"), // mid-right hooked brace
            (r"\(rb", r"⎭"),   // bottom-right hooked brace
            (r"\[bracerightbt]", r"⎭"), // bottom-right hooked brace
            (r"\[bracerightex]", r"⎪"), // right hooked brace extension
            (r"\[parenlefttp]", r"⎛"),  // top-left hooked parenthesis
            (r"\[parenleftbt]", r"⎝"),  // bottom-left hooked parenthesis
            (r"\[parenleftex]", r"⎜"),  // left hooked parenthesis extension
            (r"\[parenrighttp]", r"⎞"), // top-right hooked parenthesis
            (r"\[parenrightbt]", r"⎠"), // bottom-right hooked parenthesis
            (r"\[parenrightex]", r"⎟"), // right hooked parenthesis extension

            // Arrows:
            (r"\(<-", r"←"),   // left arrow
            (r"\(->", r"→"),   // right arrow
            (r"\(<>", r"↔"),   // left-right arrow
            (r"\(da", r"↓"),   // down arrow
            (r"\(ua", r"↑"),   // up arrow
            (r"\(va", r"↕"),   // up-down arrow
            (r"\(lA", r"⇐"),   // left double-arrow
            (r"\(rA", r"⇒"),   // right double-arrow
            (r"\(hA", r"⇔"),   // left-right double-arrow
            (r"\(uA", r"⇑"),   // up double-arrow
            (r"\(dA", r"⇓"),   // down double-arrow
            (r"\(vA", r"⇕"),   // up-down double-arrow
            (r"\(an", r"⎯"),   // horizontal arrow extension

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
            (r"\-", r"-"),          // minus (text font)
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
            (r"\(<=", r"≤"),        // less-than-equal
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
            (r"\[integral]", r"∫"),   // integral
            (r"\[sum]", r"∑"),        // summation
            (r"\[product]", r"∏"),    // product
            (r"\[coproduct]", r"∐"),  // coproduct
            (r"\(gr", r"∇"),         // gradient
            (r"\(sr", r"√"),         // square root
            (r"\[sqrt]", r"√"),       // square root
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
            (r"\[hbar]", r"ℏ"),       // Planck constant over 2π
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
            (r"\(,C", r"Ç"),  // cedilla C
            (r"\(,c", r"ç"),  // cedilla c
            (r"\(/L", r"Ł"),  // stroke L
            (r"\(/l", r"ł"),  // stroke l
            (r"\(/O", r"Ø"),  // stroke O
            (r"\(/o", r"ø"),  // stroke o
            (r"\(oA", r"Å"),  // ring A
            (r"\(oa", r"å"),  // ring a

            // Special letters:
            (r"\(-D", r"Ð"),  // Eth
            (r"\(Sd", r"ð"),  // eth
            (r"\(TP", r"Þ"),  // Thorn
            (r"\(Tp", r"þ"),  // thorn
            (r"\(.i", r"ı"),  // dotless i
            (r"\(.j", r"ȷ"),  // dotless j

            // Currency:
            (r"\(Do", r"$"),  // dollar
            (r"\(ct", r"¢"),  // cent
            (r"\(Eu", r"€"),  // Euro symbol
            (r"\(eu", r"€"),  // Euro symbol
            (r"\(Ye", r"¥"),  // yen
            (r"\(Po", r"£"),  // pound
            (r"\(Cs", r"¤"),  // Scandinavian
            (r"\(Fn", r"ƒ"),  // florin

            // Units:
            (r"\(de", r"°"),  // degree
            (r"\(%0", r"‰"),  // per-thousand
            (r"\(fm", r"′"),  // minute
            (r"\(sd", r"″"),  // second
            (r"\(mc", r"µ"),  // micro
            (r"\(Of", r"ª"),  // Spanish female ordinal
            (r"\(Om", r"º"),  // Spanish masculine ordinal

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
            (r"\*(Ba", r"|"),    // vertical bar
            (r"\*(Ne", r"≠"),    // not equal
            (r"\*(Ge", r"≥"),    // greater-than-equal
            (r"\*(Le", r"≤"),    // less-than-equal
            (r"\*(Gt", r">"),    // greater-than
            (r"\*(Lt", r"<"),    // less-than
            (r"\*(Pm", r"±"),    // plus-minus
            (r"\*(If", r"infinity"), // infinity
            (r"\*(Pi", r"pi"),   // pi
            (r"\*(Na", r"NaN"),  // NaN
            (r"\*(Am", r"&"),    // ampersand
            (r"\*R", r"®"),      // restricted mark
            (r"\*(Tm", r"(Tm)"), // trade mark
            (r"\*q", "\""),      // double-quote
            (r"\*(Rq", r"”"),    // right-double-quote
            (r"\*(Lq", r"“"),    // left-double-quote
            (r"\*(lp", r"("),    // right-parenthesis
            (r"\*(rp", r")"),    // left-parenthesis
            (r"\*(lq", r"“"),    // left double-quote
            (r"\*(rq", r"”"),    // right double-quote
            (r"\*(ua", r"↑"),    // up arrow
            (r"\*(va", r"↕"),    // up-down arrow
            (r"\*(<=", r"≤"),    // less-than-equal
            (r"\*(>=", r"≥"),    // greater-than-equal
            (r"\*(aa", r"´"),    // acute
            (r"\*(ga", r"`"),    // grave
            (r"\*(Px", r"POSIX"),// POSIX standard name
            (r"\*(Ai", r"ANSI"), // ANSI standard name
        ].iter().cloned().collect();

        let mut result = String::new();

        let ac = AhoCorasick::new(replacements.keys())
            .expect("Build error");

        ac.replace_all_with(text, &mut result, |_, key, dst| {
            dst.push_str(replacements[key]);
            true
        });

        self.replace_unicode_escapes(&result) 
    }
}

// Formatting block partial-explicit.
// impl MdocFormatter {
//     fn format_a_block(&self, macro_node: MacroNode) -> String {

//     }
// }

// Formatting Rs-Re bloock. Can contain only %* macros
// Notes:
//  - All macros are comma separated.
//  - Before the last '%A' macro has to be 'and' word. 
//  - These macros have order!
impl MdocFormatter {
    fn format_rs_block(&self, macro_node: MacroNode) -> String {
        unimplemented!()
    }

    fn format_d(&self, month_day: Option<String>, year: i32) -> String {
        match month_day {
            Some(md) => format!("{md} {year}"),
            None => format!("{year}")
        }
    }

    fn format_p(&self, macro_node: MacroNode) -> String {
        macro_node.nodes.iter().map(|node| {
            match node {
                Element::Text(text) => self.format_text_node(text),
                _ => unreachable!(".%P macro can not contain macro node or EOI!")
            }
        }).collect::<String>()
    }
}

// Format other in-line macros.
impl MdocFormatter {
    fn format_ad(&self, macro_node: MacroNode) -> String {
        macro_node.nodes.iter().map(|node| {
            match node {
                Element::Text(text) => self.format_text_node(text),
                _ => unreachable!(".Ad macro can not contain macro node or EOI!")
            }
        }).collect::<String>()
    }

    fn format_b(&self, book_title: &str) -> String {
        self.format_text_node(book_title)
    }

    fn format_t(&self, article_title: &str) -> String {
        self.format_text_node(article_title)
    }

    fn format_u(&self, uri: &str) -> String {
        self.format_text_node(uri)
    }

    fn format_ap(&self) -> String {
        "'".to_string()
    }

    fn format_ar(&self, macro_node: MacroNode) -> String {
        if macro_node.nodes.is_empty() {
            return "file ...".to_string();
        }

        macro_node.nodes.iter().map(|node| {
            match node {
                Element::Text(text) => self.format_text_node(text),
                _ => unreachable!(".Ar can not contain macro or EOI in subnodes!")
            }
        }).collect::<String>()
    }

    fn format_bt(&self) -> String {
        "is currently in beta test.".to_string()
    }

    fn format_cd(&self, macro_node: MacroNode) -> String {
        macro_node.nodes.iter().map(|node| {
            match node {
                Element::Text(text) => self.format_text_node(text),
                _ => unreachable!(".Cd macro can not contain macro node or EOI!")
            }
        }).collect::<String>()
    }

    fn format_cm(&self, macro_node: MacroNode) -> String {
        macro_node.nodes.iter().map(|node| {
            match node {
                Element::Text(text) => self.format_text_node(text),
                _ => unreachable!(".Cm macro can not contain macro node or EOI!")
            }
        }).collect::<String>()
    }
    
    fn format_db(&self) -> String {
        "".to_string()
    }

    fn format_dv(&self, macro_node: MacroNode) -> String {
        macro_node.nodes.iter().map(|node| {
            match node {
                Element::Text(text) => self.format_text_node(text),
                _ => unreachable!(".Dv macro can not contain macro node or EOI!")
            }
        }).collect::<String>()
    }

    fn format_em(&self, macro_node: MacroNode) -> String {
        let line = macro_node.nodes.iter().map(|node| {
            match node {
                Element::Text(text) => self.format_text_node(text),
                _ => unreachable!(".Em macro can not contain macro node or EOI!")
            }
        }).collect::<String>();

        if self.supports_italic() {
            format!("\x1b[3m{line}\x1b[0m")
        } else if self.supports_underline() {
            format!("\x1b[4m{line}\x1b[0m")
        } else {
            line
        }
    }

    fn format_dt(&self, title: Option<String>, section: String, arch: Option<String>) -> String {
        let title = match title {
            Some(name) => format!("{name}({section})"),
            None => format!("UNTITLED({section})")
        };

        let section = match section.as_str() {
            "1" => "General Commands Manual",
            "2" => "System Calls Manual",
            "3" => "Library Functions Manual",
            "4" => "Device Drivers Manual",
            "5" => "File Formats Manual",
            "6" => "Games Manual",
            "7" => "Miscellaneous Information Manual",
            "8" => "System Manager's Manual",
            "9" => "Kernel Developer's Manual",
            _   => ""
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

        line
    }

}

#[cfg(test)]
mod tests {
    mod special_chars {
        use crate::{man_util::{formatter::MdocFormatter, parser::{MdocDocument, MdocParser}}, FormattingSettings};

        fn get_ast(input: &str) -> MdocDocument {
            MdocParser::parse_mdoc(input).unwrap()
        }

        #[test]
        fn test_spaces() {
            let input = r"\ \~\0\|\^\&\)\%\:";
            let output = r"     ".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_lines() {
            let input = r"\(ba \(br \(ul \(ru \(rn \(bb \(sl \(rs";
            let output = r"| │ _ _ ‾ ¦ / \".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_text_markers() {
            let input = r"\(ci \(bu \(dd \(dg \(lz \(sq \(ps \(sc \(lh \(rh \(at \(sh \(CR \(OK \(CL \(SP \(HE \(DI";
            let output = r"○ • ‡ † ◊ □ ¶ § ☜ ☞ @ # ↵ ✓ ♣ ♠ ♥ ♦".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_legal_symbols() {
            let input = r"\(co \(rg \(tm";
            let output = r"© ® ™".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_punctuation() {
            let input = r"\(em \(en \(hy \e \(r! \(r?";
            let output = r"— – ‐ \\ ¡ ¿".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_quotes() {
            let input = r"\(Bq \(bq \(lq \(rq \(oq \(cq \(aq \(dq \(Fo \(Fc \(fo \(fc";
            let output = "„ ‚ “ ” ‘ ’ ' \" « » ‹ ›".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_brackets() {
            let input = r"\(lB \(rB \(lC \(rC \(la \(ra \(bv \[braceex] \[bracketlefttp] \[bracketleftbt] 
\[bracketleftex] \[bracketrighttp] \[bracketrightbt] \[bracketrightex] 
\(lt \[bracelefttp] \(lk \[braceleftmid] \(lb \[braceleftbt] \[braceleftex] 
\(rt \[bracerighttp] \(rk \[bracerightmid] \(rb \[bracerightbt] \[bracerightex] 
\[parenlefttp] \[parenleftbt] \[parenleftex] \[parenrighttp] \[parenrightbt] \[parenrightex]
";
            let output = r"[ ] { } ⟨ ⟩ ⎪ ⎪ ⎡ ⎣ ⎢ ⎤ ⎦ ⎥ ⎧ ⎧ ⎨ ⎨ ⎩ ⎩ ⎪ ⎫ ⎫ ⎬ ⎬ ⎭ ⎭ ⎪ ⎛ ⎝ ⎜ ⎞ ⎠ ⎟".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_arrows() {
            let input = r"\(<- \(-> \(<> \(da \(ua \(va \(lA \(rA \(hA \(uA \(dA \(vA \(an";
            let output = r"← → ↔ ↓ ↑ ↕ ⇐ ⇒ ⇔ ⇑ ⇓ ⇕ ⎯".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_logical() {
            let input = r"\(AN \(OR \[tno] \(no \(te \(fa \(st \(tf \(3d \(or";
            let output = r"∧ ∨ ¬ ¬ ∃ ∀ ∋ ∴ ∴ |".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_mathematical() {
            let input = r"\- \(mi \+ \(pl \(-+ \[t+-] \(+- \(pc \[tmu] 
\(mu \(c* \(c+ \[tdi] \(di \(f/ \(** \(<= \(>= \(<< \(>> \(eq \(!= \(== 
\(ne \(ap \(|= \(=~ \(~~ \(~= \(pt \(es \(mo \(nm \(sb \(nb \(sp 
\(nc \(ib \(ip \(ca \(cu \(/_ \(pp \(is \[integral] \[sum] \[product] 
\[coproduct] \(gr \(sr \[sqrt] \(lc \(rc \(lf \(rf \(if \(Ah \(Im \(Re 
\(wp \(pd \(-h \[hbar] \(12 \(14 \(34 \(18 \(38 \(58 \(78 \(S1 \(S2 \(S3
";
            let output = r"- − + + ∓ ± ± · × × ⊗ ⊕ ÷ ÷ ⁄ ∗ ≤ ≥ ≪ ≫ = ≠ ≡ ≢ ∼ ≃ ≅ ≈ ≈ ∝ ∅ ∈ ∉ ⊂ ⊄ ⊃ ⊅ ⊆ ⊇ ∩ ∪ ∠ ⊥ ∫ ∫ ∑ ∏ ∐ ∇ √ √ ⌈ ⌉ ⌊ ⌋ ∞ ℵ ℑ ℜ ℘ ∂ ℏ ℏ ½ ¼ ¾ ⅛ ⅜ ⅝ ⅞ ¹ ² ³".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_ligatures() {
            let input = r"\(ff \(fi \(fl \(Fi \(Fl \(AE \(ae \(OE \(oe \(ss \(IJ \(ij";
            let output = r"ﬀ ﬁ ﬂ ﬃ ﬄ Æ æ Œ œ ß Ĳ ĳ".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_accents() {
            let input = "\\(a\" \\(a- \\(a. \\(a^ \\(aa \\\' \\(ga \\` \\(ab \\(ac \\(ad \\(ah \\(ao \\(a~ \\(ho \\(ha \\(ti";
            let output = r"˝ ¯ ˙ ^ ´ ´ ` ` ˘ ¸ ¨ ˇ ˚ ~ ˛ ^ ~".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_accented_letters() {
            let input = r"\('A \('E \('I \('O \('U \('Y \('a \('e 
\('i \('o \('u \('y \(`A \(`E \(`I \(`O \(`U \(`a \(`e \(`i \(`o \(`u 
\(~A \(~N \(~O \(~a \(~n \(~o \(:A \(:E \(:I \(:O \(:U \(:a \(:e \(:i 
\(:o \(:u \(:y \(^A \(^E \(^I \(^O \(^U \(^a \(^e \(^i \(^o \(^u \(,C 
\(,c \(/L \(/l \(/O \(/o \(oA \(oa
";
            let output = r"Á É Í Ó Ú Ý á é í ó ú ý À È Ì Ò Ù à è ì ò ù Ã Ñ Õ ã ñ õ Ä Ë Ï Ö Ü ä ë ï ö ü ÿ Â Ê Î Ô Û â ê î ô û Ç ç Ł ł Ø ø Å å".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }
        
        #[test]
        fn test_special_letters() {
            let input = r"\(-D \(Sd \(TP \(Tp \(.i \(.j";
            let output = r"Ð ð Þ þ ı ȷ".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_currency() {
            let input = r"\(Do \(ct \(Eu \(eu \(Ye \(Po \(Cs \(Fn";
            let output = r"$ ¢ € € ¥ £ ¤ ƒ".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_units() {
            let input = r"\(de \(%0 \(fm \(sd \(mc \(Of \(Om";
            let output = r"° ‰ ′ ″ µ ª º".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_greek_leters() {
            let input = r"\(*A \(*B \(*G \(*D \(*E \(*Z 
\(*Y \(*H \(*I \(*K \(*L \(*M \(*N \(*C \(*O \(*P \(*R \(*S 
\(*T \(*U \(*F \(*X \(*Q \(*W \(*a \(*b \(*g \(*d \(*e \(*z 
\(*y \(*h \(*i \(*k \(*l \(*m \(*n \(*c \(*o \(*p \(*r \(*s 
\(*t \(*u \(*f \(*x \(*q \(*w \(+h \(+f \(+p \(+e \(ts
";
            let output = r"Α Β Γ Δ Ε Ζ Η Θ Ι Κ Λ Μ Ν Ξ Ο Π Ρ Σ Τ Υ Φ Χ Ψ Ω α β γ δ ε ζ η θ ι κ λ μ ν ξ ο π ρ σ τ υ ϕ χ ψ ω ϑ φ ϖ ϵ ς".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_predefined_strings() {
            let input = r"\*(Ba \*(Ne \*(Ge \*(Le \*(Gt \*(Lt \*(Pm \*(If \*(Pi \*(Na \*(Am \*R \*(Tm \*q \*(Rq \*(Lq \*(lp \*(rp \*(lq \*(rq \*(ua \*(va \*(<= \*(>= \*(aa \*(ga \*(Px \*(Ai";
            let output = "| ≠ ≥ ≤ > < ± infinity pi NaN & ® (Tm) \" ” “ ( ) “ ” ↑ ↕ ≤ ≥ ´ ` POSIX ANSI".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_unicode() {
            let input = r"\[u0100] \C'u01230' \[u025600]";
            let output = "Ā ሰ 𥘀".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }

        #[test]
        fn test_numbered() {
            let input = r"\N'34' \[char43]";
            let output = "\" +".to_string();
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }
    }

    mod macros {
        use crate::{man_util::{formatter::MdocFormatter, parser::{MdocDocument, MdocParser}}, FormattingSettings};

        fn get_ast(input: &str) -> MdocDocument {
            MdocParser::parse_mdoc(input).unwrap()
        }

        #[test]
        fn test_dt() {
            let input = ".Dt TITLE 7 arch";
            let output = "TITLE(7)            Miscellaneous Information Manual (arch)           TITLE(7)";
            let ast = get_ast(input);

            let formatting_settings = FormattingSettings { width: 78, indent: 5 };
            let formatter = MdocFormatter::new(formatting_settings);
            let result = String::from_utf8(formatter.format_mdoc(ast)).unwrap();
            assert_eq!(output, result)
        }
    }
}