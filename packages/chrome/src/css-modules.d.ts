// Ambient typing for CSS Modules
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

declare module '*.module.css' {
	const classes: Record<string, string>;
	export default classes;
}
