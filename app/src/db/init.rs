use clickhouse::Client;

pub async fn run_sql(client: &Client, sql: &str) -> anyhow::Result<()> {
    for stmt in split_sql_statements(sql) {
        let stmt = stmt.trim();
        if has_executable_sql(stmt) {
            client
                .query(stmt)
                .execute()
                .await
                .map_err(|err| anyhow::anyhow!("failed SQL statement:\n{}\n\n{}", stmt, err))?;
        }
    }
    Ok(())
}

fn split_sql_statements(sql: &str) -> Vec<String> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum State {
        Normal,
        SingleQuote,
        DoubleQuote,
        Backtick,
        LineComment,
        BlockComment,
    }

    let chars = sql.chars().collect::<Vec<_>>();
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut state = State::Normal;
    let mut index = 0usize;

    while index < chars.len() {
        let current_char = chars[index];
        let next_char = chars.get(index + 1).copied();

        match state {
            State::Normal => match (current_char, next_char) {
                ('-', Some('-')) => {
                    current.push(current_char);
                    current.push('-');
                    state = State::LineComment;
                    index += 1;
                }
                ('/', Some('*')) => {
                    current.push(current_char);
                    current.push('*');
                    state = State::BlockComment;
                    index += 1;
                }
                ('\'', _) => {
                    current.push(current_char);
                    state = State::SingleQuote;
                }
                ('"', _) => {
                    current.push(current_char);
                    state = State::DoubleQuote;
                }
                ('`', _) => {
                    current.push(current_char);
                    state = State::Backtick;
                }
                (';', _) => {
                    if has_executable_sql(current.trim()) {
                        statements.push(std::mem::take(&mut current));
                    } else {
                        current.clear();
                    }
                }
                _ => current.push(current_char),
            },
            State::SingleQuote => {
                current.push(current_char);
                if current_char == '\'' {
                    if next_char == Some('\'') {
                        current.push('\'');
                        index += 1;
                    } else if !is_backslash_escaped(&chars, index) {
                        state = State::Normal;
                    }
                }
            }
            State::DoubleQuote => {
                current.push(current_char);
                if current_char == '"' && !is_backslash_escaped(&chars, index) {
                    state = State::Normal;
                }
            }
            State::Backtick => {
                current.push(current_char);
                if current_char == '`' {
                    state = State::Normal;
                }
            }
            State::LineComment => {
                current.push(current_char);
                if current_char == '\n' {
                    state = State::Normal;
                }
            }
            State::BlockComment => {
                current.push(current_char);
                if current_char == '*' && next_char == Some('/') {
                    current.push('/');
                    index += 1;
                    state = State::Normal;
                }
            }
        }

        index += 1;
    }

    if has_executable_sql(current.trim()) {
        statements.push(current);
    }

    statements
}

fn is_backslash_escaped(chars: &[char], index: usize) -> bool {
    let mut backslashes = 0usize;
    let mut cursor = index;
    while cursor > 0 && chars[cursor - 1] == '\\' {
        backslashes += 1;
        cursor -= 1;
    }
    backslashes % 2 == 1
}

fn has_executable_sql(stmt: &str) -> bool {
    stmt.lines().any(|line| {
        let line = line.trim();

        !line.is_empty() && !line.starts_with("--")
    })
}

#[cfg(test)]
mod tests {
    use super::split_sql_statements;

    #[test]
    fn keeps_semicolons_inside_literals_and_comments() {
        let sql = r#"
            -- comment; still comment
            INSERT INTO events VALUES ('a;b');
            /* block; comment */
            SELECT "x;y";
        "#;
        let statements = split_sql_statements(sql);

        assert_eq!(statements.len(), 2);
        assert!(statements[0].contains("'a;b'"));
        assert!(statements[1].contains("\"x;y\""));
    }

    #[test]
    fn supports_escaped_and_doubled_quotes() {
        let sql = "SELECT 'it''s;a'; SELECT 'x\\';y';";
        let statements = split_sql_statements(sql);

        assert_eq!(statements.len(), 2);
    }
}
