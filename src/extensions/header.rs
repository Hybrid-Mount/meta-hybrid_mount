use crate::module_id::ModuleId;

pub(super) enum Header {
    Global,
    Module(ModuleId),
}

fn declaration<'a>(mut text: &'a str, key: &str) -> mlua::Result<(&'a str, &'a str)> {
    loop {
        text = text.trim_start();
        let Some(comment) = text.strip_prefix("--") else {
            break;
        };
        if let Some(long) = comment.strip_prefix('[') {
            let equals = long.len() - long.trim_start_matches('=').len();
            if let Some(body) = long[equals..].strip_prefix('[') {
                let end = format!("]{}]", "=".repeat(equals));
                text = body
                    .split_once(&end)
                    .ok_or_else(|| mlua::Error::runtime("unclosed Lua header comment"))?
                    .1;
                continue;
            }
        }
        text = comment.split_once('\n').map_or("", |(_, rest)| rest);
    }
    let (line, rest) = text.split_once('\n').unwrap_or((text, ""));
    let (_, literal) = line
        .split_once('=')
        .filter(|(name, _)| name.trim() == key)
        .ok_or_else(|| mlua::Error::runtime(format!("expected literal {key} declaration")))?;
    Ok((literal.trim(), rest))
}

pub(super) fn parse(text: &str) -> mlua::Result<(Header, String)> {
    let (version, rest) = declaration(text, "API")?;
    let (header, body) = match version {
        "1" => (Header::Global, rest),
        "2" => {
            let (literal, body) = declaration(rest, "MODULE")?;
            let id = ['\'', '"']
                .into_iter()
                .find_map(|quote| literal.strip_prefix(quote)?.strip_suffix(quote))
                .ok_or_else(|| mlua::Error::runtime("MODULE must be a quoted module ID"))?;
            (
                Header::Module(ModuleId::try_from(id).map_err(mlua::Error::external)?),
                body,
            )
        }
        _ => return Err(mlua::Error::runtime(format!("unsupported API {version:?}"))),
    };
    let lines = text[..text.len() - body.len()]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count();
    Ok((header, "\n".repeat(lines) + body))
}
