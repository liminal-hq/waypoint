# @liminal-hq/plugin-secrets

Stores, tests for and deletes passwords, passphrases and tokens in the system keyring, so an app never writes one to its own files. On Linux it talks to the Secret Service on the session bus (GNOME Keyring, KWallet through its Secret Service bridge, KeePassXC) and, inside a Flatpak, to the Secret portal, both through [`oo7`](https://crates.io/crates/oo7). On Windows it uses Credential Manager.

The plugin knows nothing about the app around it. It reports what works through `getStatus()` with a typed reason when no keyring runs or it stays locked, so an interface can hide an option and say why.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-secrets = "0.1"

# Alternatively with Git:
tauri-plugin-secrets = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_secrets::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Secrets are filed under the app's identifier (`identifier` in `tauri.conf.json`), so two apps never see each other's.

### Permissions

Grant a window only what it needs. `secrets:default` is `allow-get-status`, `allow-store`, `allow-exists`, `allow-delete` and `allow-delete-account`. It leaves out `secrets:allow-fetch`: a webview that can read secrets back is a webview that a bug or a bad script can empty. Prefer to fetch a secret in Rust, where it never crosses the bridge, and grant `allow-fetch` only to a window that has to show or use one. A window that only needs to know whether the keyring works needs `secrets:allow-get-status`.

### JavaScript

```bash
pnpm add @liminal-hq/plugin-secrets
```

## Usage

### JavaScript

```typescript
import {
	exists,
	getStatus,
	hasFeature,
	isSecretsError,
	remove,
	store,
} from '@liminal-hq/plugin-secrets';

const status = await getStatus(); // never prompts to unlock the keyring
if (hasFeature(status, 'store')) {
	const id = { service: 'sftp', account: 'home-server', kind: 'password' } as const;
	try {
		await store(id, password, 'Home server'); // the label is what the keyring's manager shows
	} catch (error) {
		if (isSecretsError(error) && error.kind === 'locked') {
			show('The keyring stayed locked, so the password was not remembered.');
		}
	}
	await exists(id); // true, without reading it
	await remove(id); // true when there was one
}
```

### Rust

```rust
use tauri_plugin_secrets::{Secret, SecretId, SecretKind, SecretsExt};

let id = SecretId::new("volume", device_uuid, SecretKind::Passphrase);
app.secrets().store(&id, None, Secret::from_text(passphrase)).await?;
if let Some(secret) = app.secrets().fetch(&id).await? {
    unlock_with(secret.expose_text().unwrap_or_default());
}
```

`Secrets` is cheap to clone. An app wraps it in whatever trait its own crates define, and tests build one over `MemoryBackend`:

```rust
let secrets = tauri_plugin_secrets::Secrets::new(std::sync::Arc::new(tauri_plugin_secrets::MemoryBackend::new()));
```

## API

| Function                                                      | What it does                                                                                            |
| ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `getStatus()`                                                 | `{ available, reason, message, flavour, features }`; see below. Never prompts.                          |
| `store(id, secret, label?)`                                   | Stores a secret, replacing one with the same id. The value crosses the bridge once and is not returned. |
| `fetch(id)`                                                   | The secret, or `null`. Needs `secrets:allow-fetch`.                                                     |
| `exists(id)`                                                  | Whether a secret exists, without reading it.                                                            |
| `remove(id)`                                                  | Deletes a secret; true when there was one.                                                              |
| `removeAccount(service, account)`                             | Deletes every kind of secret of one account and returns how many there were.                            |
| `hasFeature(status, name)`, `featureReason`, `featureMessage` | Read the status.                                                                                        |

A `SecretId` is `{ service, account, kind }`: what the secret is for, whose it is and its `SecretKind` (`password`, `passphrase`, `token` or `key`). The kind is part of the identity, so one account can hold a password and the passphrase of its key. A name is not empty, is at most 1024 bytes and has no NUL character.

Errors are `SecretsError` objects with a `kind`: `noKeyring`, `locked` (it stayed locked), `dismissed` (the person closed the unlock prompt), `unsupported`, `invalid` (with `field`) and `failed` (with `message`).

## Never in a log

- `Secret` is a buffer zeroed when it is dropped, with a redacted `Debug`, no `Display` and no `Serialize`. `SecretId`'s `Debug` leaves the account out (it can be a user name).
- The plugin logs the service and kind of an action at `debug`, never an account or a value. Errors carry a library's description and never a value.
- Nothing is written to disk by the plugin. (In a Flatpak the Secret portal keeps its own encrypted file.)
- The value of `store` and `fetch` is a JavaScript string on the webview side, which cannot be zeroed. That is another reason to keep reading in Rust.
- Do not enable `trace` logging for `zbus`, which prints message bodies.

## Features and status

`getStatus()` returns `{ available, reason, message, flavour, features }`, where each feature is `{ name, available, reason, message }`. The features are `store`, `fetch` and `delete`; today they work or fail together. `flavour` is `secret-service`, `secret-portal`, `credential-manager` or `unsupported`.

| Reason                 | Meaning                                                                                              |
| ---------------------- | ---------------------------------------------------------------------------------------------------- |
| `no-keyring`           | No keyring is running: nothing owns `org.freedesktop.secrets`, there is no session bus or no portal. |
| `locked`               | The keyring is locked and an unlock was refused or dismissed.                                        |
| `failed`               | The keyring answered but failed.                                                                     |
| `unsupported-platform` | This operating system has no keyring support in the plugin.                                          |

A locked keyring counts as working: the desktop's own prompt unlocks it when a secret is stored or read. Only after an unlock is refused does the status say `locked`, until the next success.

## Platform notes

### Linux

- Items carry the attributes `application` (the app identifier), `service`, `account` and `kind`, so the keyring's own manager can find them and another program's items are never touched. Text secrets are stored as `text/plain`, others as bytes.
- Inside a Flatpak `oo7` asks the Secret portal for the key of an encrypted file and uses that; where the portal is missing it falls back to the Secret Service.
- If the Secret Service goes away the next call reconnects.

### Windows

- Each secret is a generic credential persisted for the local machine, named `namespace/service/account/kind` (the namespace is the app identifier, `/` and `*` in a name are escaped) with the account as its user name and the label as its comment, so Windows Credentials lists it under the app's identifier.
- Credential Manager keeps at most 2560 bytes per credential: a longer secret is refused as `invalid`, never cut.
- The vault of a signed-in user is never locked, so there is no unlock prompt and the status is always available.
- Type-checked with `cargo xwin`; not yet exercised on a real Windows machine. Run the ignored `live_windows` test there (`cargo test -p tauri-plugin-secrets --test live_windows -- --ignored --nocapture`).

### Other systems

Every feature is reported unavailable with `unsupported-platform`.

## Testing

`cargo nextest run -p tauri-plugin-secrets` runs headless: the typed errors and their mapping from `oo7`'s, the status, redaction, validation and the Tauri state over an in-memory keyring, and the failure when there is no session bus.

The live round trip against a real keyring is skipped with a message unless `SECRETS_LIVE_TEST=1`, so it never touches a person's own keyring by accident. Run it under a private session bus with a throwaway keyring (no desktop needed):

```bash
dbus-run-session -- bash -c 'echo "" | gnome-keyring-daemon --unlock --components=secrets >/dev/null; SECRETS_LIVE_TEST=1 cargo test -p tauri-plugin-secrets --test live -- --nocapture'
```

Use a throwaway `XDG_DATA_HOME` too, and stop the daemon afterwards.
