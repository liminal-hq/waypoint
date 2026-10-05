// Verifies the Connect dialog's S3 form: the service presets, the bucket and access key, the secret and session token, and what is saved
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { stubLayout } from '../test/browseHarness';
import { renderWorkspace } from '../test/workspaceHarness';
import { connectStore, openConnectDialog } from './connectStore';
import { draft, FakeConnectionsClient } from './fakeConnectionsClient';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	act(() => {
		connectStore.getState().answer(null);
		connectStore.getState().close();
	});
	cleanup();
	restoreLayout();
});

const withS3 = (options: ConstructorParameters<typeof FakeConnectionsClient>[0] = {}) =>
	new FakeConnectionsClient({ schemes: ['sftp', 's3'], ...options });

async function setup(client: FakeConnectionsClient) {
	await renderWorkspace(undefined, new FakeTabsApi(), undefined, { connections: client });
	act(() => openConnectDialog());
	const dialog = await screen.findByRole('dialog', { name: 'Connect to Server' });
	const protocol = await within(dialog).findByRole('combobox', { name: 'Protocol' });
	fireEvent.change(protocol, { target: { value: 's3' } });
	return dialog;
}

const box = (dialog: HTMLElement, name: string | RegExp) =>
	within(dialog).getByRole('textbox', { name }) as HTMLInputElement;
const secret = (dialog: HTMLElement, name: string | RegExp) =>
	within(dialog).getByLabelText(name, { selector: 'input[type="password"]' });
const lastCall = (client: FakeConnectionsClient, method: string) =>
	client.calls.filter((call) => call.method === method).at(-1);
const service = (dialog: HTMLElement) =>
	within(dialog).getByRole('combobox', { name: 'Service' }) as HTMLSelectElement;

