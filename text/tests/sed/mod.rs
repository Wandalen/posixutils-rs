//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use plib::testing::{run_test, TestPlan};

fn sed_test(
    args: &[&str],
    test_data: &str,
    expected_output: &str,
    expected_err: &str,
    expected_exit_code: i32,
) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    run_test(TestPlan {
        cmd: String::from("sed"),
        args: str_args,
        stdin_data: String::from(test_data),
        expected_out: String::from(expected_output),
        expected_err: String::from(expected_err),
        expected_exit_code,
    });
}

const ABC_INPUT: &'static str = "abc";
const SCRIPT_A: &'static str = "s/a/ab/g";
const SCRIPT_B: &'static str = "s/b/bc/g";
const SCRIPT_C: &'static str = "s/c/ca/g";
const SCRIPT_SOME_NEWLINES: &'static str = "s/a/ab/g\ns/b/bc/g\ns/c/ca/g\n\n\n";
const SCRIPT_ALL_NEWLINES: &'static str = "\n\n\n";
const SCRIPT_BLANKS: &'static str = "   s/a/ab/g\n   s/b/bc/g\n   s/c/ca/g";
const SCRIPT_SEMICOLONS: &'static str = ";;;s/a/ab/g\n;;;s/b/bc/g\n;;;s/c/ca/g";

const ABC_FILE: &'static str = "tests/sed/assets/abc";
const CBA_FILE: &'static str = "tests/sed/assets/cba";
const SCRIPT_A_FILE: &'static str = "tests/sed/assets/script_a";
const SCRIPT_B_FILE: &'static str = "tests/sed/assets/script_b";
const SCRIPT_C_FILE: &'static str = "tests/sed/assets/script_c";
const SCRIPT_SOME_NEWLINES_FILE: &'static str = "tests/sed/assets/script_some_newlines";
const SCRIPT_ALL_NEWLINES_FILE: &'static str = "tests/sed/assets/script_all_newlines";
const SCRIPT_BLANKS_FILE: &'static str = "tests/sed/assets/script_blanks";
const SCRIPT_SEMICOLONS_FILE: &'static str = "tests/sed/assets/script_blanks";

#[test]
fn test_no_arguments() {
    sed_test(&[], "", "", "sed: none script was supplied\n", 1);
}

#[test]
fn test_single_script_input_stdin() {
    sed_test(&[SCRIPT_A], ABC_INPUT, "abbc", "", 0);
}

#[test]
fn test_single_script_input_file() {
    sed_test(&[SCRIPT_A, ABC_FILE], "", "abbc", "", 0);
}

#[test]
fn test_e_script_input_stdin() {
    sed_test(&["-e", SCRIPT_A], ABC_INPUT, "abbc", "", 0);
}

#[test]
fn test_e_script_input_file() {
    sed_test(&["-e", SCRIPT_A, ABC_FILE], "", "abbc", "", 0);
}

#[test]
fn test_f_script_input_stdin() {
    sed_test(&["-f", SCRIPT_A_FILE], ABC_INPUT, "abbc", "", 0);
}

#[test]
fn test_f_script_input_file() {
    sed_test(&["-f", SCRIPT_A_FILE, ABC_FILE], "", "abbc", "", 0);
}

#[test]
fn test_e_f_scripts_input_stdin() {
    sed_test(
        &["-e", SCRIPT_A, "-f", SCRIPT_B_FILE],
        ABC_INPUT,
        "abcbcc",
        "",
        0,
    );
}

#[test]
fn test_e_f_scripts_input_file() {
    sed_test(
        &["-e", SCRIPT_A, "-f", SCRIPT_B_FILE, ABC_FILE],
        "",
        "abcbcc",
        "",
        0,
    );
}

#[test]
fn test_input_explicit_stdin() {
    sed_test(&[SCRIPT_A, "-"], ABC_INPUT, "abbc", "", 0);
}

#[test]
fn test_ignore_stdin_without_dash() {
    sed_test(&[SCRIPT_A, CBA_FILE], ABC_INPUT, "cbab", "", 0);
}

