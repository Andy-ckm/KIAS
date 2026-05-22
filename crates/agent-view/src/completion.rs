//! Shell completion generation — like `kubectl completion bash`.

use std::fmt;

/// Shell types for completion generation.
#[derive(Debug, Clone, Copy)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

impl fmt::Display for Shell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bash => write!(f, "bash"),
            Self::Zsh => write!(f, "zsh"),
            Self::Fish => write!(f, "fish"),
            Self::PowerShell => write!(f, "powershell"),
        }
    }
}

/// Generate shell completion script.
pub fn generate_completion(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => BASH_COMPLETION,
        Shell::Zsh => ZSH_COMPLETION,
        Shell::Fish => FISH_COMPLETION,
        Shell::PowerShell => POWERSHELL_COMPLETION,
    }
}

/// Print installation instructions.
pub fn print_install_hint(shell: Shell) {
    match shell {
        Shell::Bash => {
            eprintln!("# Bash completion installation:");
            eprintln!("# Option 1: Source directly");
            eprintln!("source <(kias-agent-view completion bash)");
            eprintln!();
            eprintln!("# Option 2: Install to system");
            eprintln!("kias-agent-view completion bash > /etc/bash_completion.d/kias-agent-view");
        }
        Shell::Zsh => {
            eprintln!("# Zsh completion installation:");
            eprintln!("# Add to ~/.zshrc:");
            eprintln!("source <(kias-agent-view completion zsh)");
            eprintln!();
            eprintln!("# Or install to fpath:");
            eprintln!("kias-agent-view completion zsh > ${{fpath[1]}}/_kias-agent-view");
        }
        Shell::Fish => {
            eprintln!("# Fish completion installation:");
            eprintln!(
                "kias-agent-view completion fish > ~/.config/fish/completions/kias-agent-view.fish"
            );
        }
        Shell::PowerShell => {
            eprintln!("# PowerShell completion installation:");
            eprintln!("# Add to $PROFILE:");
            eprintln!("kias-agent-view completion powershell | Out-String | Invoke-Expression");
        }
    }
}

const BASH_COMPLETION: &str = r#"#!/bin/bash
# Bash completion for kias-agent-view
# Generated automatically — do not edit

_kias_agent_view() {
    local cur prev commands
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    commands="status logs top get describe cluster completion"

    if [[ ${COMP_CWORD} -eq 1 ]]; then
        COMPREPLY=( $(compgen -W "${commands}" -- "${cur}") )
        return 0
    fi

    case "${prev}" in
        status)
            COMPREPLY=( $(compgen -W "--wide -w --output -o --help -h" -- "${cur}") )
            return 0
            ;;
        logs)
            COMPREPLY=( $(compgen -W "--follow -f --tail -n --level -l --component -c --help -h" -- "${cur}") )
            return 0
            ;;
        top)
            COMPREPLY=( $(compgen -W "--interval -i --sort -s --help -h" -- "${cur}") )
            return 0
            ;;
        get)
            COMPREPLY=( $(compgen -W "agents sessions tasks nodes workflows --show-labels -l --watch -w --output -o" -- "${cur}") )
            return 0
            ;;
        describe)
            COMPREPLY=( $(compgen -W "agents sessions tasks nodes workflows" -- "${cur}") )
            return 0
            ;;
        cluster)
            COMPREPLY=( $(compgen -W "--nodes -n --help -h" -- "${cur}") )
            return 0
            ;;
        completion)
            COMPREPLY=( $(compgen -W "bash zsh fish powershell" -- "${cur}") )
            return 0
            ;;
        --output|-o)
            COMPREPLY=( $(compgen -W "table json yaml quiet" -- "${cur}") )
            return 0
            ;;
        --sort|-s)
            COMPREPLY=( $(compgen -W "cpu memory tokens" -- "${cur}") )
            return 0
            ;;
        --level|-l)
            COMPREPLY=( $(compgen -W "trace debug info warn error" -- "${cur}") )
            return 0
            ;;
    esac

    return 0
}