describe('an S3 connection', () => {
	it('asks for a bucket, an access key and its secret, and for none of what other protocols have', async () => {
		const dialog = await setup(withS3());
		expect(box(dialog, 'Bucket')).toBeVisible();
		expect(box(dialog, 'Access key ID')).toBeVisible();
		expect(secret(dialog, 'Secret access key')).toBeVisible();
		expect(secret(dialog, 'Session token (optional)')).toBeVisible();
		expect(within(dialog).queryByRole('textbox', { name: 'Port' })).toBeNull();
		expect(within(dialog).queryByRole('group', { name: 'Sign in with' })).toBeNull();
		expect(within(dialog).queryByRole('textbox', { name: 'Domain' })).toBeNull();
		expect(box(dialog, 'Address').placeholder).toBe('s3://bucket/folder');
	});

	it('lists the services, and says in a note that a move copies then deletes', async () => {
		const dialog = await setup(withS3());
		expect(
			within(service(dialog))
				.getAllByRole('option')
				.map((o) => o.textContent),
		).toEqual([
			'Amazon S3',
			'Backblaze B2',
			'Cloudflare R2',
			'Wasabi',
			'MinIO',
			'DigitalOcean Spaces',
			'Another S3 service',
		]);
		expect(within(dialog).getByRole('note', { name: '' })).toHaveTextContent(
			/moving a file here copies it on the service and then deletes the original/,
		);
	});

	it('asks for what the chosen service needs to find its endpoint', async () => {
		const dialog = await setup(withS3());
		// Amazon S3 needs nothing: its region is optional and found by itself.
		expect(within(dialog).queryByRole('textbox', { name: 'Account ID' })).toBeNull();
		expect(box(dialog, 'Region (optional)')).toBeVisible();
		fireEvent.change(service(dialog), { target: { value: 'r2' } });
		expect(box(dialog, 'Account ID')).toBeVisible();
		fireEvent.change(service(dialog), { target: { value: 'b2' } });
		expect(box(dialog, 'Region')).toBeVisible();
		expect(within(dialog).queryByRole('textbox', { name: 'Region (optional)' })).toBeNull();
		fireEvent.change(service(dialog), { target: { value: 'minio' } });
		expect(box(dialog, 'Server')).toBeVisible();
		fireEvent.change(service(dialog), { target: { value: 'custom' } });
		expect(box(dialog, 'Endpoint')).toBeVisible();
	});

	it('puts the bucket in the path where the service needs it, and lets the person change that', async () => {
		const dialog = await setup(withS3());
		const pathStyle = () =>
			within(dialog).getByRole('checkbox', { name: 'Put the bucket in the path' });
		expect(pathStyle()).not.toBeChecked();
		fireEvent.change(service(dialog), { target: { value: 'minio' } });
		expect(pathStyle()).toBeChecked();
		fireEvent.click(pathStyle());
		expect(pathStyle()).not.toBeChecked();
	});

	it('saves the bucket, the key id, the endpoint a preset makes and the options, and no secret', async () => {
		const client = withS3();
		const dialog = await setup(client);
		fireEvent.change(box(dialog, 'Bucket'), { target: { value: 'Photos' } });
		fireEvent.change(box(dialog, 'Access key ID'), { target: { value: '004abc' } });
		fireEvent.change(secret(dialog, 'Secret access key'), { target: { value: 'hunter2' } });
		fireEvent.change(service(dialog), { target: { value: 'b2' } });
		fireEvent.change(box(dialog, 'Region'), { target: { value: 'us-west-004' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
		await within(dialog).findByRole('button', { name: /Edit/ });
		const saved = lastCall(client, 'add')?.args[0] as ReturnType<typeof draft>;
		expect(saved).toMatchObject({
			scheme: 's3',
			host: 'Photos',
			port: null,
			user: '004abc',
			auth: 'auto',
		});
		expect(saved.options).toMatchObject({
			s3Endpoint: 'https://s3.us-west-004.backblazeb2.com',
			s3Preset: 'b2',
			s3Region: null,
			s3PathStyle: null,
		});
		expect(JSON.stringify(saved)).not.toContain('hunter2');
	});

	it('sends the access key, its secret and the session token as the answer to the first attempt', async () => {
		const client = withS3();
		const dialog = await setup(client);
		fireEvent.change(box(dialog, 'Bucket'), { target: { value: 'photos' } });
		fireEvent.change(box(dialog, 'Access key ID'), { target: { value: 'AKIA' } });
		fireEvent.change(secret(dialog, 'Secret access key'), { target: { value: 'sssh' } });
		fireEvent.change(secret(dialog, 'Session token (optional)'), { target: { value: 'tok' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Test' }));
		await within(dialog).findByText(/^Connected to/);
		expect(client.answers).toEqual([
			{ kind: 'accessKey', keyId: 'AKIA', secret: 'sssh', sessionToken: 'tok' },
		]);
		// The secret is gone from the form once it was sent.
		expect((secret(dialog, 'Secret access key') as HTMLInputElement).value).toBe('');
	});

	it('names the bucket and the service input that are missing, before anything is sent', async () => {
		const client = withS3();
		const dialog = await setup(client);
		fireEvent.click(within(dialog).getByRole('button', { name: 'Test' }));
		await waitFor(() => expect(box(dialog, 'Bucket')).toHaveAttribute('aria-invalid', 'true'));
		expect(box(dialog, 'Bucket')).toHaveAccessibleDescription('Type the name of the bucket.');
		fireEvent.change(box(dialog, 'Bucket'), { target: { value: 'photos' } });
		fireEvent.change(service(dialog), { target: { value: 'r2' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Test' }));
		await waitFor(() => expect(box(dialog, 'Account ID')).toHaveAttribute('aria-invalid', 'true'));
		expect(lastCall(client, 'test')).toBeUndefined();
	});

	it('reads a typed address with an endpoint into the fields', async () => {
		const dialog = await setup(withS3());
		fireEvent.change(box(dialog, 'Address'), {
			target: { value: 's3://Photos/2026?endpoint=https%3A%2F%2Fs3.eu-central-1.wasabisys.com' },
		});
		await waitFor(() => expect(box(dialog, 'Bucket').value).toBe('Photos'));
		expect(service(dialog).value).toBe('wasabi');
		expect(box(dialog, 'Region').value).toBe('eu-central-1');
	});

	it('shows a saved connection’s service, key id and bucket when it is edited', async () => {
		const client = withS3({
			connections: [
				draft({
					scheme: 's3',
					host: 'media',
					user: 'AKIA',
					options: {
						...draft().options,
						s3Endpoint: 'https://abc123.r2.cloudflarestorage.com',
						s3Preset: 'r2',
					},
				}),
			],
		});
		await renderWorkspace(undefined, new FakeTabsApi(), undefined, { connections: client });
		act(() => openConnectDialog({ mode: 'edit', id: 'c1' }));
		const dialog = await screen.findByRole('dialog', { name: 'Edit Connection' });
		await waitFor(() => expect(box(dialog, 'Bucket').value).toBe('media'));
		expect(box(dialog, 'Access key ID').value).toBe('AKIA');
		expect(service(dialog).value).toBe('r2');
		expect(box(dialog, 'Account ID').value).toBe('abc123');
	});
});
