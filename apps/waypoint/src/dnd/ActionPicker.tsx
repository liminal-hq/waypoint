// The picker a right-button or Alt drop opens at the release point: Copy Here, Move Here, Link Here, Cancel
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import { useRef, type ReactNode } from 'react';
import { t, type MessageId } from '../i18n/messages';
import { CloseSmallIcon } from '../icons/AppIcons';
import { CopyToIcon, LinkIcon, MoveToIcon } from '../icons/MenuIcons';
import type { PickerRequest } from './fileDrag';
import type { PickerVerb as Verb } from './dropAction';

const ITEMS: Record<Verb, { label: MessageId; icon: () => ReactNode }> = {
	copy: { label: 'dnd.picker.copy', icon: () => <CopyToIcon /> },
	move: { label: 'dnd.picker.move', icon: () => <MoveToIcon /> },
	link: { label: 'dnd.picker.link', icon: () => <LinkIcon /> },
	compress: { label: 'dnd.picker.compress', icon: () => <CopyToIcon /> },
	extract: { label: 'dnd.picker.extract', icon: () => <MoveToIcon /> },
};

/** The menu items for the verbs on offer, ending with Cancel. */
export function pickerItems(verbs: readonly Verb[], intoArchive = false): MenuItem[] {
	return [
		...verbs.map((verb): MenuItem => ({
			type: 'action',
			id: verb,
			label: t(intoArchive && verb === 'copy' ? 'dnd.picker.add' : ITEMS[verb].label),
			icon: ITEMS[verb].icon(),
		})),
		{ type: 'separator', id: 'picker-separator' },
		{ type: 'action', id: 'cancel', label: t('dnd.picker.cancel'), icon: <CloseSmallIcon /> },
	];
}

interface ActionPickerProps {
	request: PickerRequest;
	/** Called once the picker has gone, whichever way. */
	onDone: () => void;
}

/**
 * The shared context menu, so the keyboard works as it does everywhere: arrows move, Enter chooses,
 * Esc cancels. Closing it without choosing is a cancel, and says so.
 */
export function ActionPicker({ request, onDone }: ActionPickerProps) {
	const chosen = useRef(false);
	const finished = useRef(false);
	return (
		<ContextMenu
			items={pickerItems(request.verbs, request.intoArchive === true)}
			position={request.position}
			ariaLabel={t('dnd.picker.label')}
			onSelect={(item) => {
				if (item.id === 'cancel') return;
				chosen.current = true;
				request.choose(item.id as Verb);
			}}
			onClose={() => {
				if (finished.current) return;
				finished.current = true;
				if (!chosen.current) request.cancel();
				onDone();
			}}
		/>
	);
}
