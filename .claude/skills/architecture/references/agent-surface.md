# Agent surface

Agents operate mergegate entirely through `mg`. Three layers, all shipped inside the binary:

1. **Self-describing CLI.** Short `--help`, errors that name the next command, `needs_decision` with options.
2. **Skill embedded in the binary.** `mg skill install` writes the copy matching that binary, so the skill and the CLI never drift. A test asserts that every command the skill mentions exists.
3. **`mg resume`** prints a self-contained prompt (item context plus unaddressed feedback) for agents without skill support.

No MCP server. The CLI works in every harness, while MCP tool schemas cost context in every session.

## CLI contract

- **Never prompt.** Every input is a flag.
- **Missing decisions.** A command that can't proceed without one exits 2 and prints `{"needs_decision": {"question": …, "options": […], "default": …}}`. The agent asks the operator and re-runs with the chosen flag.
- **Every setting has a command to change it later.** Changes that affect open items say so first, through `needs_decision`.
- **Output.** `--json` on every command; compact by default, because every byte costs tokens.
- **Stable exit codes.** `mg wait`: 0 events returned, 75 timeout with no events, 3 item closed.
- **`mg doctor`** verifies the whole setup and prints fixes.