#[test]
fn test_input_file_and_explicit_stdin() {
    // Reorderind STDIN and input file
    sed_test(&[SCRIPT_A, "-", CBA_FILE], ABC_INPUT, "abbc\ncbab", "", 0);
    sed_test(&[SCRIPT_A, CBA_FILE, "-"], ABC_INPUT, "cbab\nabbc", "", 0);
}

#[test]
fn test_single_script_multiple_input_files() {
    // Reorderind input files
    sed_test(&[SCRIPT_A, ABC_FILE, CBA_FILE], "", "abbc\ncbab", "", 0);
    sed_test(&[SCRIPT_A, CBA_FILE, ABC_FILE], "", "cbab\nabbc", "", 0);
}

#[test]
fn test_e_scripts_multiple_input_files() {
    // Reorderind input files
    sed_test(
        &["-e", SCRIPT_A, "-e", SCRIPT_B, ABC_FILE, CBA_FILE],
        "",
        "abcbcc\ncbcabc",
        "",
        0,
    );
    sed_test(
        &["-e", SCRIPT_A, "-e", SCRIPT_B, CBA_FILE, ABC_FILE],
        "",
        "cbcabc\nabcbcc",
        "",
        0,
    );
}

#[test]
fn test_e_scripts_multiple_input_files_mixed_order() {
    // Reorderind input files
    sed_test(
        &[ABC_FILE, "-e", SCRIPT_A, CBA_FILE, "-e", SCRIPT_B],
        "",
        "abcbcc\ncbcabc",
        "",
        0,
    );
    sed_test(
        &[CBA_FILE, "-e", SCRIPT_A, ABC_FILE, "-e", SCRIPT_B],
        "",
        "cbcabc\nabcbcc",
        "",
        0,
    );
}

#[test]
fn test_f_scripts_multiple_input_files() {
    // Reorderind input files
    sed_test(
        &["-f", SCRIPT_A_FILE, "-f", SCRIPT_B_FILE, ABC_FILE, CBA_FILE],
        "",
        "abcbcc\ncbcabc",
        "",
        0,
    );
    sed_test(
        &["-f", SCRIPT_A_FILE, "-f", SCRIPT_B_FILE, CBA_FILE, ABC_FILE],
        "",
        "cbcabc\nabcbcc",
        "",
        0,
    );
}

#[test]
fn test_f_scripts_multiple_input_files_mixed_order() {
    // Reorderind input files
    sed_test(
        &[ABC_FILE, "-f", SCRIPT_A_FILE, CBA_FILE, "-f", SCRIPT_B_FILE],
        "",
        "abcbcc\ncbcabc",
        "",
        0,
    );
    sed_test(
        &[CBA_FILE, "-f", SCRIPT_A_FILE, ABC_FILE, "-f", SCRIPT_B_FILE],
        "",
        "cbcabc\nabcbcc",
        "",
        0,
    );
}

#[test]
fn test_e_scripts_unique_order_unique_results() {
    sed_test(
        &["-e", SCRIPT_A, "-e", SCRIPT_B, "-e", SCRIPT_C],
        ABC_INPUT,
        "abcabcaca",
        "",
        0,
    );
    sed_test(
        &["-e", SCRIPT_A, "-e", SCRIPT_C, "-e", SCRIPT_B],
        ABC_INPUT,
        "abcbcca",
        "",
        0,
    );
    sed_test(
        &["-e", SCRIPT_B, "-e", SCRIPT_A, "-e", SCRIPT_C],
        ABC_INPUT,
        "abbcaca",
        "",
        0,
    );
    sed_test(
        &["-e", SCRIPT_B, "-e", SCRIPT_C, "-e", SCRIPT_A],
        ABC_INPUT,
        "abbcabcab",
        "",
        0,
    );
    sed_test(
        &["-e", SCRIPT_C, "-e", SCRIPT_A, "-e", SCRIPT_B],
        ABC_INPUT,
        "abcbccabc",
        "",
        0,
    );
    sed_test(
        &["-e", SCRIPT_C, "-e", SCRIPT_B, "-e", SCRIPT_A],
        ABC_INPUT,
        "abbccab",
        "",
        0,
    );
}

