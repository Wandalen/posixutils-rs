//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use crate::man_util::parser::Element;
use text_production::{AtType, BsxType, BxType, DxType, FxType, NxType, OxType, StType};
use types::*;

pub mod text_production;
pub mod types;

#[derive(Debug, PartialEq)]
pub enum Macro {
    A {
        author_name: String,
    },
    B {
        book_title: String,
    },
    C {
        publication_location: String,
    },
    D {
        month_day: Option<(String, u8)>,
        year: i32,
    },
    I {
        issuer_name: String,
    },
    J {
        journal_name: String,
    },
    N {
        issue_number: String,
    },
    O {
        information: String,
    },
    P,
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
        volume_number: String,
    },
    Ad,
    An {
        author_name_type: AnType,
    },
    Ao, // Begin a block enclosed by angle brackets
    Ac, // Close an Ao block
    Ap,
    Aq,
    Ar,
    At(AtType),
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
    Bsx(BsxType),
    Bt,
    Bx(BxType),
    Cd,
    Cm,
    D1,
    Db, // Obsolete
    Dd {
        date: DdDate
    },
    Dl,
    Do,
    Dc, // Close a Do block
    Dq,
    Dt {
        title: Option<String>,
        section: String,
        arch: Option<String>,
    },
    Dv,
    Dx(DxType),
    Em,
    En,
    Eo {
        opening_delimiter: Option<char>,
        closing_delimiter: Option<char>,
    },
    Ec,
    Er,
    Es { // Obsolete
        opening_delimiter: char,
        closing_delimiter: char,
    },
    Ev,
    Ex,
    Fa,
    Fd {
        directive: String,
        arguments: Vec<String>,
    },
    Fl,
    Fn {
        funcname: String
    },
    Fo {
        funcname: String,
    },
    Fc, // End a function context started by Fo
    Fr, // Obsolete
    Ft,
    Fx(FxType),
    Hf,
    Ic,
    In {
        filename: String,
    },
    It {
        args: Vec<Element>
    },
    Lb {
        lib_name: String,
    },
    Li,
    Lk {
        uri: String,
    },
    Lp,
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
    No {
        words: Vec<String>,
    },
    Ns,
    Nx(NxType),
    Oo,
    Oc, // Close multi-line Oo context
    Op,
    Os {
        fotter_text: Option<String>,
    },
    Ot {
        func_type: String,
    },
    Ox(OxType),
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
    Pq,
    Ql,
    Qo,
    Qc, // Close quoted context opened by Qo
    Qq,
    Rs,
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
    Sq,
    Ss {
        title: String,
    },
    St(StType),
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
    Vt,
    Xo,
    Xc, // Close a scope opened by Xo
    Xr {
        name: String,
        section: String,
    },
}
