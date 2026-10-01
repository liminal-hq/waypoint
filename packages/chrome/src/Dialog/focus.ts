// Focus helpers for the dialog: tabbable discovery, the default initial target and the Tab trap
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const TABBABLE =
	'a[href], button, input:not([type="hidden"]), select, textarea, summary, [tabindex], [contenteditable]:not([contenteditable="false"])';

function isTabbable(element: HTMLElement, dialog: HTMLElement): boolean {
	if (element.closest('dialog') !== dialog) return false; // inside a stacked dialog
	if (element.hasAttribute('disabled') || element.hidden) return false;
	if (element.closest('[inert], [hidden]')) return false;
	const tabindex = element.getAttribute('tabindex');
	return tabindex === null || Number.parseInt(tabindex, 10) >= 0;
}

/** Elements a Tab press can reach inside `dialog`, in DOM order, leaving out stacked dialogs. */
export function tabbables(dialog: HTMLElement): HTMLElement[] {
	return [...dialog.querySelectorAll<HTMLElement>(TABBABLE)].filter((el) => isTabbable(el, dialog));
}

/**
 * The default focus target: the first field in the body, otherwise a footer button that is never
 * destructive. With a danger button present the secondary (cancel) button wins; without one the
 * primary does. Returns null when only destructive buttons exist.
 */
export function defaultFocusTarget(dialog: HTMLElement): HTMLElement | null {
	const marked = dialog.querySelector<HTMLElement>('[data-autofocus]');
	if (marked && marked.closest('dialog') === dialog) return marked;
	const body = dialog.querySelector<HTMLElement>('[data-dialog-body]');
	const field = body && tabbables(dialog).find((el) => body.contains(el));
	if (field) return field;
	const buttons = tabbables(dialog).filter(
		(el) => el.hasAttribute('data-dialog-button') && el.dataset.variant !== 'danger',
	);
	const hasDanger = dialog.querySelector('[data-dialog-button][data-variant="danger"]') !== null;
	const preferred = hasDanger ? 'secondary' : 'primary';
	return buttons.find((el) => el.dataset.variant === preferred) ?? buttons[0] ?? null;
}

/** Keeps Tab and Shift+Tab inside `dialog`. Returns true when it moved focus itself. */
export function trapTab(event: KeyboardEvent, dialog: HTMLElement, fallback: HTMLElement): boolean {
	const items = tabbables(dialog);
	if (items.length === 0) {
		event.preventDefault();
		fallback.focus();
		return true;
	}
	const first = items[0] as HTMLElement;
	const last = items[items.length - 1] as HTMLElement;
	const active = document.activeElement;
	const inside = active instanceof HTMLElement && dialog.contains(active);
	if (event.shiftKey && (!inside || active === first || active === fallback)) {
		event.preventDefault();
		last.focus();
		return true;
	}
	if (!event.shiftKey && (!inside || active === last)) {
		event.preventDefault();
		first.focus();
		return true;
	}
	return false;
}
