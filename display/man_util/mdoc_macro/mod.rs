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

pub enum Macro {
    RsA {
        author_names: Vec<String>,
    },
    RsB {
        book_title: String,
    },
    RsC {
        publication_location: String,
    },
    RsD {
        month_day: Option<(String, String)>,
        year: i32,
    },
    RsI {
        issuer_name: String,
    },
    RsJ {
        journal_name: String,
    },
    RsN {
        issue_number: u16,
    },
    RsO {
        information: String,
    },
    // Book or journal page number of an Rs block. Conventionally, the argument starts with ‘p.’ for a single page or ‘pp.’ for a range of pages, for example:
    RsP {
        page_number: String,
    },
    RsQ {
        insitution_author: String,
    },
    RsR {
        report_name: String,
    },
    RsT {
        article_title: String,
    },
    RsU {
        uri: String,
    },
    RsV {
        volume_number: u16,
    },
    Ad {
        address: String,
    },
    An {
        split: bool,
        author_names: Vec<String>,
    },
    Ao,
    Ac, // Close an Ao block
    Ap,
    Aq {
        line: String,
    },
    Ar {
        // If input is absent, use "file ..." string.
        // https://man.openbsd.org/mdoc#Ar
        placeholder: String,
    },
    // Text production
    At(AtAndTUnix),
    Bx(Bsd),
    Bsx(BsdOs),
    Nx(NetBsd),
    Fx(FreeBsd),
    Ox(OpenBsd),
    Dx(DragonFly),
    St(Standard),
    // Ex(),
    // Rv(),
}
