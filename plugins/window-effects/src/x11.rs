// Asks an X11 compositor to blur behind a window through the `_KDE_NET_WM_BLUR_BEHIND_REGION` property
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, PropMode};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

const BLUR_PROPERTY: &[u8] = b"_KDE_NET_WM_BLUR_BEHIND_REGION";

/// Sets the blur region of the X11 window `xid`, `region` being `x, y, width, height` for each rectangle in device pixels (empty: the whole window), or removes the effect when `region` is `None`. A property can be changed by any client, so this uses a connection of its own to the display and closes it again.
pub fn set_blur(display: &str, xid: u32, region: Option<&[u32]>) -> Result<(), String> {
    let (conn, _) = RustConnection::connect(Some(display)).map_err(|error| error.to_string())?;
    let atom = conn
        .intern_atom(false, BLUR_PROPERTY)
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| error.to_string())?
        .atom;
    match region {
        Some(region) => conn
            .change_property32(PropMode::REPLACE, xid, atom, AtomEnum::CARDINAL, region)
            .map(drop),
        None => conn.delete_property(xid, atom).map(drop),
    }
    .map_err(|error| error.to_string())?;
    // A round trip, so the server has handled the change before the connection closes.
    conn.get_input_focus()
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| error.to_string())?;
    Ok(())
}
