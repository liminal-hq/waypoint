// Records the Wayland proxies GTK3 does not expose: the `xdg_toplevel` of every window and GTK's `wl_data_device`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// GDK 3's protocol stubs are inlined, so every request they make goes through `wl_proxy_marshal_flags`. This file
// defines that function, forwards each call to libwayland's `wl_proxy_marshal_array_flags`, and notes which proxy a
// request created. For the interposer to win symbol resolution the final executable must export it: link it with
// `-rdynamic` and `-Wl,--undefined=wl_proxy_marshal_flags` (a library crate cannot add link arguments downstream).
// Nothing here changes a request; where GDK is not on Wayland, or the symbol is not exported, nothing is recorded.
//
// The types are declared here (matching `wayland-util.h`) so building needs no Wayland headers.

#define _GNU_SOURCE
#include <dlfcn.h>
#include <pthread.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct wl_proxy;
struct wl_object;
struct wl_array;
struct wl_interface;

struct wl_message {
  const char *name;
  const char *signature;
  const struct wl_interface **types;
};

struct wl_interface {
  const char *name;
  int version;
  int method_count;
  const struct wl_message *methods;
  int event_count;
  const struct wl_message *events;
};

union wl_argument {
  int32_t i;
  uint32_t u;
  int32_t f;
  const char *s;
  struct wl_object *o;
  uint32_t n;
  struct wl_array *a;
  int32_t h;
};

extern const struct wl_interface *wl_proxy_get_interface(struct wl_proxy *proxy) __attribute__((weak));

#define WTD_MARSHAL_FLAG_DESTROY 1

static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
// xdg_surface -> wl_surface
static struct wl_proxy **xs, **xs_surf;
// xdg_toplevel -> xdg_surface
static struct wl_proxy **xt, **xt_xs;
static int nxs, nxt, capxs, capxt;
static struct wl_proxy *gtk_dd;
static unsigned long hits;
// Proxies that could not be recorded because memory ran out: their windows cannot be dragged.
static unsigned long overflows;

typedef struct wl_proxy *(*marshal_array_fn)(struct wl_proxy *, uint32_t, const struct wl_interface *, uint32_t,
                                              uint32_t, union wl_argument *);

// Makes room for one more entry in a pair of parallel arrays; false (and counted) when memory is short.
static int grow(struct wl_proxy ***a, struct wl_proxy ***b, int *cap, int n) {
  if (n < *cap) return 1;
  int next = *cap ? *cap * 2 : 64;
  struct wl_proxy **na = realloc(*a, (size_t)next * sizeof **a);
  if (na) *a = na;
  struct wl_proxy **nb = na ? realloc(*b, (size_t)next * sizeof **b) : NULL;
  if (nb) *b = nb;
  if (!na || !nb) {
    overflows++;
    return 0;
  }
  *cap = next;
  return 1;
}

// The function GDK means to call, resolved once. Looked up eagerly so a process that cannot be served says so at start.
static marshal_array_fn resolve_real(void) {
  static marshal_array_fn real;
  if (!real) real = (marshal_array_fn)dlsym(RTLD_NEXT, "wl_proxy_marshal_array_flags");
  return real;
}

__attribute__((constructor)) static void wtd_init(void) {
  if (!resolve_real() && getenv("WAYLAND_DISPLAY")) {
    fprintf(stderr, "wayland-toplevel-drag: `wl_proxy_marshal_array_flags` is not available; Wayland requests cannot be forwarded\n");
  }
}

static void forget(struct wl_proxy *p) {
  for (int i = 0; i < nxs; i++) {
    if (xs[i] == p) {
      xs[i] = xs[nxs - 1];
      xs_surf[i] = xs_surf[nxs - 1];
      nxs--;
      break;
    }
  }
  for (int i = 0; i < nxt; i++) {
    if (xt[i] == p) {
      xt[i] = xt[nxt - 1];
      xt_xs[i] = xt_xs[nxt - 1];
      nxt--;
      break;
    }
  }
  if (gtk_dd == p) gtk_dd = NULL;
}

