/// Splits a line into commands separated by `;`, honouring double quotes and stripping
/// `//` comments.
pub fn split_commands(line: &str) -> Vec<&str> {
    let mut commands = Vec::new();
    let mut quoted = false;
    let mut start = 0;
    let bytes = line.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'"' => quoted = !quoted,
            b'/' if !quoted && bytes.get(i + 1) == Some(&b'/') => break,
            b';' if !quoted => {
                commands.push(&line[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    commands.push(&line[start..i]);
    commands.retain(|c| !c.trim().is_empty());
    commands
}

/// Splits a single command into tokens on whitespace; double quotes group a token.
pub fn tokenize(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut has_token = false;

    for ch in command.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                has_token = true;
            }
            c if c.is_whitespace() && !quoted => {
                if has_token {
                    tokens.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            c => {
                current.push(c);
                has_token = true;
            }
        }
    }

    if has_token {
        tokens.push(current);
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_commands_and_strips_comments() {
        assert_eq!(
            split_commands(r#"echo "a;b"; sv_gravity 0.1 // comment; ignored"#),
            vec![r#"echo "a;b""#, " sv_gravity 0.1 "]
        );
        assert!(split_commands("// only a comment").is_empty());
    }

    #[test]
    fn tokenizes_with_quotes() {
        assert_eq!(
            tokenize(r#"  bind  space "+jump"  "#),
            vec!["bind", "space", "+jump"]
        );
        assert_eq!(
            tokenize(r#"cl_player_name "Major Tom""#),
            vec!["cl_player_name", "Major Tom"]
        );
        assert_eq!(tokenize(r#"echo """#), vec!["echo", ""]);
    }
}
