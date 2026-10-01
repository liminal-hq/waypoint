// Test setup: jest-dom matchers and DOM cleanup between tests
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

// happy-dom's `<dialog>` only toggles the `open` attribute. This mirrors what the platform adds for
// `showModal()`: an InvalidStateError when already open, a stack of modals, and Esc firing a
// cancelable `cancel` event at the top one and then closing it unless that event is prevented. It
// does not trap focus or make the page inert; the real platform does, and the component has its own
// Tab trap that the tests exercise.
const modals: HTMLDialogElement[] = [];

HTMLDialogElement.prototype.showModal = function showModal(this: HTMLDialogElement) {
	if (this.hasAttribute('open')) {
		throw new DOMException('The dialog is already open', 'InvalidStateError');
	}
	this.setAttribute('open', '');
	modals.push(this);
};

const nativeClose = HTMLDialogElement.prototype.close;
HTMLDialogElement.prototype.close = function close(this: HTMLDialogElement, value?: string) {
	const at = modals.indexOf(this);
	if (at >= 0) modals.splice(at, 1);
	nativeClose.call(this, value);
};

// On `window`, after every document and element listener: like a default action, it only runs when
// nothing prevented the keydown.
window.addEventListener('keydown', (event) => {
	if (event.key !== 'Escape' || event.defaultPrevented) return;
	const top = modals[modals.length - 1];
	if (!top) return;
	const cancel = new Event('cancel', { cancelable: true });
	top.dispatchEvent(cancel);
	if (!cancel.defaultPrevented) top.close();
});

afterEach(() => {
	cleanup();
	modals.length = 0;
});