#[test]
fn test_f_scripts_unique_order_unique_results() {
    sed_test(
        &[
            "-f",
            SCRIPT_A_FILE,
            "-f",
            SCRIPT_B_FILE,
            "-f",
            SCRIPT_C_FILE,
        ],
        ABC_INPUT,
        "abcabcaca",
        "",
        0,
    );
    sed_test(
        &[
            "-f",
            SCRIPT_A_FILE,
            "-f",
            SCRIPT_C_FILE,
            "-f",
            SCRIPT_B_FILE,
        ],
        ABC_INPUT,
        "abcbcca",
        "",
        0,
    );
    sed_test(
        &[
            "-f",
            SCRIPT_B_FILE,
            "-f",
            SCRIPT_A_FILE,
            "-f",
            SCRIPT_C_FILE,
        ],
        ABC_INPUT,
        "abbcaca",
        "",
        0,
    );
    sed_test(
        &[
            "-f",
            SCRIPT_B_FILE,
            "-f",
            SCRIPT_C_FILE,
            "-f",
            SCRIPT_A_FILE,
        ],
        ABC_INPUT,
        "abbcabcab",
        "",
        0,
    );
    sed_test(
        &[
            "-f",
            SCRIPT_C_FILE,
            "-f",
            SCRIPT_A_FILE,
            "-f",
            SCRIPT_B_FILE,
        ],
        ABC_INPUT,
        "abcbccabc",
        "",
        0,
    );
    sed_test(
        &[
            "-f",
            SCRIPT_C_FILE,
            "-f",
            SCRIPT_B_FILE,
            "-f",
            SCRIPT_A_FILE,
        ],
        ABC_INPUT,
        "abbccab",
        "",
        0,
    );
}

#[test]
fn test_mixed_e_f_scripts() {
    // -e script -f script -e script
    sed_test(
        &["-e", SCRIPT_A, "-f", SCRIPT_B_FILE, "-e", SCRIPT_C],
        ABC_INPUT,
        "abcabcaca",
        "",
        0,
    );
    // -f script -e script -f script
    sed_test(
        &["-f", SCRIPT_A_FILE, "-e", SCRIPT_C, "-f", SCRIPT_B_FILE],
        ABC_INPUT,
        "abcbcca",
        "",
        0,
    );
}

