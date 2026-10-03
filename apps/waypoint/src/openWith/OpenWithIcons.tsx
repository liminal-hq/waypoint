// Inline glyphs for Open With, and an application's own icon with a glyph where it has none
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useState } from 'react';
import { Glyph, type IconProps } from '../icons/AppIcons';
import styles from './OpenWithIcons.module.css';

/** A window with a corner turned up, for Open With and an application that has no icon. */
export const OpenWithIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2.5 3.5h8v3.5M2.5 3.5v9h9M6.5 9.5l7-7M9.5 2.5h4v4" />
	</Glyph>
);

interface AppIconProps {
	/** The icon's address (`OpenWithClient.iconUrl`). */
	src: string;
}

/** An application's icon at 16 px; the glyph stands in when the picture is not served. */
export function AppIcon({ src }: AppIconProps) {
	const [failed, setFailed] = useState(false);
	if (failed) return <OpenWithIcon />;
	return (
		<img
			className={styles.icon}
			src={src}
			alt=""
			width={16}
			height={16}
			draggable={false}
			onError={() => setFailed(true)}
		/>
	);
}
