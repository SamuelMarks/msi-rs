//! Abstract Syntax Tree (AST) definitions for Windows Installer SQL dialect.

/// A binary comparison or matching operator in a SQL `WHERE` clause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// Equal (`=`)
    Equal,
    /// Not equal (`!=` or `<>`)
    NotEqual,
    /// Less than (`<`)
    LessThan,
    /// Greater than (`>`)
    GreaterThan,
    /// Less than or equal (`<=`)
    LessOrEqual,
    /// Greater than or equal (`>=`)
    GreaterOrEqual,
    /// Pattern matching (`LIKE`)
    Like,
}

/// A boolean expression inside a `WHERE` filter clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    /// Binary comparison: `<column> <op> <literal>`
    Comparison {
        /// Column name or expression.
        column: String,
        /// Comparison operator.
        op: BinaryOp,
        /// Literal value or positional parameter.
        value: SqlValue,
    },
    /// Null check: `<column> IS NULL` or `<column> IS NOT NULL`
    IsNull {
        /// Column name.
        column: String,
        /// True if negated (`IS NOT NULL`).
        negated: bool,
    },
    /// Logical AND of two expressions.
    And(Box<Self>, Box<Self>),
    /// Logical OR of two expressions.
    Or(Box<Self>, Box<Self>),
    /// Logical NOT of an expression.
    Not(Box<Self>),
}

/// A literal value or parameter placeholder in a SQL statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlValue {
    /// String literal `'text'`
    String(String),
    /// Integer literal `123`
    Integer(i32),
    /// Positional parameter placeholder `?`
    Parameter,
    /// Explicit NULL literal
    Null,
}

/// Order direction for sorting in `ORDER BY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrderDirection {
    /// Ascending sort (`ASC`).
    #[default]
    Ascending,
    /// Descending sort (`DESC`).
    Descending,
}

/// A column sorting term in `ORDER BY`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderByTerm {
    /// Column name to sort on.
    pub column: String,
    /// Direction of sort.
    pub direction: OrderDirection,
}

/// SQL column type tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlType {
    /// Variable or fixed character string.
    String,
    /// 16-bit short integer.
    Short,
    /// 32-bit long integer.
    Long,
    /// Binary stream object.
    Stream,
}

/// Column type definition in a `CREATE TABLE` or `ALTER TABLE` statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlColumnDef {
    /// Column name.
    pub name: String,
    /// Data type.
    pub data_type: SqlType,
    /// Character length for `CHAR` or `VARCHAR` types.
    pub length: u8,
    /// True if column is non-nullable (`NOT NULL`).
    pub not_null: bool,
    /// True if column is marked `PRIMARY KEY`.
    pub primary_key: bool,
    /// True if column contains localizable strings (`LOCALIZABLE`).
    pub localizable: bool,
}

/// Top-level parsed Windows Installer SQL statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// `SELECT [DISTINCT] <columns> FROM <table> [WHERE <expr>] [ORDER BY <order>]`
    Select {
        /// True if `DISTINCT` keyword specified.
        distinct: bool,
        /// List of projected column names or empty for wildcard `*`.
        columns: Vec<String>,
        /// Primary table name.
        table: String,
        /// Optional join tables (inner join).
        joins: Vec<String>,
        /// Optional filter expression.
        where_clause: Option<Expression>,
        /// Optional sorting terms.
        order_by: Vec<OrderByTerm>,
    },
    /// `INSERT INTO <table> [(<columns>)] VALUES (<values>)`
    Insert {
        /// Target table name.
        table: String,
        /// Optional explicit column names list.
        columns: Option<Vec<String>>,
        /// Values to insert.
        values: Vec<SqlValue>,
    },
    /// `UPDATE <table> SET <col> = <val> [, ...] [WHERE <expr>]`
    Update {
        /// Target table name.
        table: String,
        /// Assignments mapping column name to new value.
        assignments: Vec<(String, SqlValue)>,
        /// Optional filter expression.
        where_clause: Option<Expression>,
    },
    /// `DELETE FROM <table> [WHERE <expr>]`
    Delete {
        /// Target table name.
        table: String,
        /// Optional filter expression.
        where_clause: Option<Expression>,
    },
    /// `CREATE TABLE <table> (<column_defs>)`
    CreateTable {
        /// New table name.
        table: String,
        /// Column definitions.
        columns: Vec<SqlColumnDef>,
    },
    /// `ALTER TABLE <table> ADD <column_def> [HOLD]`
    AlterTable {
        /// Target table name.
        table: String,
        /// New column definition to add.
        column: SqlColumnDef,
        /// True if `HOLD` keyword specified.
        hold: bool,
    },
    /// `DROP TABLE <table>`
    DropTable {
        /// Table name to drop.
        table: String,
    },
}
