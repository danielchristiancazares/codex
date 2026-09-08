# GitHub Copilot authentication over SSH

The native Copilot provider follows `cli_auth_credentials_store` for login,
login status, logout, model discovery, and API authentication. This fixes the
fork's previous behavior, which always used the OS keyring for Copilot and could
fail with `User interaction is not allowed` in an SSH session even after the
macOS login keychain was unlocked.

For an installation operated entirely over SSH, use the default `file` mode,
or set it explicitly in `$CODEX_HOME/config.toml`:

```toml
cli_auth_credentials_store = "file"
```

Then run:

```sh
codex login --provider copilot
codex login status --provider copilot
```

Login prints a GitHub device-authorization URL and code. Open that URL on any
device and authorize the code; the SSH process completes login and saves the
credential. The host running Codex does not need a desktop session, a local
browser, an unlocked keychain, or a macOS application-access prompt.

File mode stores the GitHub token and its machine identity together in
`$CODEX_HOME/copilot-auth.json`, separately from OpenAI/ChatGPT's `auth.json`.
Writes replace the file atomically and use owner-only `0600` permissions on
Unix. This is a plaintext credential file with the same access model as Codex's
existing file authentication. Keep the Codex home protected as account data.

Existing keyring credentials are preserved. File mode never consults them, so
an installation that previously stored Copilot credentials only in the keyring
needs one device login to populate the new file. `--force` starts a new device
flow when an existing credential is malformed or its store cannot be read.

Other modes remain available:

- `keyring` retains the existing OS credential entry and fails if access is
  unavailable. It does not create a file fallback.
- `auto` uses an existing file fallback first; otherwise it loads the keyring.
  Saving falls back to a private file when the keyring fails. Once present, the
  file remains authoritative even if the keyring becomes accessible again.
  Forced login can recover from a keyring read failure. Logout reports an error
  and preserves the file if an older keyring credential cannot be removed.
- `ephemeral` keeps a successful login in the authentication object's shared
  process memory without persisting it. It cannot authenticate a later process.

The runtime provider uses the authentication manager's resolved Codex home and
storage policy. Endpoint refresh and rejected-credential state are shared by
providers belonging to that authentication manager, avoiding credential reuse
between unrelated runtime configurations. Recovery rereads the selected store
so a credential replaced by a separate login process can be used.