struct wl_proxy *wl_proxy_marshal_flags(struct wl_proxy *proxy, uint32_t opcode, const struct wl_interface *interface,
                                        uint32_t version, uint32_t flags, ...) {
  marshal_array_fn real = resolve_real();
  // This function replaces GDK's own call, so returning without forwarding would silently break the request for the
  // whole process. Without these two symbols nothing here can be forwarded: say so and stop, rather than carry on wrong.
  if (!real || !wl_proxy_get_interface) {
    fprintf(stderr, "wayland-toplevel-drag: cannot forward a Wayland request (`wl_proxy_marshal_array_flags`%s); aborting\n",
            real ? " found, `wl_proxy_get_interface` missing" : " missing");
    abort();
  }
  const struct wl_interface *pi = wl_proxy_get_interface(proxy);
  const char *sig = pi->methods[opcode].signature;
  union wl_argument args[20];
  memset(args, 0, sizeof args);
  int n = 0;
  va_list ap;
  va_start(ap, flags);
  for (const char *p = sig; *p && n < 20; p++) {
    switch (*p) {
    case '?':
    case '0' ... '9':
      continue;
    case 'i': args[n++].i = va_arg(ap, int32_t); break;
    case 'u': args[n++].u = va_arg(ap, uint32_t); break;
    case 'f': args[n++].f = va_arg(ap, int32_t); break;
    case 's': args[n++].s = va_arg(ap, const char *); break;
    case 'o': args[n++].o = va_arg(ap, struct wl_object *); break;
    case 'n': args[n++].o = va_arg(ap, struct wl_object *); break;
    case 'a': args[n++].a = va_arg(ap, struct wl_array *); break;
    case 'h': args[n++].h = va_arg(ap, int32_t); break;
    }
  }
  va_end(ap);

  struct wl_proxy *ret = real(proxy, opcode, interface, version, flags, args);

  pthread_mutex_lock(&lock);
  hits++;
  if (flags & WTD_MARSHAL_FLAG_DESTROY) forget(proxy);
  if (ret && interface && interface->name) {
    if (!strcmp(interface->name, "xdg_surface")) {
      if (grow(&xs, &xs_surf, &capxs, nxs)) {
        xs[nxs] = ret;
        xs_surf[nxs++] = (struct wl_proxy *)args[1].o;
      }
    } else if (!strcmp(interface->name, "xdg_toplevel")) {
      if (grow(&xt, &xt_xs, &capxt, nxt)) {
        xt[nxt] = ret;
        xt_xs[nxt++] = proxy;
      }
    } else if (!strcmp(interface->name, "wl_data_device")) {
      // The latest one: a single seat is assumed.
      gtk_dd = ret;
    }
  }
  pthread_mutex_unlock(&lock);
  return ret;
}

// The number of requests the interposer has seen: non-zero only if it is the `wl_proxy_marshal_flags` GDK calls.
unsigned long wtd_interposer_hits(void) {
  pthread_mutex_lock(&lock);
  unsigned long h = hits;
  pthread_mutex_unlock(&lock);
  return h;
}

// How many proxies could not be recorded (memory ran out): non-zero means some window cannot be dragged.
unsigned long wtd_interposer_overflows(void) {
  pthread_mutex_lock(&lock);
  unsigned long n = overflows;
  pthread_mutex_unlock(&lock);
  return n;
}

// The `xdg_toplevel` GDK made for `wl_surface`, or NULL.
void *wtd_find_xdg_toplevel(void *wl_surface) {
  void *found = NULL;
  pthread_mutex_lock(&lock);
  for (int i = 0; i < nxt && !found; i++)
    for (int j = 0; j < nxs; j++)
      if (xs[j] == xt_xs[i] && xs_surf[j] == wl_surface) {
        found = xt[i];
        break;
      }
  pthread_mutex_unlock(&lock);
  return found;
}

// GTK's `wl_data_device`, or NULL.
void *wtd_find_data_device(void) {
  pthread_mutex_lock(&lock);
  void *p = gtk_dd;
  pthread_mutex_unlock(&lock);
  return p;
}
