//Tâches : expand_vars, command_substitution, try_builtin, et tous les builtin_ (cd, pwd, export, etc.).

pub fn expand_vars(s: &str, ctx: &mut ExecContext) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'$' {
            i += 1;
            if i >= bytes.len() { result.push('$'); break; }
            if bytes[i] == b'(' {
                i += 1; let start = i; let mut depth = 1usize;
                while i < bytes.len() {
                    match bytes[i] {
                        b'(' => { depth += 1; i += 1; }
                        b')' => { depth -= 1; if depth == 0 { break; } i += 1; }
                        _ => { i += 1; }
                    }
                }
                let cmd_str = &s[start..i];
                if i < bytes.len() { i += 1; }
                let output = command_substitution(cmd_str, ctx);
                result.push_str(&output);
            } else if bytes[i] == b'{' {
                i += 1; let start = i;
                while i < bytes.len() && bytes[i] != b'}' { i += 1; }
                let var = &s[start..i];
                if i < bytes.len() { i += 1; }
                let val = ctx.env.get(var).map(|s| s.as_str()).unwrap_or("");
                result.push_str(val);
            } else if bytes[i] == b'?' {
                result.push_str(&ctx.last_exit.to_string());
                i += 1;
            } else if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1; }
                let var = &s[start..i];
                let val = ctx.env.get(var).map(|s| s.as_str()).unwrap_or("");
                result.push_str(val);
            } else { result.push('$'); }
        } else { result.push(bytes[i] as char); i += 1; }
    }
    result
}

pub fn command_substitution(cmd_str: &str, ctx: &mut ExecContext) -> String {
    use std::process::{Command, Stdio};
    let tokens = match crate::lexer::tokenize(cmd_str) { Ok(t) => t, Err(_) => return String::new() };
    let list = match crate::parser::parse(&tokens) { Ok(l) => l, Err(_) => return String::new() };
    if list.items.is_empty() { return String::new(); }
    let pipeline = &list.items[0].pipeline;
    if pipeline.commands.is_empty() { return String::new(); }
    let first_cmd = &pipeline.commands[0];
    if first_cmd.argv.is_empty() { return String::new(); }
    let name = &first_cmd.argv[0];
    let path = match resolve_path(name, ctx) { Some(p) => p, None => return String::new() };
    let output = Command::new(&path).args(&first_cmd.argv[1..]).envs(ctx.env.iter()).stdout(Stdio::piped()).stderr(Stdio::null()).output();
    match output {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout).to_string();
            s.trim_end_matches('\n').trim_end_matches('\r').to_string()
        }
        Err(_) => String::new(),
    }
}

fn try_builtin(argv: &[String], ctx: &mut ExecContext) -> Option<i32> {
    match argv.first().map(|s| s.as_str()) {
        Some("cd")     => Some(builtin_cd(argv, ctx)),
        Some("pwd")    => Some(builtin_pwd()),
        Some("exit")   => Some(builtin_exit(argv, ctx)),
        Some("export") => Some(builtin_export(argv, ctx)),
        Some("unset")  => Some(builtin_unset(argv, ctx)),
        Some("echo")   => Some(builtin_echo(argv)),
        Some("true")   => Some(0),
        Some("false")  => Some(1),
        Some("type")   => Some(builtin_type(argv, ctx)),
        Some("hash")   => Some(0),
        _ => None,
    }
}

fn builtin_cd(argv: &[String], ctx: &mut ExecContext) -> i32 {
    let target = match argv.get(1) { Some(d) => d.clone(), None => ctx.env.get("HOME").cloned().unwrap_or_else(|| "/".to_string()) };
    match std::env::set_current_dir(&target) {
        Ok(_) => {
            if let Ok(cwd) = std::env::current_dir() {
                let s = cwd.to_string_lossy().to_string();
                ctx.env.insert("OLDPWD".into(), ctx.env.get("PWD").cloned().unwrap_or_default());
                ctx.env.insert("PWD".into(), s);
            }
            0
        }
        Err(e) => { eprintln!("\x1b[91mrustshell: cd: {}: {}\x1b[0m", target, e); 1 }
    }
}

fn builtin_pwd() -> i32 {
    match std::env::current_dir() {
        Ok(p) => { println!("{}", p.display()); 0 }
        Err(e) => { eprintln!("\x1b[91mrustshell: pwd: {}\x1b[0m", e); 1 }
    }
}

fn builtin_exit(argv: &[String], ctx: &ExecContext) -> i32 {
    let code = argv.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(ctx.last_exit);
    std::process::exit(code);
}

fn builtin_export(argv: &[String], ctx: &mut ExecContext) -> i32 {
    if argv.len() == 1 {
        let mut vars: Vec<_> = ctx.env.iter().collect();
        vars.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in vars { println!("declare -x {}=\"{}\"", k, v); }
        return 0;
    }
    for arg in &argv[1..] {
        if let Some(eq) = arg.find('=') {
            let key = arg[..eq].to_string();
            let val = arg[eq+1..].to_string();
            ctx.env.insert(key.clone(), val.clone());
            std::env::set_var(&key, &val);
        } else if let Ok(val) = std::env::var(arg) { ctx.env.insert(arg.clone(), val); }
    }
    0
}

fn builtin_unset(argv: &[String], ctx: &mut ExecContext) -> i32 {
    for arg in &argv[1..] { ctx.env.remove(arg); std::env::remove_var(arg); }
    0
}

fn builtin_echo(argv: &[String]) -> i32 {
    let (newline, start) = if argv.get(1).map(|s| s.as_str()) == Some("-n") { (false, 2) } else { (true, 1) };
    let out = argv[start..].join(" ");
    if newline { println!("{}", out); } else { print!("{}", out); }
    0
}

fn builtin_type(argv: &[String], ctx: &ExecContext) -> i32 {
    let mut exit = 0;
    let builtins = ["cd","pwd","exit","export","unset","echo","true","false","type","history","jobs","fg","bg","clear","help"];
    for name in &argv[1..] {
        if builtins.contains(&name.as_str()) { println!("{} est un builtin du shell", name); }
        else if let Some(path) = resolve_path(name, ctx) { println!("{} est {}", name, path); }
        else { eprintln!("\x1b[91mrustshell: type: {}: introuvable\x1b[0m", name); exit = 1; }
    }
    exit
}