complete -F _kias_agent_view kias-agent-view
"#;

const ZSH_COMPLETION: &str = r#"#compdef kias-agent-view
# Zsh completion for kias-agent-view
# Generated automatically — do not edit

_kias_agent_view() {
    local -a commands
    commands=(
        'status:Show agent status overview'
        'logs:Display or stream agent logs'
        'top:Resource monitoring'
        'get:Get/list resources'
        'describe:Describe a resource in detail'
        'cluster:Show cluster/node overview'
        'completion:Generate shell completions'
    )

    _arguments -C \
        '1:command:->commands' \
        '*::arg:->args'

    case $state in
        commands)
            _describe 'command' commands
            ;;
        args)
            case $words[1] in
                status)
                    _arguments \
                        '(-w --wide)'{-w,--wide}'[Show wide output]' \
                        '(-o --output)'{-o,--output}'[Output format]:format:(table json yaml quiet)'
                    ;;
                logs)
                    _arguments \
                        '(-f --follow)'{-f,--follow}'[Follow log output]' \
                        '(-n --tail)'{-n,--tail}'[Number of lines]:lines' \
                        '(-l --level)'{-l,--level}'[Log level]:level:(trace debug info warn error)' \
                        '(-c --component)'{-c,--component}'[Filter by component]:component'
                    ;;
                top)
                    _arguments \
                        '(-i --interval)'{-i,--interval}'[Refresh interval]:seconds' \
                        '(-s --sort)'{-s,--sort}'[Sort by column]:column:(cpu memory tokens)'
                    ;;
                get)
                    _arguments \
                        '1:resource:(agents sessions tasks nodes workflows)' \
                        '(-l --show-labels)'{-l,--show-labels}'[Show labels]' \
                        '(-w --watch)'{-w,--watch}'[Watch for changes]' \
                        '(-o --output)'{-o,--output}'[Output format]:format:(table json yaml quiet)'
                    ;;
                describe)
                    _arguments \
                        '1:resource:(agents sessions tasks nodes workflows)' \
                        '2:id'
                    ;;
                completion)
                    _arguments '1:shell:(bash zsh fish powershell)'
                    ;;
                cluster)
                    _arguments \
                        '(-n --nodes)'{-n,--nodes}'[Show node details]'
                    ;;
            esac
            ;;
    esac
}

_kias_agent_view "$@"
"#;

const FISH_COMPLETION: &str = r#"# Fish completion for kias-agent-view
# Generated automatically — do not edit

# Subcommands
complete -c kias-agent-view -n '__fish_use_subcommand' -a 'status' -d 'Show agent status overview'
complete -c kias-agent-view -n '__fish_use_subcommand' -a 'logs' -d 'Display or stream agent logs'
complete -c kias-agent-view -n '__fish_use_subcommand' -a 'top' -d 'Resource monitoring'
complete -c kias-agent-view -n '__fish_use_subcommand' -a 'get' -d 'Get/list resources'
complete -c kias-agent-view -n '__fish_use_subcommand' -a 'describe' -d 'Describe a resource'
complete -c kias-agent-view -n '__fish_use_subcommand' -a 'cluster' -d 'Show cluster overview'
complete -c kias-agent-view -n '__fish_use_subcommand' -a 'completion' -d 'Generate shell completions'

# Global options
complete -c kias-agent-view -s o -l output -d 'Output format' -xa 'table json yaml quiet'
complete -c kias-agent-view -s s -l server -d 'API server URL'

# status
complete -c kias-agent-view -n '__fish_seen_subcommand_from status' -s w -l wide -d 'Wide output'

# logs
complete -c kias-agent-view -n '__fish_seen_subcommand_from logs' -s f -l follow -d 'Follow logs'
complete -c kias-agent-view -n '__fish_seen_subcommand_from logs' -s n -l tail -d 'Number of lines'
complete -c kias-agent-view -n '__fish_seen_subcommand_from logs' -s l -l level -d 'Log level' -xa 'trace debug info warn error'
complete -c kias-agent-view -n '__fish_seen_subcommand_from logs' -s c -l component -d 'Filter by component'

