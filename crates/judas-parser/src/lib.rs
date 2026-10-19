pub mod error;
pub mod lexer;
pub mod parser;
pub mod token;

pub use error::ParseError;
pub use lexer::Lexer;
pub use parser::Parser;
pub use token::{Token, TokenKind};

use judas_ast::{ModuleDecl, SourceFile};

/// Parses the source file and returns the AST.
///
/// # Errors
///
/// Returns an error if the source file cannot be parsed.
pub fn parse_source(input: &str) -> Result<SourceFile, ParseError> {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse_source_file()
}

/// Parses the module declaration and returns the AST.
///
/// # Errors
///
/// Returns an error if the module declaration cannot be parsed.
pub fn parse_module(input: &str) -> Result<ModuleDecl, ParseError> {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse_module_decl()
}
