//! Desktop Exec parsing produces argv directly. It never invokes a shell implicitly.

use anyhow::{Result, bail, ensure};

pub fn parse(
    value: &str,
    name: &str,
    icon: Option<&str>,
    desktop_path: &str,
) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut in_quote = false;
    let mut quoted = false;
    let mut started = false;
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                in_quote = !in_quote;
                quoted = true;
                started = true;
            }
            '\\' => {
                let next = chars
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("unfinished Exec escape"))?;
                if in_quote && !matches!(next, '"' | '\\' | '$' | '`') {
                    token.push('\\');
                }
                token.push(next);
                started = true;
            }
            ch if ch.is_ascii_whitespace() && !in_quote => {
                if started {
                    tokens.push((std::mem::take(&mut token), quoted));
                }
                started = false;
                quoted = false;
            }
            ch => {
                token.push(ch);
                started = true;
            }
        }
    }
    ensure!(!in_quote, "unmatched Exec quote");
    if started {
        tokens.push((token, quoted));
    }
    let mut argv = Vec::new();
    let mut file_code_seen = false;
    for (token, quoted) in tokens {
        if token == "%i" {
            ensure!(
                !quoted,
                "field codes inside quoted arguments are unsupported"
            );
            if let Some(icon) = icon {
                argv.extend(["--icon".into(), icon.into()]);
            }
            continue;
        }
        if matches!(token.as_str(), "%f" | "%F" | "%u" | "%U") {
            ensure!(
                !quoted,
                "field codes inside quoted arguments are unsupported"
            );
            ensure!(!file_code_seen, "multiple file/URI field codes");
            file_code_seen = true;
            // A launcher activation has no files or URIs to supply.
            continue;
        }
        let mut expanded = String::new();
        let mut chars = token.chars();
        while let Some(ch) = chars.next() {
            if ch != '%' {
                expanded.push(ch);
                continue;
            }
            let code = chars
                .next()
                .ok_or_else(|| anyhow::anyhow!("unfinished Exec field code"))?;
            if code == '%' {
                expanded.push('%');
                continue;
            }
            ensure!(
                !quoted,
                "field codes inside quoted arguments are unsupported"
            );
            match code {
                'c' => expanded.push_str(name),
                'k' => expanded.push_str(desktop_path),
                'f' | 'u' => {
                    ensure!(!file_code_seen, "multiple file/URI field codes");
                    file_code_seen = true;
                }
                'F' | 'U' | 'i' => bail!("Exec %{code} must be a separate argument"),
                _ => bail!("unsupported Exec field code %{code}"),
            }
        }
        argv.push(expanded);
    }
    ensure!(
        argv.first()
            .is_some_and(|program| !program.is_empty() && !program.contains('=')),
        "invalid Exec executable"
    );
    Ok(argv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_field_codes_and_literal_shell_text_remain_arguments() {
        let argv = parse(
            r#""/path with spaces/app" "" "%literal%%" %i %c %k %U "$(touch /tmp/never-executed)";"#,
            "Example App",
            Some("app-icon"),
            "/apps/example.desktop",
        );
        // Unknown field codes remain errors even when a quote would make expansion undefined.
        assert!(argv.is_err());
        let argv = parse(
            r#""/path with spaces/app" "" "100%%" %i %c %k %U "$(touch /tmp/never-executed)""#,
            "Example App",
            Some("app-icon"),
            "/apps/example.desktop",
        )
        .unwrap();
        assert_eq!(
            argv,
            [
                "/path with spaces/app",
                "",
                "100%",
                "--icon",
                "app-icon",
                "Example App",
                "/apps/example.desktop",
                "$(touch /tmp/never-executed)"
            ]
        );
    }

    #[test]
    fn malformed_and_ambiguous_expansions_are_rejected() {
        for exec in [
            "",
            "NAME=value app",
            "app \"unfinished",
            "app %z",
            "app %",
            "app --icon=%i",
            "app %U %f",
            "app \"%c\"",
        ] {
            assert!(parse(exec, "App", None, "/app.desktop").is_err(), "{exec}");
        }
    }
}