# top
complete -c kias-agent-view -n '__fish_seen_subcommand_from top' -s i -l interval -d 'Refresh interval'
complete -c kias-agent-view -n '__fish_seen_subcommand_from top' -s s -l sort -d 'Sort by' -xa 'cpu memory tokens'

# get
complete -c kias-agent-view -n '__fish_seen_subcommand_from get' -xa 'agents sessions tasks nodes workflows'
complete -c kias-agent-view -n '__fish_seen_subcommand_from get' -s l -l show-labels -d 'Show labels'
complete -c kias-agent-view -n '__fish_seen_subcommand_from get' -s w -l watch -d 'Watch for changes'

# completion
complete -c kias-agent-view -n '__fish_seen_subcommand_from completion' -xa 'bash zsh fish powershell'
"#;

const POWERSHELL_COMPLETION: &str = r#"# PowerShell completion for kias-agent-view
# Generated automatically — do not edit

using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'kias-agent-view' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'kias-agent-view'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-')) {
                break
            }
            $element.Value
        }
    ) -join ';'

    $completions = @(switch ($command) {
        'kias-agent-view' {
            [CompletionResult]::new('status', 'status', [CompletionResultType]::ParameterValue, 'Show agent status overview')
            [CompletionResult]::new('logs', 'logs', [CompletionResultType]::ParameterValue, 'Display or stream agent logs')
            [CompletionResult]::new('top', 'top', [CompletionResultType]::ParameterValue, 'Resource monitoring')
            [CompletionResult]::new('get', 'get', [CompletionResultType]::ParameterValue, 'Get/list resources')
            [CompletionResult]::new('describe', 'describe', [CompletionResultType]::ParameterValue, 'Describe a resource')
            [CompletionResult]::new('cluster', 'cluster', [CompletionResultType]::ParameterValue, 'Show cluster overview')
            [CompletionResult]::new('completion', 'completion', [CompletionResultType]::ParameterValue, 'Generate completions')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
"#;

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_completion_not_empty() {
        let comp = generate_completion(Shell::Bash);
        assert!(!comp.is_empty());
        assert!(comp.contains("_kias_agent_view"));
        assert!(comp.contains("complete -F"));
    }

    #[test]
    fn test_zsh_completion_not_empty() {
        let comp = generate_completion(Shell::Zsh);
        assert!(!comp.is_empty());
        assert!(comp.contains("#compdef kias-agent-view"));
    }

    #[test]
    fn test_fish_completion_not_empty() {
        let comp = generate_completion(Shell::Fish);
        assert!(!comp.is_empty());
        assert!(comp.contains("complete -c kias-agent-view"));
    }

    #[test]
    fn test_powershell_completion_not_empty() {
        let comp = generate_completion(Shell::PowerShell);
        assert!(!comp.is_empty());
        assert!(comp.contains("Register-ArgumentCompleter"));
    }

    #[test]
    fn test_all_shells_covered() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell] {
            let comp = generate_completion(shell);
            assert!(comp.len() > 100, "{shell} completion too short");
        }
    }

    #[test]
    fn test_shell_display() {
        assert_eq!(Shell::Bash.to_string(), "bash");
        assert_eq!(Shell::Zsh.to_string(), "zsh");
        assert_eq!(Shell::Fish.to_string(), "fish");
        assert_eq!(Shell::PowerShell.to_string(), "powershell");
    }

    #[test]
    fn test_bash_completion_has_subcommands() {
        let comp = generate_completion(Shell::Bash);
        for cmd in &[
            "status",
            "logs",
            "top",
            "get",
            "describe",
            "cluster",
            "completion",
        ] {
            assert!(comp.contains(cmd), "Bash completion missing command: {cmd}");
        }
    }

    #[test]
    fn test_install_hint_doesnt_panic() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell] {
            print_install_hint(shell);
        }
    }
}
