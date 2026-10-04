// The Windows keyring: Credential Manager, through `CredWriteW`, `CredReadW`, `CredDeleteW` and `CredEnumerateW`
//
// Each secret is a generic credential, persisted for the local machine only, named `namespace/service/account/kind` (so Windows Credentials lists it under the app's identifier) with the account as its user name. Credential Manager keeps at most 2560 bytes per credential; a longer secret is refused as invalid, never truncated. It is the current user's vault and is never locked in a signed-in session, so there is nothing to unlock and nothing to prompt for.
//
// Type-checked with `cargo xwin`; not exercised on a real Windows machine yet (the milestone's verification feature does that).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredEnumerateW, CredFree, CredReadW, CredWriteW, CREDENTIALW,
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
};

use crate::backend::{Backend, BoxFuture};
use crate::credman::{
    account_filter, check_limits, classify, target_name, win32_code, ERROR_NOT_FOUND,
};
use crate::error::{Result, SecretsError};
use crate::models::{Flavour, PluginStatus, Secret, SecretId};

pub struct Platform {
    namespace: String,
}

impl Platform {
    pub fn new(namespace: String) -> Self {
        Platform { namespace }
    }
}

/// A UTF-16 string with its terminating NUL, which the API wants.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn failure(error: &windows::core::Error) -> SecretsError {
    classify(win32_code(error.code().0))
}

fn is_not_found(error: &windows::core::Error) -> bool {
    win32_code(error.code().0) == ERROR_NOT_FOUND
}

/// Runs a blocking Win32 call off the async runtime's threads.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| SecretsError::failed("the credential task stopped"))?
}

fn write(target: &str, account: &str, label: Option<&str>, secret: &Secret) -> Result<()> {
    check_limits(target, secret.len())?;
    let mut target = wide(target);
    let mut account = wide(account);
    let mut comment = label.map(wide);
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target.as_mut_ptr()),
        Comment: comment
            .as_mut()
            .map(|comment| PWSTR(comment.as_mut_ptr()))
            .unwrap_or(PWSTR::null()),
        CredentialBlobSize: secret.len() as u32,
        // `CredWriteW` copies the blob and does not write to it.
        CredentialBlob: secret.expose_bytes().as_ptr() as *mut u8,
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: PWSTR(account.as_mut_ptr()),
        ..Default::default()
    };
    // SAFETY: every pointer in `credential` is to a buffer that lives to the end of this function.
    unsafe { CredWriteW(&credential, 0) }.map_err(|error| failure(&error))
}

fn read(target: &str) -> Result<Option<Secret>> {
    let target = wide(target);
    let mut found: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: `target` is NUL-terminated and `found` is a valid out pointer.
    match unsafe { CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None, &mut found) } {
        Ok(()) => {}
        Err(error) if is_not_found(&error) => return Ok(None),
        Err(error) => return Err(failure(&error)),
    }
    // SAFETY: on success `found` points to a credential that stays valid until `CredFree`; its blob is copied into a buffer that is zeroed on drop, and the system's copy is wiped before it is freed.
    unsafe {
        let credential = &*found;
        let size = credential.CredentialBlobSize as usize;
        let secret = if size == 0 || credential.CredentialBlob.is_null() {
            Secret::from_bytes(Vec::new())
        } else {
            Secret::from_bytes(std::slice::from_raw_parts(credential.CredentialBlob, size).to_vec())
        };
        if size > 0 && !credential.CredentialBlob.is_null() {
            std::ptr::write_bytes(credential.CredentialBlob, 0, size);
        }
        CredFree(found as *const _);
        Ok(Some(secret))
    }
}

fn delete(target: &str) -> Result<bool> {
    let target = wide(target);
    // SAFETY: `target` is NUL-terminated.
    match unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) } {
        Ok(()) => Ok(true),
        Err(error) if is_not_found(&error) => Ok(false),
        Err(error) => Err(failure(&error)),
    }
}

/// The target names that match a filter.
fn enumerate(filter: &str) -> Result<Vec<String>> {
    let filter = wide(filter);
    let mut count = 0u32;
    let mut list: *mut *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: `filter` is NUL-terminated; `count` and `list` are valid out pointers.
    match unsafe { CredEnumerateW(PCWSTR(filter.as_ptr()), None, &mut count, &mut list) } {
        Ok(()) => {}
        Err(error) if is_not_found(&error) => return Ok(Vec::new()),
        Err(error) => return Err(failure(&error)),
    }
    let mut names = Vec::with_capacity(count as usize);
    // SAFETY: on success `list` points to `count` valid credentials until `CredFree`; only the target names are read.
    unsafe {
        for index in 0..count as usize {
            let credential = &**list.add(index);
            if let Ok(name) = PCWSTR(credential.TargetName.0).to_string() {
                names.push(name);
            }
        }
        CredFree(list as *const _);
    }
    Ok(names)
}

impl Backend for Platform {
    fn status(&self) -> BoxFuture<'_, PluginStatus> {
        // The vault of a signed-in user is always there; there is nothing to probe without writing to it.
        Box::pin(async { PluginStatus::all_available(Flavour::CredentialManager) })
    }

    fn store(
        &self,
        id: SecretId,
        label: Option<String>,
        secret: Secret,
    ) -> BoxFuture<'_, Result<()>> {
        let target = target_name(&self.namespace, &id.service, &id.account, id.kind);
        Box::pin(async move {
            blocking(move || write(&target, &id.account, label.as_deref(), &secret)).await
        })
    }

    fn fetch(&self, id: SecretId) -> BoxFuture<'_, Result<Option<Secret>>> {
        let target = target_name(&self.namespace, &id.service, &id.account, id.kind);
        Box::pin(async move { blocking(move || read(&target)).await })
    }

    fn exists(&self, id: SecretId) -> BoxFuture<'_, Result<bool>> {
        let target = target_name(&self.namespace, &id.service, &id.account, id.kind);
        Box::pin(async move { blocking(move || read(&target).map(|found| found.is_some())).await })
    }

    fn delete(&self, id: SecretId) -> BoxFuture<'_, Result<bool>> {
        let target = target_name(&self.namespace, &id.service, &id.account, id.kind);
        Box::pin(async move { blocking(move || delete(&target)).await })
    }

    fn delete_account(&self, service: String, account: String) -> BoxFuture<'_, Result<usize>> {
        let filter = account_filter(&self.namespace, &service, &account);
        Box::pin(async move {
            blocking(move || {
                let mut deleted = 0;
                for name in enumerate(&filter)? {
                    if delete(&name)? {
                        deleted += 1;
                    }
                }
                Ok(deleted)
            })
            .await
        })
    }
}
