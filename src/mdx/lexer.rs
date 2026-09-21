//! Lexer for the Excel MDX subset (plan 047).
//!
//! Tokenizes just enough MDX to parse the shapes Excel/MSOLAP sends and the
//! ones we explicitly support: bracketed identifiers, member keys, quoted
//! strings, numbers, punctuation and bare keywords/function names. Everything
//! else is a malformed token — the caller faults instead of guessing.

/// One lexical token.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// `[identifier]`
    Bracket(String),
    /// `&[key]` (also accepts the XML-escaped `&amp;[key]`).
    Key(String),
    /// A bare word: keyword or function name (`ON`, `FROM`, `HEAD`, …).
    Ident(String),
    /// A numeric literal (including a leading `-`).
    Number(String),
    /// A quoted string (`'…'` or `"…"`).
    Str(String),
    Dot,
    Comma,
    Colon,
    LParen,
    RParen,
    LBrace,
    RBrace,
    /// `!` — for VBA-style references (`VBA![Date]()`).
    Bang,
    Minus,
    Gt,
    Ge,
    Lt,
    Le,
    Eq,
    Ne,
}

impl Token {
    /// Case-insensitive keyword match.
    pub fn is_ident(&self, word: &str) -> bool {
        matches!(self, Token::Ident(i) if i.eq_ignore_ascii_case(word))
    }
}

/// Lexical errors. `Malformed` means the text is not valid MDX for this subset;
/// callers turn it into a fault.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    Malformed(String),
    Unsupported(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Malformed(m) => write!(f, "{m}"),
            ParseError::Unsupported(m) => write!(f, "{m}"),
        }
    }
}

/// Tokenize an MDX statement.
pub fn lex(input: &str) -> Result<Vec<Token>, ParseError> {
    let b = input.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        match c {
            b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            b'[' => {
                let start = i + 1;
                let mut j = start;
                while j < b.len() && b[j] != b']' {
                    j += 1;
                }
                if j >= b.len() {
                    return Err(ParseError::Malformed("unterminated [identifier]".into()));
                }
                out.push(Token::Bracket(input[start..j].to_string()));
                i = j + 1;
            }
            b'&' => {
                // `&[key]`, or XML-escaped `&amp;[key]`.
                let key_start = if b.get(i + 1) == Some(&b'[') {
                    i + 2
                } else if input[i..].starts_with("&amp;[") {
                    i + 6
                } else {
                    return Err(ParseError::Malformed("stray '&' in MDX".into()));
                };
                let mut j = key_start;
                while j < b.len() && b[j] != b']' {
                    j += 1;
                }
                if j >= b.len() {
                    return Err(ParseError::Malformed("unterminated &[key]".into()));
                }
                out.push(Token::Key(input[key_start..j].to_string()));
                i = j + 1;
            }
            b'\'' | b'"' => {
                let quote = c;
                let start = i + 1;
                let mut j = start;
                while j < b.len() && b[j] != quote {
                    j += 1;
                }
                if j >= b.len() {
                    return Err(ParseError::Malformed("unterminated string".into()));
                }
                out.push(Token::Str(input[start..j].to_string()));
                i = j + 1;
            }
            b'.' => {
                out.push(Token::Dot);
                i += 1;
            }
            b',' => {
                out.push(Token::Comma);
                i += 1;
            }
            b':' => {
                out.push(Token::Colon);
                i += 1;
            }
            b'(' => {
                out.push(Token::LParen);
                i += 1;
            }
            b')' => {
                out.push(Token::RParen);
                i += 1;
            }
            b'{' => {
                out.push(Token::LBrace);
                i += 1;
            }
            b'}' => {
                out.push(Token::RBrace);
                i += 1;
            }
            b'!' => {
                out.push(Token::Bang);
                i += 1;
            }
            b'-' if !b.get(i + 1).is_some_and(|n| n.is_ascii_digit()) => {
                out.push(Token::Minus);
                i += 1;
            }
            b'>' => {
                if b.get(i + 1) == Some(&b'=') {
                    out.push(Token::Ge);
                    i += 2;
                } else {
                    out.push(Token::Gt);
                    i += 1;
                }
            }
            b'<' => {
                if b.get(i + 1) == Some(&b'>') {
                    out.push(Token::Ne);
                    i += 2;
                } else if b.get(i + 1) == Some(&b'=') {
                    out.push(Token::Le);
                    i += 2;
                } else {
                    out.push(Token::Lt);
                    i += 1;
                }
            }
            b'=' => {
                out.push(Token::Eq);
                i += 1;
            }
            _ if c.is_ascii_digit()
                || (c == b'-' && b.get(i + 1).is_some_and(|n| n.is_ascii_digit())) =>
            {
                let start = i;
                let mut j = i + 1;
                while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.') {
                    j += 1;
                }
                out.push(Token::Number(input[start..j].to_string()));
                i = j;
            }
            _ if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                let mut j = i + 1;
                while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                    j += 1;
                }
                out.push(Token::Ident(input[start..j].to_string()));
                i = j;
            }
            _ => {
                return Err(ParseError::Malformed(format!(
                    "unexpected character '{}'",
                    c as char
                )));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_a_drilldown_member_statement() {
        let toks = lex(
            "SELECT NON EMPTY Hierarchize(DrilldownMember({{DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}}, {[Date].[Calendar].[Year].&[2022]},,,INCLUDE_CALC_MEMBERS)) ON COLUMNS FROM [Sales]",
        )
        .expect("lex");
        assert!(toks.iter().any(|t| t.is_ident("DrilldownMember")));
        assert!(toks.contains(&Token::Key("2022".into())));
        assert!(toks.contains(&Token::Bracket("Date".into())));
        assert!(toks.contains(&Token::LBrace));
        assert!(toks.contains(&Token::RParen));
    }

    #[test]
    fn lexes_xml_escaped_keys() {
        let toks = lex("[Date].[Calendar].[Year].&amp;[2024]").expect("lex");
        assert_eq!(toks.last(), Some(&Token::Key("2024".into())));
    }

    #[test]
    fn lexes_strings_and_numbers() {
        let toks = lex("HEAD({[D].[H].[L].&[a] : [D].[H].[L].&[b]}, 3)").expect("lex");
        assert!(toks.contains(&Token::Colon));
        assert!(toks.contains(&Token::Number("3".into())));
        let toks = lex("WITH SET [x] AS 'Filter([D].[H].[L].Members, 1)'").expect("lex");
        assert!(
            toks.iter()
                .any(|t| matches!(t, Token::Str(s) if s.starts_with("Filter(")))
        );
    }

    #[test]
    fn malformed_input_is_rejected() {
        assert!(matches!(lex("[Date"), Err(ParseError::Malformed(_))));
        assert!(matches!(lex("'oops"), Err(ParseError::Malformed(_))));
        assert!(matches!(lex("a & b"), Err(ParseError::Malformed(_))));
    }
}
