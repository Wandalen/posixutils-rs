//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use text_production::{AtAndTUnix, Bsd, BsdOs, DragonFly, FreeBsd, NetBsd, OpenBsd, Standard};

pub mod text_production;

pub enum BdType {
    Centered,
    Filled,
    Literal,
    Ragged,
    Unfilled,
}

pub enum OffsetType {
    Indent,
    IndentTwo,
    Left,
    Right,
}

pub enum BfType {
    Emphasis, // Or Em
    Literal, // Or Li
    Symboic, // Or Sy
}

pub enum BlType {
    Bullet,
    Column,
    Dash, // Or "hyphen"
    Diag,
    Enum,
    Hyphen,
    Inset,
    Item,
    Ohang,
    Tag,
}

pub enum ItType {
    MandatoryArgs(Vec<String>),
    OptionalArgs(Vec<String>),
    None,
    Cell {
        // TODO: https://man.openbsd.org/mdoc#It
    },
}

pub enum SmMode {
    On,
    Off,
}

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
    // Book or journal page number of an Rs block. Conventionally, the argument starts with ‘p.’ for a single page or ‘pp.’ for a range of pages, for example:
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

pub enum Macro {
    Ad {
        address: String,
    },
    An {
        split: bool,
        author_names: Vec<String>,
    },
    Ao, // Begin a block enclosed by angle brackets
    Ac, // Close an Ao block
    Ap,
    Aq {
        line: String,
    },
    Ar {
        // Command arguments. If an argument is not provided, the string “file ...” is used as a default.
        // https://man.openbsd.org/mdoc#Ar
        placeholder: Vec<String>,
    },
    At(AtAndTUnix),
    Bd {
        block_type: BdType,
        offset: Option<OffsetType>,
        compact: bool,
    },
    Ed, // End a display context started by Bd
    Bf(BfType),
    Ef, // End a display context started by Bd
    Bk,
    Ek, // End a keep context started by Bk
    Bl {
        list_type: BlType,
        offset : Option<OffsetType>,
        compact : bool,
    },
    El, // End a list context started by Bl
    Bo,
    Bc, // Close a Bo block
    Bq,
    Bro,
    Brc, // Close a Bro block
    Brq,
    Bsx(BsdOs),
    Bt,
    Bx(Bsd),
    Cd {
        line: String,
    },
    Cm {
        keywords: Vec<String>,
    },
    D1 {
        line: String,
    },
    Db, // Obsolete
    Dd {
        // TODO: $Mdocdate$ | month day, year
    },
    Dl {
        line: String,
    },
    Do,
    Dc, // Close a Do block
    Dq,
    Dt {
        title: String,
        section: String,
        arch: Option<String>,
    },
    Dv {
        identifiers: Vec<String>,
    },
    Dx(DragonFly),
    En {
        // Obsolete
        words: Vec<String>,
    },
    Eo {
        opening_delimiter: Option<char>,
    },
    Ec {
        // Close a scope started by Eo
        closing_dilimiter: Option<char>,
    },
    Er {
        identifiers: Vec<String>,
    },
    Es {
        // Obsolete
        opening_delimiter: char,
        closing_delimiter: char,
    },
    Ev {
        identifiers: Vec<String>,
    },
    Ex {
        utilities: Vec<String>,
    },
    Fa {
        arguments: Vec<String>,
    },
    Fd {
        directive: String,
        arguments: Vec<String>,
    },
    Fl {
        words: Vec<String>,
    },
    Fn {
        func_name: String,
        arguments: Vec<String>,
    },
    Fo {
        func_name: String,
    },
    Fc, // End a function context started by Fo
    Fr {
        number: i32,
    },
    Ft {
        func_type: String,
    },
    Fx(FreeBsd),
    Hf {
        // TODO: Not implemented???
        // https://man.openbsd.org/mdoc#Hf
        file_name: String,
    },
    Ic {
        keywords: Vec<String>,
    },
    In {
        file_name: String,
    },
    // TODO: Finish https://man.openbsd.org/mdoc#It
    It(ItType),
    Lb {
        lib_name: String,
    },
    Li {
        words: Vec<String>,
    },
    Lk {
        uri: String,
        display_name: Option<String>,
    },
    Mt {
        // TODO: Parse??
        // https://man.openbsd.org/mdoc#Mt
        mail_to: String
    },
    Nd {
        line: String,
    },
    Nm {
        name: Option<String>,
    },
    No {
        words: Vec<String>,
    },
    Ns,
    Nx(NetBsd),
    Oo,
    Oc, // Close multi-line Oo context
    Op {
        line: String,
    },
    Os {
        fotter_text: Option<String>
    },
    Ot {
        func_type: String,
    },
    Ox(OpenBsd),
    Pa {
        names: Vec<String>,
    },
    Pf {
        prefix: String,
        macro_name: String,
        arguments: Vec<String>,
    },
    Po,
    Pc, // Close parenthesised context opened by Po
    Pp,
    Pq {
        line: String,
    },
    Ql {
        line: String,
    },
    Qo,
    Qc, // Close quoted context opened by Qo
    Rs(Vec<RsSubmacro>),
    Re, // Close an Rs block
    Rv {
        functions: Vec<String>,
    },
    Sh {
        title: String,
    },
    Sm(SmMode),
    So,
    Sc, // Close single-quoted context opened by So
    Sq {
        line: String,
    },
    Ss {
        title: String,
    },
    St(Standard),
    Sx {
        title: String,
    },
    Sy {
        words: Vec<String>,
    },
    Ta,
    Tg {
        term: Option<String>,
    },
    Tn {
        words: Vec<String>,
    },
    Ud,
    Ux,
    Va {
        func_type: Option<String>,
        identifier: Vec<String>,
    },
    Vt {
        var_type: String,
        identifier: Option<String>,
    },
    Xo,
    Xc, // Close a scope opened by Xo
    Xr {
        name: String,
        section: String,
    }
}
