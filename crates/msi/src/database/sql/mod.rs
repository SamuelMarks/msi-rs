//! Native pure-Rust Windows Installer SQL dialect engine for MSI packages.
//!
//! Grounded directly in the official Windows Installer SDK SQL syntax specification:
//! - Keywords: `SELECT`, `FROM`, `WHERE`, `ORDER BY`, `ASC`, `DESC`, `INSERT`, `INTO`, `VALUES`, `UPDATE`, `SET`, `DELETE`, `CREATE`, `TABLE`, `ALTER`, `ADD`, `DROP`, `HOLD`, `FREE`, `DISTINCT`, `IS`, `NULL`, `NOT`, `AND`, `OR`, `LIKE`.
//! - Query execution against [`LinkedDatabase`].

pub mod ast;
pub mod executor;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use executor::*;
pub use lexer::*;
pub use parser::*;
