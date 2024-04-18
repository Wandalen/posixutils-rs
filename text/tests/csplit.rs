#[test]
fn test_escaped_end_pos() {
    // Test with no escape characters
    assert_eq!(escaped_end_pos("abc/def", '/'), Some(3));

    // Test with escape characters
    assert_eq!(escaped_end_pos("abc\\/def", '/'), Some(7));

    // Test with multiple escape characters
    assert_eq!(escaped_end_pos("abc\\\\/def", '/'), Some(8));

    // Test with no delimiter found
    assert_eq!(escaped_end_pos("abcdef", '/'), None);

    // Test with delimiter at the beginning of the string
    assert_eq!(escaped_end_pos("/abcdef", '/'), Some(0));

    // Test with empty string
    assert_eq!(escaped_end_pos("", '/'), None);
}

#[test]
fn test_incr_suffix() {
    // Test incrementing suffix with length 2
    let mut state = OutputState::new("prefix", 2);
    assert_eq!(state.suffix, "");
    assert_eq!(state.incr_suffix(), Ok(()));
    assert_eq!(state.suffix, "aa");
    assert_eq!(state.incr_suffix(), Ok(()));
    assert_eq!(state.suffix, "ab");
    assert_eq!(state.incr_suffix(), Ok(()));
    assert_eq!(state.suffix, "ac");
    assert_eq!(state.incr_suffix(), Ok(()));
    assert_eq!(state.suffix, "ad");

    // Test incrementing suffix with length 3
    let mut state = OutputState::new("prefix", 3);
    assert_eq!(state.suffix, "");
    assert_eq!(state.incr_suffix(), Ok(()));
    assert_eq!(state.suffix, "aaa");
    assert_eq!(state.incr_suffix(), Ok(()));
    assert_eq!(state.suffix, "aab");
    assert_eq!(state.incr_suffix(), Ok(()));
    assert_eq!(state.suffix, "aac");
    assert_eq!(state.incr_suffix(), Ok(()));
    assert_eq!(state.suffix, "aad");
    // Continue testing more increments as needed
}

#[test]
fn test_parse_op_rx() {
    // Test valid regular expression operand without offset
    let opstr = "/pattern/";
    let delim = '/';
    match parse_op_rx(opstr, delim) {
        Ok(Operand::Rx(regex, offset, is_skip)) => {
            assert_eq!(regex.as_str(), "pattern");
            assert_eq!(offset, 0);
            assert_eq!(is_skip, false);
        }
        _ => panic!("Expected Ok(Operand::Rx)"),
    }

    // Test valid regular expression operand with positive offset
    let opstr = "/pattern/+3";
    let delim = '/';
    match parse_op_rx(opstr, delim) {
        Ok(Operand::Rx(regex, offset, is_skip)) => {
            assert_eq!(regex.as_str(), "pattern");
            assert_eq!(offset, 3);
            assert_eq!(is_skip, false);
        }
        _ => panic!("Expected Ok(Operand::Rx)"),
    }

    // Test valid regular expression operand with negative offset
    let opstr = "/pattern/-2";
    let delim = '/';
    match parse_op_rx(opstr, delim) {
        Ok(Operand::Rx(regex, offset, is_skip)) => {
            assert_eq!(regex.as_str(), "pattern");
            assert_eq!(offset, -2);
            assert_eq!(is_skip, false);
        }
        _ => panic!("Expected Ok(Operand::Rx)"),
    }

    // Test valid regular expression operand with leading '+'
    let opstr = "/pattern/+5";
    let delim = '/';
    match parse_op_rx(opstr, delim) {
        Ok(Operand::Rx(regex, offset, is_skip)) => {
            assert_eq!(regex.as_str(), "pattern");
            assert_eq!(offset, 5);
            assert_eq!(is_skip, false);
        }
        _ => panic!("Expected Ok(Operand::Rx)"),
    }

    // Test valid regular expression operand with skip mode
    let opstr = "%pattern%";
    let delim = '%';
    match parse_op_rx(opstr, delim) {
        Ok(Operand::Rx(regex, offset, is_skip)) => {
            assert_eq!(regex.as_str(), "pattern");
            assert_eq!(offset, 0);
            assert_eq!(is_skip, true);
        }
        _ => panic!("Expected Ok(Operand::Rx)"),
    }

    // Test invalid regular expression operand
    let opstr = "/pattern";
    let delim = '/';
    match parse_op_rx(opstr, delim) {
        Err(e) => {
            assert_eq!(e.kind(), ErrorKind::Other);
            assert_eq!(e.to_string(), "invalid regex str");
        }
        _ => panic!("Expected Err"),
    }
}

#[test]
fn test_parse_op_repeat() {
    // Test valid repeating operand
    let opstr = "{5}";
    match parse_op_repeat(opstr) {
        Ok(Operand::Repeat(n)) => assert_eq!(n, 5),
        _ => panic!("Expected Ok(Operand::Repeat)"),
    }

    // Test invalid repeating operand (non-numeric)
    let opstr = "{abc}";
    match parse_op_repeat(opstr) {
        Err(e) => {
            assert_eq!(e.kind(), ErrorKind::Other);
            assert_eq!(e.to_string(), "invalid repeating operand");
        }
        _ => panic!("Expected Err"),
    }

    // Test invalid repeating operand (missing braces)
    let opstr = "5";
    match parse_op_repeat(opstr) {
        Err(e) => {
            assert_eq!(e.kind(), ErrorKind::Other);
            assert_eq!(e.to_string(), "invalid repeating operand");
        }
        _ => panic!("Expected Err"),
    }
}

#[test]
fn test_parse_op_linenum() {
    // Test valid line number operand
    let opstr = "10";
    match parse_op_linenum(opstr) {
        Ok(Operand::LineNum(n)) => assert_eq!(n, 10),
        _ => panic!("Expected Ok(Operand::LineNum)"),
    }

    // Test invalid line number operand (non-numeric)
    let opstr = "abc";
    match parse_op_linenum(opstr) {
        Err(e) => {
            assert_eq!(e.kind(), ErrorKind::Other);
            assert_eq!(e.to_string(), "invalid digit found in string");
        }
        _ => panic!("Expected Err"),
    }
}

#[test]
fn test_parse_operands() {
    // Test valid operands
    let args = Args {
        prefix: String::from("xx"),
        keep: false,
        num: 2,
        suppress: false,
        filename: String::from("test.txt"),
        operands: vec![
            String::from("/pattern/+1"),
            String::from("%skip/10"),
            String::from("15"),
            String::from("{3}"),
        ],
    };

    match parse_operands(&args) {
        Ok(ops) => {
            assert_eq!(ops.ops.len(), 4);
            match &ops.ops[0] {
                Operand::Rx(re, offset, _) => {
                    assert_eq!(re.as_str(), "pattern");
                    assert_eq!(*offset, 1);
                }
                _ => panic!("Expected Operand::Rx"),
            }
            match &ops.ops[1] {
                Operand::Rx(re, offset, _) => {
                    assert_eq!(re.as_str(), "skip");
                    assert_eq!(*offset, 10);
                }
                _ => panic!("Expected Operand::Rx"),
            }
            match &ops.ops[2] {
                Operand::LineNum(n) => assert_eq!(*n, 15),
                _ => panic!("Expected Operand::LineNum"),
            }
            match &ops.ops[3] {
                Operand::Repeat(n) => assert_eq!(*n, 3),
                _ => panic!("Expected Operand::Repeat"),
            }
        }
        _ => panic!("Expected Ok(SplitOps)"),
    }
}
