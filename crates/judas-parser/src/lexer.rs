use crate::error::ParseError;
use crate::token::{Token, TokenKind};
use judas_ast::Span;

pub struct Lexer<'a> {
    input: &'a str,
    chars: Vec<(usize, char)>,
    cursor: usize,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    /// Initializes a new lexer with the given input string.
    ///
    /// # Arguments
    ///
    /// * `input` - The input string to tokenize.
    pub fn new(input: &'a str) -> Self {
        let chars: Vec<(usize, char)> = input.char_indices().collect();
        Self {
            input,
            chars,
            cursor: 0,
            line: 1,
            column: 1,
        }
    }

    /// Returns the current position in the input string.
    ///
    /// # Returns
    ///
    /// A tuple `(offset, line, column)` representing the current position.
    fn current_pos(&self) -> (usize, usize, usize) {
        if self.cursor < self.chars.len() {
            (self.chars[self.cursor].0, self.line, self.column)
        } else {
            (self.input.len(), self.line, self.column)
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.chars.get(self.cursor).map(|(_, c)| *c)
    }

    fn peek_next_char(&self) -> Option<char> {
        self.chars.get(self.cursor + 1).map(|(_, c)| *c)
    }

    /// Advances the cursor to the next character in the input string.
    /// If the cursor is already at the end, returns `None`.
    /// If the cursor is at the end of a line, the line and column
    /// are updated to reflect the end of the line.
    fn advance(&mut self) -> Option<char> {
        if let Some((_, c)) = self.chars.get(self.cursor) {
            let ch = *c;
            self.cursor += 1;
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            Some(ch)
        } else {
            None
        }
    }

    /// Skips over whitespace and comments in the input string.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the end of the input string is reached, `Err` otherwise.
    fn skip_whitespace_and_comments(&mut self) -> Result<(), ParseError> {
        loop {
            match self.peek_char() {
                // Skips over whitespace characters.
                Some(' ' | '\t' | '\r' | '\n') => {
                    self.advance();
                }

                // Skips over single-line comments.
                Some('/') if self.peek_next_char() == Some('/') => {
                    // Single-line comment
                    self.advance();
                    self.advance();
                    while let Some(c) = self.peek_char() {
                        if c == '\n' {
                            self.advance();
                            break;
                        }
                        self.advance();
                    }
                }

                // Skips over multi-line comments.
                Some('/') if self.peek_next_char() == Some('*') => {
                    // Multi-line comment
                    let (start_byte, start_line, start_col) = self.current_pos();
                    self.advance();
                    self.advance();
                    let mut closed = false;
                    while let Some(c) = self.advance() {
                        if c == '*' && self.peek_char() == Some('/') {
                            self.advance();
                            closed = true;
                            break;
                        }
                    }
                    if !closed {
                        return Err(ParseError::new(
                            "Unterminated block comment",
                            Span::new(start_byte, self.input.len(), start_line, start_col),
                        ));
                    }
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// Tokenizes the input string into a vector of tokens.
    ///
    /// # Returns
    ///
    /// `Ok(tokens)` if the input string is successfully
    /// tokenized, `Err` otherwise.
    pub fn tokenize(&mut self) -> Result<Vec<Token>, ParseError> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace_and_comments()?;

            let (start_byte, start_line, start_col) = self.current_pos();
            let ch = match self.peek_char() {
                Some(c) => c,
                None => {
                    tokens.push(Token::new(
                        TokenKind::Eof,
                        Span::new(start_byte, start_byte, start_line, start_col),
                    ));
                    break;
                }
            };

            // Two-character operators or single
            let kind = match ch {
                // Mostly parses operators and punctuation
                '(' => {
                    self.advance();
                    TokenKind::LParen
                }
                ')' => {
                    self.advance();
                    TokenKind::RParen
                }
                '{' => {
                    self.advance();
                    TokenKind::LBrace
                }
                '}' => {
                    self.advance();
                    TokenKind::RBrace
                }
                '[' => {
                    self.advance();
                    TokenKind::LBracket
                }
                ']' => {
                    self.advance();
                    TokenKind::RBracket
                }
                ',' => {
                    self.advance();
                    TokenKind::Comma
                }
                ':' => {
                    self.advance();
                    TokenKind::Colon
                }
                ';' => {
                    self.advance();
                    TokenKind::Semicolon
                }
                '.' => {
                    self.advance();
                    TokenKind::Dot
                }
                '~' => {
                    self.advance();
                    TokenKind::Tilde
                }
                '+' => {
                    self.advance();
                    TokenKind::Plus
                }
                '-' => {
                    self.advance();
                    TokenKind::Minus
                }
                '*' => {
                    self.advance();
                    TokenKind::Star
                }
                '/' => {
                    self.advance();
                    TokenKind::Slash
                }
                '%' => {
                    self.advance();
                    TokenKind::Percent
                }
                '^' => {
                    self.advance();
                    TokenKind::Caret
                }
                '=' => {
                    self.advance();
                    if self.peek_char() == Some('>') {
                        self.advance();
                        TokenKind::FatArrow
                    } else if self.peek_char() == Some('=') {
                        self.advance();
                        TokenKind::EqEq
                    } else {
                        TokenKind::Eq
                    }
                }
                '!' => {
                    self.advance();
                    if self.peek_char() == Some('=') {
                        self.advance();
                        TokenKind::NotEq
                    } else {
                        TokenKind::Bang
                    }
                }
                '<' => {
                    self.advance();
                    if self.peek_char() == Some('=') {
                        self.advance();
                        TokenKind::LtEq
                    } else if self.peek_char() == Some('<') {
                        self.advance();
                        TokenKind::Shl
                    } else {
                        TokenKind::Lt
                    }
                }
                '>' => {
                    self.advance();
                    if self.peek_char() == Some('=') {
                        self.advance();
                        TokenKind::GtEq
                    } else if self.peek_char() == Some('>') {
                        self.advance();
                        TokenKind::Shr
                    } else {
                        TokenKind::Gt
                    }
                }
                '&' => {
                    self.advance();
                    if self.peek_char() == Some('&') {
                        self.advance();
                        TokenKind::AmpAmp
                    } else {
                        TokenKind::Amp
                    }
                }
                '|' => {
                    self.advance();
                    if self.peek_char() == Some('|') {
                        self.advance();
                        TokenKind::PipePipe
                    } else {
                        TokenKind::Pipe
                    }
                }

                // Parses string literals.
                //
                // ```
                // "hello"
                // ```
                '"' => {
                    self.advance();
                    let mut s = String::new();
                    let mut closed = false;
                    while let Some(c) = self.advance() {
                        if c == '"' {
                            closed = true;
                            break;
                        } else if c == '\\' {
                            match self.advance() {
                                Some('n') => s.push('\n'),
                                Some('r') => s.push('\r'),
                                Some('t') => s.push('\t'),
                                Some('\\') => s.push('\\'),
                                Some('"') => s.push('"'),
                                Some(other) => s.push(other),
                                None => break,
                            }
                        } else {
                            s.push(c);
                        }
                    }
                    if !closed {
                        let (end_byte, _, _) = self.current_pos();
                        return Err(ParseError::new(
                            "Unterminated string literal",
                            Span::new(start_byte, end_byte, start_line, start_col),
                        ));
                    }
                    TokenKind::StringLiteral(s)
                }

                // Parses numeric literals.
                //
                // ```
                // 42
                // ```
                '0'..='9' => {
                    // Check for 0x (hex) or 0b (binary)
                    if ch == '0'
                        && (self.peek_next_char() == Some('x')
                            || self.peek_next_char() == Some('X'))
                    {
                        self.advance(); // '0'
                        self.advance(); // 'x'
                        let mut num_str = String::new();
                        while let Some(c) = self.peek_char() {
                            if c.is_ascii_hexdigit() || c == '_' {
                                if c != '_' {
                                    num_str.push(c);
                                }
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        let val = i128::from_str_radix(&num_str, 16).map_err(|e| {
                            let (end_byte, _, _) = self.current_pos();
                            ParseError::new(
                                format!("Invalid hexadecimal literal: {}", e),
                                Span::new(start_byte, end_byte, start_line, start_col),
                            )
                        })?;
                        TokenKind::IntLiteral(val)
                    } else if ch == '0'
                        && (self.peek_next_char() == Some('b')
                            || self.peek_next_char() == Some('B'))
                    {
                        self.advance(); // '0'
                        self.advance(); // 'b'
                        let mut num_str = String::new();
                        while let Some(c) = self.peek_char() {
                            if c == '0' || c == '1' || c == '_' {
                                if c != '_' {
                                    num_str.push(c);
                                }
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        let val = i128::from_str_radix(&num_str, 2).map_err(|e| {
                            let (end_byte, _, _) = self.current_pos();
                            ParseError::new(
                                format!("Invalid binary literal: {}", e),
                                Span::new(start_byte, end_byte, start_line, start_col),
                            )
                        })?;
                        TokenKind::IntLiteral(val)
                    } else {
                        let mut num_str = String::new();
                        while let Some(c) = self.peek_char() {
                            if c.is_ascii_digit() || c == '_' {
                                if c != '_' {
                                    num_str.push(c);
                                }
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        let val = num_str.parse::<i128>().map_err(|e| {
                            let (end_byte, _, _) = self.current_pos();
                            ParseError::new(
                                format!("Invalid integer literal: {}", e),
                                Span::new(start_byte, end_byte, start_line, start_col),
                            )
                        })?;
                        TokenKind::IntLiteral(val)
                    }
                }

                // Parses identifiers (module, function, etc.).
                //
                // `module`, `on`, `if`, `else`, `struct`...
                c if c.is_alphabetic() || c == '_' => {
                    let mut ident = String::new();
                    while let Some(c) = self.peek_char() {
                        if c.is_alphanumeric() || c == '_' {
                            ident.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    match ident.as_str() {
                        "module" => TokenKind::Module,
                        "enum" => TokenKind::Enum,
                        "input" => TokenKind::Input,
                        "output" => TokenKind::Output,
                        "register" => TokenKind::Register,
                        "connect" => TokenKind::Connect,
                        "on" => TokenKind::On,
                        "rising" => TokenKind::Rising,
                        "falling" => TokenKind::Falling,
                        "if" => TokenKind::If,
                        "else" => TokenKind::Else,
                        "match" => TokenKind::Match,
                        "let" => TokenKind::Let,
                        "assert" => TokenKind::Assert,
                        "struct" => TokenKind::Struct,
                        "interface" => TokenKind::Interface,
                        "master" => TokenKind::Master,
                        "slave" => TokenKind::Slave,
                        "true" => TokenKind::BoolLiteral(true),
                        "false" => TokenKind::BoolLiteral(false),
                        _ => TokenKind::Ident(ident),
                    }
                }

                // If the character is not recognized, return an error.
                // Useful for rejecting unicode characters and
                // non widely supported characters.
                unknown => {
                    let (end_byte, _, _) = self.current_pos();
                    return Err(ParseError::new(
                        format!("Unexpected character: '{}'", unknown),
                        Span::new(start_byte, end_byte, start_line, start_col),
                    ));
                }
            };

            let (end_byte, _, _) = self.current_pos();
            tokens.push(Token::new(
                kind,
                Span::new(start_byte, end_byte, start_line, start_col),
            ));
        }

        Ok(tokens)
    }
}
