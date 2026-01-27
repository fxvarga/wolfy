//! Lexer for rasi-like theme files using logos

use crate::theme::types::Color;
use logos::Logos;

/// Token type for the theme lexer
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\n\r]+")] // Skip whitespace
#[logos(skip r"//[^\n]*")] // Skip line comments
#[logos(skip r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/")] // Skip block comments
pub enum Token {
    // Structural tokens
    #[token("{")]
    BraceOpen,

    #[token("}")]
    BraceClose,

    #[token(":")]
    Colon,

    #[token(";")]
    Semicolon,

    #[token(",")]
    Comma,

    #[token("(")]
    ParenOpen,

    #[token(")")]
    ParenClose,

    #[token("*")]
    Star,

    #[token(".")]
    Dot,

    #[token("[")]
    BracketOpen,

    #[token("]")]
    BracketClose,

    // Keywords
    #[token("rgb")]
    Rgb,

    #[token("rgba")]
    Rgba,

    #[token("url")]
    Url,

    #[token("true")]
    True,

    #[token("false")]
    False,

    #[token("inherit")]
    Inherit,

    #[token("horizontal")]
    Horizontal,

    #[token("vertical")]
    Vertical,

    // Units
    #[token("px")]
    UnitPx,

    #[token("em")]
    UnitEm,

    #[token("%")]
    UnitPercent,

    #[token("mm")]
    UnitMm,

    // Hex colors - parsed directly to Color
    #[regex(r"#[0-9a-fA-F]{3}", |lex| parse_hex_color(lex.slice()))]
    #[regex(r"#[0-9a-fA-F]{4}", |lex| parse_hex_color(lex.slice()))]
    #[regex(r"#[0-9a-fA-F]{6}", |lex| parse_hex_color(lex.slice()))]
    #[regex(r"#[0-9a-fA-F]{8}", |lex| parse_hex_color(lex.slice()))]
    HexColor(Color),

    // Numbers (integer or float)
    #[regex(r"-?[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    Float(f64),

    #[regex(r"-?[0-9]+", |lex| lex.slice().parse::<i64>().ok(), priority = 2)]
    Integer(i64),

    // Quoted strings
    #[regex(r#""[^"]*""#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    String(String),

    // Identifiers (property names, widget names, etc.)
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_-]*", |lex| lex.slice().to_string())]
    Ident(String),
}

fn parse_hex_color(s: &str) -> Option<Color> {
    Color::from_hex(s).ok()
}

/// Wrapper for lexer with position tracking
pub struct Lexer<'a> {
    inner: logos::Lexer<'a, Token>,
    source: &'a str,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            inner: Token::lexer(source),
            source,
        }
    }

    pub fn source(&self) -> &'a str {
        self.source
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<(usize, Token, usize), LexerError>;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.inner.next()?;
        let span = self.inner.span();

        match token {
            Ok(tok) => Some(Ok((span.start, tok, span.end))),
            Err(_) => Some(Err(LexerError {
                span: span.clone(),
                slice: self.source[span].to_string(),
            })),
        }
    }
}

#[derive(Debug)]
pub struct LexerError {
    pub span: std::ops::Range<usize>,
    pub slice: String,
}

impl std::fmt::Display for LexerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Unexpected token '{}' at position {}",
            self.slice, self.span.start
        )
    }
}

impl std::error::Error for LexerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let source = "textbox { color: #ff0000; }";
        let tokens: Vec<_> = Lexer::new(source).map(|r| r.map(|(_, t, _)| t)).collect();

        assert!(tokens[0].as_ref().unwrap() == &Token::Ident("textbox".to_string()));
        assert!(tokens[1].as_ref().unwrap() == &Token::BraceOpen);
        assert!(tokens[2].as_ref().unwrap() == &Token::Ident("color".to_string()));
    }

    #[test]
    fn test_hex_colors() {
        let source = "#fff #ff00ff #12345678";
        let tokens: Vec<_> = Lexer::new(source)
            .filter_map(|r| r.ok())
            .map(|(_, t, _)| t)
            .collect();

        assert!(matches!(tokens[0], Token::HexColor(_)));
        assert!(matches!(tokens[1], Token::HexColor(_)));
        assert!(matches!(tokens[2], Token::HexColor(_)));
    }

    #[test]
    fn test_numbers_and_units() {
        let source = "12px 1.5em 50%";
        let tokens: Vec<_> = Lexer::new(source)
            .filter_map(|r| r.ok())
            .map(|(_, t, _)| t)
            .collect();

        assert!(matches!(tokens[0], Token::Integer(12)));
        assert!(matches!(tokens[1], Token::UnitPx));
        assert!(matches!(tokens[2], Token::Float(f) if (f - 1.5).abs() < 0.001));
        assert!(matches!(tokens[3], Token::UnitEm));
        assert!(matches!(tokens[4], Token::Integer(50)));
        assert!(matches!(tokens[5], Token::UnitPercent));
    }

    #[test]
    fn test_comments() {
        let source = r#"
            // line comment
            color: red; /* block comment */
            background: blue;
        "#;
        let tokens: Vec<_> = Lexer::new(source)
            .filter_map(|r| r.ok())
            .map(|(_, t, _)| t)
            .collect();

        // Comments should be skipped
        assert!(tokens
            .iter()
            .all(|t| !matches!(t, Token::Ident(s) if s.contains("comment"))));
    }

    #[test]
    fn test_strings() {
        let source = r#"font: "Segoe UI";"#;
        let tokens: Vec<_> = Lexer::new(source)
            .filter_map(|r| r.ok())
            .map(|(_, t, _)| t)
            .collect();

        assert!(matches!(&tokens[2], Token::String(s) if s == "Segoe UI"));
    }
}
