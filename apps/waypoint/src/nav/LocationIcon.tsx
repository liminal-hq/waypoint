// The icon of a location in a tab or a menu: its folder in the window's icon theme, or a server badge for a remote one
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { ConnectionState } from '@liminal-hq/waypoint-protocol/generated/ConnectionState';
import { FileIcon } from '../browse/FileIcon';
import { useSettleHold } from '../icons/iconSettle';
import { useOptionalVfsClient } from '../browse/VfsClientContext';
import { ServerIcon } from '../connections/ConnectionIcons';
import { useConnectionsView } from '../connections/ConnectionsContext';
import { stateOf } from '../connections/connectionsModel';
import { stateTone } from '../connections/remoteModel';
import type { VfsClient } from '../services/vfsClient';
import styles from './LocationIcon.module.css';
import { useLocationInfo } from './locationInfo';

interface LocationGlyphProps {
	/** The state of the server the location is on; `null` for a local folder. */
	remote: ConnectionState | null;
	/** An extra class for the glyph. */
	className?: string;
}

/**
 * What a tab and every menu row that names a location draw for it, so they cannot drift apart: the
 * folder in the window's icon theme, or the server glyph with a corner dot for its state where the
 * location is on a server. Decorative: the row's own text carries the meaning.
 */
export function LocationGlyph({ remote, className }: LocationGlyphProps) {
	if (!remote) return <FileIcon group="folder" className={className} />;
	return (
		<span
			className={className ? `${styles.remote} ${className}` : styles.remote}
			data-tone={stateTone(remote)}
		>
			<ServerIcon />
		</span>
	);
}

function ResolvedLocationIcon({
	client,
	location,
	className,
}: LocationIconProps & { client: VfsClient }) {
	const info = useLocationInfo(client, location);
	// Until Rust has said which server the location is on, the folder is a stand-in for the server glyph.
	useSettleHold(info === null);
	const connection = info?.connection;
	const remote = useConnectionsView((view) => (connection ? stateOf(view, connection) : null));
	return <LocationGlyph remote={remote} className={className} />;
}

interface LocationIconProps {
	location: Location;
	className?: string;
}

/**
 * The icon for `location`, for a menu row (a model builds it with `createElement`, so it reads the
 * window's icon theme and the server's state itself and follows both live). Outside a window, with no
 * file system to ask, it is the plain folder.
 */
export function LocationIcon({ location, className }: LocationIconProps) {
	const client = useOptionalVfsClient();
	if (!client) return <LocationGlyph remote={null} className={className} />;
	return <ResolvedLocationIcon client={client} location={location} className={className} />;
}
