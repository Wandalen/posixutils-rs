//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use pest::iterators::Pair;

use crate::man_util::parser::Rule;

use chrono::Datelike;

#[derive(Debug, Clone, PartialEq)]
pub enum BdType {
    Centered,
    Filled,
    Literal,
    Ragged,
    Unfilled,
}

impl From<Pair<'_, Rule>> for BdType {
    fn from(pair: Pair<'_, Rule>) -> Self {
        match pair.into_inner().next().unwrap().as_rule() {
            Rule::bd_centered => Self::Centered,
            Rule::bd_filled => Self::Filled,
            Rule::bd_literal => Self::Literal,
            Rule::bd_ragged => Self::Ragged,
            Rule::bd_unfilled => Self::Unfilled,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OffsetType {
    Indent,
    IndentTwo,
    Left,
    Right,
    Center,
}

impl From<Pair<'_, Rule>> for OffsetType {
    fn from(pair: Pair<'_, Rule>) -> Self {
        match pair.into_inner().next().unwrap().as_rule() {
            Rule::off_indent => Self::Indent,
            Rule::off_indent_two => Self::IndentTwo,
            Rule::off_left => Self::Left,
            Rule::off_right => Self::Right,
            Rule::off_center => Self::Center,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BfType {
    Emphasis,
    Literal,
    Symbolic,
}

impl From<Pair<'_, Rule>> for BfType {
    fn from(pair: Pair<'_, Rule>) -> Self {
        match pair.into_inner().next().unwrap().as_rule() {
            Rule::bf_emphasis | Rule::bf_em => Self::Emphasis,
            Rule::bf_literal | Rule::bf_li => Self::Literal,
            Rule::bf_symbolic | Rule::bf_sy => Self::Symbolic,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlType {
    Bullet,
    Column,
    Dash,
    Diag,
    Enum,
    Hang,
    Inset,
    Item,
    Ohang,
    Tag,
}

impl From<Pair<'_, Rule>> for BlType {
    fn from(pair: Pair<'_, Rule>) -> Self {
        match pair.into_inner().next().unwrap().as_rule() {
            Rule::bl_bullet => Self::Bullet,
            Rule::bl_column => Self::Column,
            Rule::bl_dash | Rule::bl_hyphen => Self::Dash,
            Rule::bl_diag => Self::Diag,
            Rule::bl_enum => Self::Enum,
            Rule::bl_hang => Self::Hang,
            Rule::bl_inset => Self::Inset,
            Rule::bl_item => Self::Item,
            Rule::bl_ohang => Self::Ohang,
            Rule::bl_tag => Self::Tag,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnType {
    Split,
    NoSplit,
    Name,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SmMode {
    On,
    Off,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Date {
    pub month_day: (String, u8),
    pub year: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DdDate {
    MDYFormat(Date),
    StrFormat(String),
}

impl From<chrono::NaiveDate> for DdDate {
    fn from(date: chrono::NaiveDate) -> DdDate {
        let month = match date.month() {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => unreachable!(),
        };

        DdDate::MDYFormat(Date {
            month_day: (month.to_string(), date.day() as u8),
            year: date.year() as u16,
        })
    }
}