#[test]
fn test_script_some_newlines() {
    sed_test(&[SCRIPT_SOME_NEWLINES], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_script_all_newlines() {
    sed_test(&[SCRIPT_ALL_NEWLINES], ABC_INPUT, ABC_INPUT, "", 0);
}

#[test]
fn test_e_script_some_newlines() {
    sed_test(&["-e", SCRIPT_SOME_NEWLINES], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_e_script_all_newlines() {
    sed_test(&["-e", SCRIPT_ALL_NEWLINES], ABC_INPUT, ABC_INPUT, "", 0);
}

#[test]
fn test_f_script_some_newlines() {
    sed_test(
        &["-f", SCRIPT_SOME_NEWLINES_FILE],
        ABC_INPUT,
        "abcabcaca",
        "",
        0,
    );
}

#[test]
fn test_f_script_all_newlines() {
    sed_test(
        &["-f", SCRIPT_ALL_NEWLINES_FILE],
        ABC_INPUT,
        ABC_INPUT,
        "",
        0,
    );
}

#[test]
fn test_single_script_ignore_blank_chars() {
    sed_test(&[SCRIPT_BLANKS], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_e_script_ignore_blank_chars() {
    sed_test(&["-e", SCRIPT_BLANKS], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_f_script_ignore_blank_chars() {
    sed_test(&["-f", SCRIPT_BLANKS_FILE], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_single_script_ignore_semicolon_chars() {
    sed_test(&[SCRIPT_SEMICOLONS], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_e_script_ignore_semicolon_chars() {
    sed_test(&["-e", SCRIPT_SEMICOLONS], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_f_script_ignore_semicolon_chars() {
    sed_test(
        &["-f", SCRIPT_SEMICOLONS_FILE],
        ABC_INPUT,
        "abcabcaca",
        "",
        0,
    );
}


/*

sed [-n] [file]

sed -e '' [file]

sed -f [file]

sed [-n] -e '' -e '' -e '' [file]

sed [-n] -f -f -f [file]

sed [-n] -e '' -f [file]

sed [-n] -e '' -e '' -e '' -f -f -f [file]

sed  -f -f -f -e '' -e '' -e '' [file]

sed [-n] -e '' -f -e '' -f -e '' [file]

sed [-n] -e '' -f -f -e '' -f -f -e '' [file]

sed [-n] -e '' -e '' -f -e '' -e '' -f [file]

+ ngpw wfile
- ngpw
- ngpwngp
- ngpw wfilengp
- ngpw wfile ngp
+ ngpw wfile ;ngp
+ ngpw wfile; ngp
+ ngpw wfile\n ngp

{}


{ {{}}{{{}}}{{}} }
{ {{}}{{{}}}{{}} }}
{{ {{}}{{{}}}{{}} }
{{
}}

0,
0,0,
0,0,0
0,0
0,+
,
,0
,,
,;
0;
0;0;
0;0;0
0;0,0
0;$;0
0;0
;;
;,

a

a\ text
a\ text

b

b

c

c\ text

d



D



g



G



h



H



i

i\ text

I



n



N



p



P



q



r



s//



s//n



s//g



s//p



s//w wfile



t

t
t label

w

w wfile

x

x
,x

y//



:



=



<empty>



#



*/

/*

Tests:
- args
- script sequence atributes
- command


// https://ru.wikibooks.org/wiki/Sed:_%D1%80%D1%83%D0%BA%D0%BE%D0%B2%D0%BE%D0%B4%D1%81%D1%82%D0%B2%D0%BE
// https://pubs.opengroup.org/onlinepubs/9699919799/utilities/ed.html#

p
$p
1 p
3p
2,8 p
3,$p
1 p ; p
p;p;p
\n    p\n p\n    p
\np\np\np
1 { p ; p }
1,3 { p ; p }
1,3 { p ; p } ; 1,2 { p ; p } ; {p ; p}
2,4 !p
2,4 !{p}

7,5,9 p
7,9 p
7; p
7;5; p
7;5;9; p
7;5,9 p
7;$;4 p
7;9 p
; p
;7 p
;; p
/pattern/- p

1,+3p
/5/,+3p
7, p
7,5, p
7,+ p
, p
,7 p
,, p
,; p
;, p
7;+ p
+++ p
-2 p
3 ---- 2
1 2 3 p

:begin ; b begin
:x ; /=$/ { N ; s/=\n//g ; bx }
/1/b else ; s/a/z/ ; :else ; y/123/456/

:begin ; n ; bbegin
:begin ; N ; bbegin

G

/1/!s/a/z/ ; y/123/456/
/start/,/end/p
/start/,$p
1,/end/p
/string [[:digit:]]* /p
/./,/^$/p
\,.*, p
\:[ac]: p
1,\,stop, p
\:start:,\,stop, p
\`'"$PATTERN"'`p
\n1,$ {\n/begin/,/end/ {\ns/#.* //\n\ns/[[:blank:]]*$//\n/^$/ d\np\n}\n}
/./{H;$!d} ; x ; s/^/\nSTART-->/ ; s/$/\n<--END/

s/a/A/p
s/a/A/g
s/b/B/g
s/c/C/g
s/a/A/2047
s/param=.* /param=new_value/
s/\([[:alnum:]]*\).* /\1/
s/[[:alnum:]]* //2
$ s/[[:alnum:]]* //2
s/WORD/Hello World/p ; p
s/.* /[&]/
s/SUBST/program\/lib\/module\/lib.so/
s|SUBST|program/lib/module/lib.so|
s_SUBST_program/lib/module/lib.so_
s/#.* //;s/[[:blank:]]*$//;/^$/ d;p
s/#.* //;s/[[:blank:]]*$//;/^$/ d;p       //   /etc/ssh/sshd_config
s/\(^[*][[:space:]]\)/   \1/
s/\(^[*][[:space:]]\)/   \1/;/List of products:/G
s/\(^[*][[:space:]]\)/   \1/;/List of products:/a ---------------
s/h\.0\.\(.*\)/ \U\1/

y:ABCDEFGHIJKLMNOPQRSTUVWXYZ:abcdefghijklmnopqrstuvwxyz:

// https://gist.github.com/chunyan/b426e4b696ff3e7b9afb

/^$/d;G
G;G
n;d
/regex/{x;p;x;}
/regex/G
/regex/{x;p;x;G;}
N;s/\n/\t/
N; s/^/     /; s/ *\(.\{6,\}\)\n/\1  /
/./N; s/\n/ /
$=
s/.$//
s/^M$//
s/\x0D$//
s/$/`echo -e \\\r`/
s/$'"/`echo \\\r`/
s/$/`echo \\\r`/
s/$/\r/
s/$//
s/\r//
s/[ \t]*$//
s/^[ \t]* //;s/[ \t]*$//
s/^/     /
:a;s/^.\{1,78\}$/ &/;ta
s/foo/bar/
s/foo/bar/4
s/foo/bar/g
s/\(.*\)foo\(.*foo\)/\1bar\2/
s/\(.*\)foo/\1bar/
/baz/s/foo/bar/g
/baz/!s/foo/bar/g
s/scarlet/red/g;s/ruby/red/g;s/puce/red/g
s/scarlet\|ruby\|puce/red/g
1!G;h;$!d
1!G;h;$p
/\n/!G;s/\(.\)\(.*\n\)/&\2\1/;//D;s/.//
$!N;s/\n/ /
:a;/\\$/N; s/\\\n//; ta
:a;$!N;s/\n=/ /;ta P;D
:a;s/\B[0-9]\{3\}\>/,&/;ta
:a;s/(^|[^0-9.])([0-9]+)([0-9]{3})/\1\2,\3/g;ta
n;n;n;n;G;
10q
q
:a;$q;N;11,$D;ba
$!N;$!D
$!d
$p
$!{h;d;}x
1{$q;};$!{h;d;};x
1{$d;};$!{h;d;};x
/regexp/p
/regexp/!d
/regexp/!p
/regexp/d
/regexp/{g;1!p;};h
/regexp/{n;p;}
/regexp/{=;x;1!p;g;$!N;p;D;};h
/AAA/!d; /BBB/!d; /CCC/!d
/AAA.*BBB.*CCC/!d
/AAA\|BBB\|CCC/!d
/./{H;$!d;};x;/AAA/!d;
/./{H;$!d;};x;/AAA\|BBB\|CCC/b;d
/^.\{65\}/p
/^.\{65\}/!p
/regexp/,$p
8,12p
8,12!d
52p
52!d
52q;d
3,${p;n;n;n;n;n;n;}
/Iowa/,/Montana/p
/Iowa/,/Montana/d
$!N; /^\(.*\)\n\1$/!P; D
$!N; s/^\(.*\)\n\1$/\1/; t; D
1,10d
$d
N;$!P;$!D;$d
:a;$d;N;2,10ba;P;D
n;n;n;n;n;n;n;d;
/pattern/d
/^$/d
/./!d
/./,/^$/!d
/^$/N;/\n$/D
/^$/N;/\n$/N;//D
/./,$!d
:a;/^\n*$/{$d;N;ba;}
/^$/{p;h;};/./{x;/./p;}
s/.`echo \\\b`//g
s/.^H//g
s/.\x08//g
/^$/q
1,/^$/d
/^Subject: * /!d; s///;q
/^Reply-To:/q; /^From:/h; /./d;g;q
s/ *(.*)//; s/>.* //; s/.*[:<] * //
s/^/> /
s/^> //
:a;s/<[^>]*>//g;/</N;//ba
/./{H;d;};x;s/\n/={NL}=/g
1s/={NL}=//;s/={NL}=/\n/g
s/^\(.*\)\.TXT/pkzip -mo \1 \1.TXT/
51q;45,50p

// https://habr.com/ru/companies/ruvds/articles/667490/

N;s/\n/\t/
N; s/^/ /; s/ *\(.\{4,\}\)\n/\1 /
/./=
/./N; s/\n/ /
3,5d
2,$d
/easy/,+2d
/^#/d;/^$/d
n,$p
/everyone/,5p
/learn/,+2p
s/old_pattern/new_pattern/i
5!s/life/love/
/is/ s/live/love/

*/

/*

Complete list of command features for check:
1) delimiters handling
2) args handling
3) all use cases in/out
4) ?

script "pattern space before" "pattern space after" "hold space before" "hold space after"

// base (args, input files etc)

correct:


wrong: 


// delimiter

correct:
;;;;
;\n;\n;;
;\\;\\;;

wrong: 
gh
g h
g; h \n gh \n g h ; gh \\

// {}

correct:
{}
{ }
{ \n \n \n }
{ { \n } {} {\n} { } }
{ { { { { { { { {} } } } } } } } }

wrong: 
{
}
{ { { { { { {} } } } } } } } }
{ { { { { { { { {} } } } } } }

// a

correct:
a\\text
a\\   text\\in\\sed
a\ text text ; text 

wrong:
a\
a\\ 
a  \text
a\text
a\text\in\sed
a\ text text \n text 

// b

correct:
b
b label
b; :label
b label; :label
:label; b label - infinite loop
b label1
b lab2el1abc
b loop_
b _start
b my_label

wrong: 
b #%$?@&*;
b label#
b 1label
b 1234
b g
b; label
b :label
b label :label
:label b label

// c

correct:
c\\text
c\\   text\\in\\sed
c\ text text ; text
c\r "a\nb\nc" "\n\nr"
0 c\r "a\nb\nc" "r\nb\nc"
0,2 c\r "a\nb\nc\nd" "\n\nr\nd"

wrong: 
c\
c\\ 
c  \text
c\text
c\text\in\sed
c\ text text \n text 

// d

correct:
d "abc\ndfg" ""
d "abcdfg" "" 
d; d - useless

wrong: 
d b
d d
dd

// D

correct:
D "abc\ndfg" "dfg" 
D "abcdfg" ""

wrong: 
D b
D D
DD

// g

correct:
0 h; 1 g "abc\n123" "abc\nabc" "" "abc" 

wrong: 
g g

// G

correct:
0 h; 1 G "abc\n123" "abc\n123\nabc" "" "abc" 

wrong: 
G G

// h

correct:
0 h; 1 h "abc\n123" "abc\n123" "" "123\n" 

wrong: 
h g
h h
hh

// H

correct:
0 H; 1 H "abc\n123" "abc\n123" "" "abc\n123\n" 

wrong: 
H g
H H
HH

// i

correct:


wrong: 


// I

correct:


wrong: 
I g
I I
II

// n

correct:


wrong: 
n g
n n
nn

// N

correct:


wrong: 
N g
N N
NN

// p

correct:


wrong: 
p g
p p
pp

// P

correct:


wrong: 
P g
P P
PP

// q

correct:
q
q; q - useless 

wrong: 
q g
q q
qq

// r

correct:


wrong: 


// s

correct:


wrong: 


// s n

correct:


wrong: 


// s g

correct:


wrong: 


// s p

correct:


wrong: 


// s w

correct:


wrong: 


// s n g

correct:


wrong: 


// s n p

correct:


wrong: 


// s n w

correct:


wrong: 


// s g p

correct:


wrong: 


// s g w

correct:


wrong: 


// s p w

correct:


wrong: 


// t

correct:


wrong: 


// w

correct:


wrong: 


// x

correct:
h; s/.* /abc/; x "abc" "abc" "" "abc" 

wrong: 
x h
x x
xx

// y

correct:


wrong: 


// :

correct:
:label1
:loop
:_start
:my_label

wrong: 
:1label
:1234 

// =

correct:
=

wrong: 
= g
= =
==

// #

correct:
{ #\\ }
{ #\n }
\n#h "abc" "abc" "" "abc" 

wrong: 
{ # }
{ \\# }
{ \n# }
a\text#abc\ntext
a\#text\ntext

// address

correct:


wrong: 


// address BRE

correct:


wrong: 


// s BRE

correct:


wrong: 


// special chararacters

correct:


wrong: 


// combinations

correct:


wrong: 


// other

correct:


wrong: 


*/