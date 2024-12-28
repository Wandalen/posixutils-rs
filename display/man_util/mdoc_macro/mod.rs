//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use text_production::{AtAndTUnix, Bsd, BsdOs, DragonFly, FreeBsd, NetBsd, OpenBsd, Standard};
use types::*;

pub mod text_production;
pub mod types;

#[derive(Debug, PartialEq)]
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
    Ef, // End a display context started by Bf
    Bk,
    Ek, // End a keep context started by Bk
    Bl {
        list_type: BlType,
        offset: Option<OffsetType>,
        compact: bool,
        columns: Vec<String>,
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
        month: String,
        day: u8,
        year: i32,
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
        // https://man.openbsd.org/mdoc#Mt
        mail_to: String,
    },
    Nd {
        line: String,
    },
    Nm {
        name: Option<Vec<String>>,
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
    },
}
