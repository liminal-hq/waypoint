// The dot beside a sidebar place or favourite that has changes inside it, with the count in words
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { GitMarkView } from '../git/GitMarkView';
import type { GitBadge } from '../services/gitClient';

/** Nothing for a folder with nothing changed or outside a working tree. */
export function SidebarGitMark({ badge }: { badge: GitBadge | undefined }) {
	if (!badge || badge.changed === 0) return null;
	return (
		<GitMarkView
			mark={{ inside: badge.changed, conflictedInside: badge.conflicted }}
			variant="chip"
		/>
	);
}
