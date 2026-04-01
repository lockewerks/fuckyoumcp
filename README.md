# fuckyoumcp

The Windows 11 system MCP server that every other MCP server wishes it was.

**90 tools. 18 categories. 33 direct Win32 syscalls. Sub-millisecond response times.** Built in Rust because we're not here to fuck around with Node.js startup times and PowerShell's "please wait while I load the entire .NET runtime to tell you what your CPU is called."

## What the hell is this?

An [MCP (Model Context Protocol)](https://modelcontextprotocol.io) server that gives AI assistants **full system control** over Windows 11. Not "click this button" UI automation bullshit — actual system control. Processes, services, registry, firewall, network, the whole goddamn operating system.

Other Windows MCP servers use PowerShell for everything and make you wait 1-2 seconds per tool call. We call Win32 APIs directly from Rust. Our `process_list` runs in **9ms**. Our `memory_info` runs in **<1ms**. Their equivalent takes **1,500ms**. Do the math.

## Architecture (or: Why This Is Fast)

```
┌─────────────────────────────────────────────────────────┐
│  fuckyoumcp.exe (4.3MB Rust binary)                     │
│                                                         │
│  33 tools ──→ Direct Win32 syscalls ──→ <1ms response   │
│               CreateToolhelp32Snapshot, OpenSCManagerW,  │
│               RegOpenKeyExW, GetTcpTable2, etc.          │
│                                                         │
│  57 tools ──→ Persistent PowerShell pool ──→ 200-1500ms │
│               3x pre-warmed pwsh.exe processes           │
│               (for COM-only APIs that Win32 can't touch) │
└─────────────────────────────────────────────────────────┘
```

**Native Win32 tools (33):** Process management, services, registry, filesystem, network connections, system info, clipboard, disk info — all via direct syscalls. No subprocess. No serialization overhead. Just raw speed.

**PowerShell pool tools (57):** Firewall rules, scheduled tasks, event logs, user management, Windows features, audio, updates — stuck behind COM/WMI interfaces that only PowerShell can reach without losing your mind. The pool keeps 3 `pwsh.exe` processes warm so at least you're not paying startup cost.

## The 90 Tools

| Category | Count | Backend | Tools |
|----------|-------|---------|-------|
| **System Info** | 7 | Native + PS | `system_info` `cpu_info` `memory_info` `disk_info` `gpu_info` `battery_info` `network_adapters` |
| **Process** | 5 | Native | `process_list` `process_detail` `process_kill` `process_start` `process_tree` |
| **Service** | 6 | Native | `service_list` `service_detail` `service_start` `service_stop` `service_restart` `service_set_startup` |
| **Filesystem** | 8 | Native + PS | `fs_list` `fs_search` `fs_info` `fs_permissions` `fs_streams` `fs_drives` `fs_share_list` `fs_share_create` |
| **Registry** | 6 | Native + PS | `registry_read` `registry_write` `registry_delete` `registry_list` `registry_search` `registry_export` |
| **Network** | 8 | Native + PS | `network_connections` `network_config` `network_ping` `network_dns_lookup` `network_trace_route` `network_port_test` `network_wifi` `network_bandwidth` |
| **Firewall** | 5 | PS | `firewall_rules_list` `firewall_rule_create` `firewall_rule_delete` `firewall_rule_toggle` `firewall_status` |
| **Event Log** | 4 | PS | `eventlog_query` `eventlog_sources` `eventlog_stats` `eventlog_clear` |
| **Scheduled Tasks** | 6 | PS | `task_list` `task_detail` `task_create` `task_delete` `task_run` `task_toggle` |
| **Software** | 3 | PS | `software_list` `software_detail` `software_uninstall` |
| **Users & Groups** | 9 | PS | `user_list` `user_detail` `user_create` `user_delete` `user_modify` `group_list` `group_members` `group_add_member` `group_remove_member` |
| **Environment** | 7 | PS | `env_list` `env_get` `env_set` `env_delete` `path_list` `path_add` `path_remove` |
| **PowerShell/CMD/WMI** | 3 | PS | `powershell_execute` `cmd_execute` `wmi_query` |
| **Windows Features** | 3 | PS | `feature_list` `feature_enable` `feature_disable` |
| **Clipboard** | 2 | Native | `clipboard_get` `clipboard_set` |
| **Display & Audio** | 3 | PS | `display_info` `audio_devices` `audio_volume` |
| **Performance** | 3 | Native + PS | `perf_snapshot` `perf_top` `perf_counter` |
| **Windows Update** | 2 | PS | `update_list` `update_history` |

## Installation

### Prerequisites

- **Windows 11** (or 10, but why are you still on 10?)
- **PowerShell 7+** (`winget install Microsoft.PowerShell`)
- **Rust** (`winget install Rustlang.Rustup`) — only needed to build from source

### Build from source

```bash
git clone https://github.com/lockewerks/fuckyoumcp.git
cd fuckyoumcp
cargo build --release
```

Your binary is at `target/release/fuckyoumcp.exe` (4.3MB, stripped, LTO'd).

### Add to your MCP client

Most MCP clients use a JSON config file. Add fuckyoumcp as a server:

**Settings file** (e.g. `settings.json`, `mcp_config.json`, etc.):

```json
{
  "mcpServers": {
    "fuckyoumcp": {
      "command": "C:\\path\\to\\fuckyoumcp.exe"
    }
  }
}
```

### Desktop app config

For desktop MCP apps, the config is usually at `%APPDATA%\<app>\config.json`:

```json
{
  "mcpServers": {
    "fuckyoumcp": {
      "command": "C:\\path\\to\\fuckyoumcp.exe"
    }
  }
}
```

## Monitoring

Every tool call is logged to `%TEMP%\fuckyoumcp.log` with timestamps, tool names, execution times, and error details.

```powershell
# Watch it live
Get-Content -Path "$env:TEMP\fuckyoumcp.log" -Wait -Tail 20
```

```bash
# Or from Git Bash / WSL
tail -f $TEMP/fuckyoumcp.log
```

Sample output:
```
0.532ms  INFO ▶ native tool="process_list"
0.541ms  INFO ✓ native done tool="process_list" ms=9 bytes=1277
1.200ms  INFO ▶ call tool="eventlog_query"
2.450ms  INFO ✓ done tool="eventlog_query" ms=1250 bytes=8432
```

## Configuration

| Env Variable | Default | Description |
|-------------|---------|-------------|
| `FYMCP_POOL_SIZE` | `3` | Number of persistent PowerShell workers |
| `RUST_LOG` | `info` | Log level (`debug`, `info`, `warn`, `error`) |

## Performance

Measured on AMD Ryzen AI 9 HX 370, Windows 11 Pro:

| Tool | Backend | Latency |
|------|---------|---------|
| `memory_info` | Native Win32 | **<1ms** |
| `system_info` | Native Win32 | **<1ms** |
| `disk_info` | Native Win32 | **<1ms** |
| `service_list` | Native Win32 | **4ms** |
| `process_list` | Native Win32 | **9ms** |
| `cpu_info` | PowerShell | ~1,100ms |
| `eventlog_query` | PowerShell | ~1,250ms |
| `firewall_rules_list` | PowerShell | ~1,500ms |

Native tools are **100-1000x faster** than PowerShell-backed tools. The 33 native tools cover the most commonly used operations. The 57 PowerShell tools handle the COM/WMI-only operations that would require 10x the code to implement natively.

## Why "fuckyoumcp"?

Because we looked at the existing Windows MCP landscape and said exactly that. Every other server was either:

- **UI automation** (cool, but we want system control, not screen clicking)
- **PowerShell wrappers** that spawn a new `pwsh.exe` for every. single. command.
- **TypeScript** servers adding 200ms of Node.js startup to every interaction

So we wrote it in Rust with direct Win32 syscalls because we have standards and those standards include sub-millisecond response times.

## License

[MIT](LICENSE) — do whatever you want, just don't blame us.

## Contributing

PRs welcome. If you want to migrate one of the 57 PowerShell tools to native Win32, you are a hero and we will buy you a beer. If you want to add a new tool, go for it — just put it in the right category in `server.rs` and it'll get picked up by the tool router automatically.
