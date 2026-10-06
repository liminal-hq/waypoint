// The WebDAV provider against a scripted server: the failures, challenges and odd answers a real
// server will not give on demand, and what the provider sends in each request.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::io::{Read, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, UNIX_EPOCH};

use base64::Engine as _;
use support::mock::{multistatus, Body, Mock, Reply, Seen};
use support::{config, options, plain, Logins, PASSWORD, USER};
use waypoint_path::VfsPath;
use waypoint_protocol::{AuthPrompt, ConnectionState, VfsError};
use waypoint_provider_webdav::{AuthMode, Preset, WebDavConfig, WebDavProvider};
use waypoint_vfs::{CancelToken, EntryKind, Provider, ScannedEntry, VolumeSpace, WriteOptions};

fn at(mock: &Mock, path: &str) -> VfsPath {
    VfsPath::from_uri(&format!("dav://127.0.0.1:{}{path}", mock.port)).unwrap()
}

fn list(provider: &WebDavProvider, path: &VfsPath) -> Result<Vec<ScannedEntry>, VfsError> {
    provider.list(path, &CancelToken::new(), 0, &mut |_| {})
}

fn ok_listing(seen: &Seen) -> Reply {
    let folder = seen.target.trim_end_matches('/');
    Reply::xml(
        207,
        multistatus(&format!("{folder}/"), (0..3).map(|n| format!("f{n}"))),
    )
}

fn not_found() -> Reply {
    Reply::new(404)
}

fn file_xml(href: &str) -> String {
    format!(
        "<D:multistatus xmlns:D=\"DAV:\">{}</D:multistatus>",
        support::mock::entry_xml("", href)
    )
}

#[test]
fn a_server_that_asks_to_slow_down_is_rate_limited() {
    let mock = Mock::start(|_, n| match n {
        0 => Reply::new(429).header("Retry-After", "3"),
        1 => Reply::new(503).header("Retry-After", "Wed, 21 Oct 2037 07:28:00 GMT"),
        2 => Reply::new(429),
        _ => Reply::new(503),
    });
    let provider = plain(None);
    let p = at(&mock, "/x");
    assert!(matches!(
        provider.stat(&p),
        Err(VfsError::RateLimited {
            retry_after_ms: Some(3000),
            ..
        })
    ));
    assert!(matches!(
        provider.stat(&p),
        Err(VfsError::RateLimited { retry_after_ms: Some(ms), .. }) if ms > 86_400_000
    ));
    assert!(matches!(
        provider.stat(&p),
        Err(VfsError::RateLimited {
            retry_after_ms: None,
            ..
        })
    ));
    // A 503 that says nothing about when is an ordinary failure.
    assert!(matches!(provider.stat(&p), Err(VfsError::Io { .. })));
    assert_eq!(mock.requests().len(), 4, "nothing is retried by itself");
}

#[test]
fn a_server_that_leaves_properties_out_still_lists() {
    let mock =
        Mock::start(|_, _| Reply::xml(207, include_str!("fixtures/nginx-dav-ext-depth1.xml")));
    let provider = plain(None);
    let entries = list(&provider, &at(&mock, "/files/")).unwrap();
    let find = |name: &str| entries.iter().find(|e| e.name == name).unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(find("docs").kind, EntryKind::Directory);
    assert_eq!(find("café.txt").size, Some(1500));
    let bare = find("no properties.bin");
    assert_eq!(
        (bare.kind, bare.size, bare.modified_ms),
        (EntryKind::File, None, None)
    );
    let sent = &mock.requests()[0];
    assert_eq!(sent.method, "PROPFIND");
    assert_eq!(sent.header("depth"), Some("1"));
    let body = String::from_utf8_lossy(&sent.body);
    assert!(body.contains("<resourcetype/>") && !body.contains("checksums"));
    // A listing asks only for what its rows show.
    assert!(!body.contains("getetag") && !body.contains("getcontenttype"));
    assert_eq!(
        sent.target, "/files",
        "a folder is asked for without a slash"
    );
}

#[test]
fn hrefs_that_are_absolute_or_under_another_prefix_still_name_the_entries() {
    let mock = Mock::start(|_, _| {
        Reply::xml(
            207,
            "<multistatus xmlns=\"DAV:\">\
             <response><href>/proxy/prefix/x/</href><propstat><prop><resourcetype><collection/></resourcetype></prop><status>HTTP/1.1 200 OK</status></propstat></response>\
             <response><href>http://127.0.0.1:1/proxy/prefix/x/a%20b</href><propstat><prop><getcontentlength>4</getcontentlength></prop><status>HTTP/1.1 200 OK</status></propstat></response>\
             <response><href>/proxy/prefix/x/gone</href><status>HTTP/1.1 404 Not Found</status></response>\
             <response><href>/proxy/prefix/x/sub/</href><propstat><prop><resourcetype><collection/></resourcetype></prop><status>HTTP/1.1 200 OK</status></propstat></response>\
             </multistatus>",
        )
    });
    let entries = list(&plain(None), &at(&mock, "/x")).unwrap();
    let names: Vec<_> = entries
        .iter()
        .map(|e| e.name.to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["a b", "sub"]);
    assert_eq!(entries[0].size, Some(4));
    assert_eq!(entries[1].kind, EntryKind::Directory);
}

#[test]
fn an_empty_folder_and_a_file_are_told_apart() {
    let mock = Mock::start(|seen, _| {
        match seen.target.as_str() {
        "/empty" => Reply::xml(207, multistatus("/empty/", std::iter::empty())),
        "/file.txt" => Reply::xml(207, file_xml("/file.txt")),
        // A server that leaves `resourcetype` out of the folder's own response.
        "/loose" => Reply::xml(
            207,
            "<multistatus xmlns=\"DAV:\"><response><href>/loose/</href><propstat><prop/><status>HTTP/1.1 200 OK</status></propstat></response></multistatus>",
        ),
        _ => not_found(),
    }
    });
    let provider = plain(None);
    assert!(list(&provider, &at(&mock, "/empty")).unwrap().is_empty());
    assert!(list(&provider, &at(&mock, "/loose")).unwrap().is_empty());
    assert!(matches!(
        list(&provider, &at(&mock, "/file.txt")),
        Err(VfsError::NotADirectory { .. })
    ));
    assert!(matches!(
        list(&provider, &at(&mock, "/nothing")),
        Err(VfsError::NotFound { .. })
    ));
}

#[test]
fn an_answer_that_is_not_xml_is_an_error_not_an_empty_folder() {
    let mock = Mock::start(|_, n| match n {
        0 => Reply::new(207).body("<html>proxy error"),
        _ => Reply::new(207).body(""),
    });
    let provider = plain(None);
    assert!(matches!(
        list(&provider, &at(&mock, "/x")),
        Err(VfsError::Io { .. })
    ));
    assert!(matches!(
        list(&provider, &at(&mock, "/x")),
        Err(VfsError::Io { .. })
    ));
}

#[test]
fn names_are_escaped_on_the_wire_and_folders_end_in_a_slash() {
    let mock = Mock::start(|seen, _| match seen.method.as_str() {
        "PROPFIND" => not_found(),
        "MKCOL" => Reply::new(201),
        "MOVE" => Reply::new(201),
        _ => Reply::new(500),
    });
    let provider = plain(None);
    let folder = at(&mock, "/a%20b/100%25/%23hash/caf%C3%A9");
    provider.create_dir(&folder).unwrap();
    let requests = mock.requests();
    let mkcol = requests.iter().find(|r| r.method == "MKCOL").unwrap();
    assert_eq!(mkcol.target, "/a%20b/100%25/%23hash/caf%C3%A9/");
    let from = at(&mock, "/semi%3Bcolon%2Ccomma");
    let to = at(&mock, "/q%3Fmark/%E6%97%A5%E6%9C%AC%E8%AA%9E");
    provider.rename(&from, &to, false).unwrap();
    let requests = mock.requests();
    let moved = requests.iter().find(|r| r.method == "MOVE").unwrap();
    assert_eq!(moved.target, "/semi%3Bcolon%2Ccomma");
    assert_eq!(
        moved.header("destination"),
        Some(
            format!(
                "http://127.0.0.1:{}/q%3Fmark/%E6%97%A5%E6%9C%AC%E8%AA%9E",
                mock.port
            )
            .as_str()
        )
    );
    assert_eq!(moved.header("overwrite"), Some("F"));
    provider.rename(&from, &to, true).unwrap();
    assert_eq!(
        mock.requests().last().unwrap().header("overwrite"),
        Some("T")
    );
}

#[test]
fn redirects_are_followed_and_a_login_never_follows_them_to_another_origin() {
    let elsewhere = Mock::start(|seen, _| ok_listing(seen));
    let elsewhere_port = elsewhere.port;
    let mock = Mock::start(move |seen, _| {
        if seen.target.starts_with("/moved") {
            Reply::new(308).header(
                "Location",
                &format!("http://127.0.0.1:{elsewhere_port}/there"),
            )
        } else if seen.target == "/old" {
            Reply::new(301).header("Location", "/new")
        } else if seen.header("authorization").is_none() {
            Reply::new(401).header("WWW-Authenticate", "Basic realm=\"x\"")
        } else {
            ok_listing(seen)
        }
    });
    let logins = Logins::with(USER, PASSWORD);
    let provider = plain(Some(logins));
    // Same origin: followed, with the method and the depth kept, and the login sent.
    assert_eq!(list(&provider, &at(&mock, "/old")).unwrap().len(), 3);
    let requests = mock.requests();
    let followed = requests
        .iter()
        .filter(|r| r.target == "/new")
        .collect::<Vec<_>>();
    assert!(!followed.is_empty());
    assert!(followed
        .iter()
        .all(|r| r.method == "PROPFIND" && r.header("depth") == Some("1")));
    // Another origin: followed, and asked for nothing it was not asked for.
    assert_eq!(list(&provider, &at(&mock, "/moved")).unwrap().len(), 3);
    let there = elsewhere.requests();
    assert_eq!(there.len(), 1);
    assert!(
        there[0].header("authorization").is_none(),
        "the login stays at its origin"
    );
}

#[test]
fn a_redirect_loop_ends() {
    let mock = Mock::start(|_, _| Reply::new(302).header("Location", "/again"));
    assert!(matches!(
        plain(None).stat(&at(&mock, "/x")),
        Err(VfsError::Io { .. })
    ));
}

fn basic(user: &str, password: &str) -> String {
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"))
    )
}

#[test]
fn basic_is_sent_ahead_once_the_server_has_asked_for_it() {
    let expected = basic(USER, PASSWORD);
    let mock = Mock::start(move |seen, _| match seen.header("authorization") {
        None => Reply::new(401).header("WWW-Authenticate", "Basic realm=\"files\""),
        Some(sent) if sent == expected => ok_listing(seen),
        Some(_) => Reply::new(403),
    });
    let logins = Logins::with(USER, PASSWORD);
    let provider = plain(Some(logins.clone()));
    let p = at(&mock, "/x");
    list(&provider, &p).unwrap();
    list(&provider, &p).unwrap();
    list(&provider, &p).unwrap();
    let requests = mock.requests();
    let challenged = requests
        .iter()
        .filter(|r| r.header("authorization").is_none())
        .count();
    assert_eq!(
        (requests.len(), challenged),
        (4, 1),
        "one challenge, then ahead of the question"
    );
    assert_eq!(
        logins.asked.lock().unwrap().len(),
        1,
        "the source is asked once"
    );
}

#[test]
fn a_user_in_the_location_is_the_one_the_login_is_made_as() {
    let expected = basic("bob", "pw");
    let mock = Mock::start(move |seen, _| match seen.header("authorization") {
        Some(sent) if sent == expected => ok_listing(seen),
        _ => Reply::new(401).header("WWW-Authenticate", "Basic realm=\"x\""),
    });
    // The source answers a password with no user of its own, as the keyring does for a saved login.
    struct PasswordOnly;
    impl waypoint_vfs::CredentialSource for PasswordOnly {
        fn credential(
            &self,
            _: &waypoint_path::ConnectionKey,
            prompt: &AuthPrompt,
        ) -> Option<waypoint_vfs::Credential> {
            assert_eq!(
                *prompt,
                AuthPrompt::Password {
                    user: Some("bob".to_owned())
                }
            );
            Some(waypoint_vfs::Credential::Password {
                user: None,
                password: waypoint_vfs::Secret::from("pw"),
            })
        }
    }
    let provider = WebDavProvider::dav(config(None).with_credentials(Arc::new(PasswordOnly)));
    let path = VfsPath::from_uri(&format!("dav://bob@127.0.0.1:{}/x", mock.port)).unwrap();
    assert_eq!(list(&provider, &path).unwrap().len(), 3);
}

#[test]
fn a_stale_nonce_is_answered_again_without_counting_as_a_refusal() {
    let mock = Mock::start(|seen, n| {
        let challenge = |nonce: &str, stale: bool| {
            Reply::new(401).header(
                "WWW-Authenticate",
                &format!(
                    "Digest realm=\"r\", nonce=\"{nonce}\", qop=\"auth\", algorithm=MD5{}",
                    if stale { ", stale=true" } else { "" }
                ),
            )
        };
        match n {
            0 => challenge("n1", false),
            1 => challenge("n2", true),
            _ => {
                let header = seen.header("authorization").unwrap_or_default();
                assert!(
                    header.contains("nonce=\"n2\"") && header.contains("nc=00000001"),
                    "{header}"
                );
                ok_listing(seen)
            }
        }
    });
    let logins = Logins::with(USER, PASSWORD);
    let provider = plain(Some(logins.clone()));
    assert_eq!(list(&provider, &at(&mock, "/x")).unwrap().len(), 3);
    assert_eq!(*logins.rejected.lock().unwrap(), 0);
    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests[1]
        .header("authorization")
        .unwrap()
        .starts_with("Digest "));
}

#[test]
fn digest_is_preferred_to_basic_when_both_are_offered() {
    let mock = Mock::start(|seen, _| match seen.header("authorization") {
        None => Reply::new(401)
            .header("WWW-Authenticate", "Basic realm=\"r\"")
            .header(
                "WWW-Authenticate",
                "Digest realm=\"r\", nonce=\"n\", qop=\"auth\"",
            ),
        Some(sent) => {
            assert!(sent.starts_with("Digest "), "{sent}");
            ok_listing(seen)
        }
    });
    list(&plain(Some(Logins::with(USER, PASSWORD))), &at(&mock, "/x")).unwrap();
}

#[test]
fn a_connection_set_to_digest_never_sends_a_password_in_the_clear() {
    let mock = Mock::start(|_, _| Reply::new(401).header("WWW-Authenticate", "Basic realm=\"r\""));
    let provider = WebDavProvider::dav(
        config(Some(Logins::with(USER, PASSWORD)))
            .with_options(options().with_auth(AuthMode::Digest)),
    );
    assert!(matches!(
        provider.stat(&at(&mock, "/x")),
        Err(VfsError::Io { .. })
    ));
    assert!(mock
        .requests()
        .iter()
        .all(|r| r.header("authorization").is_none()));
}

#[test]
fn a_server_that_offers_a_login_this_version_cannot_make_says_so() {
    let mock = Mock::start(|_, _| Reply::new(401).header("WWW-Authenticate", "Negotiate"));
    let provider = plain(Some(Logins::with(USER, PASSWORD)));
    assert!(
        matches!(provider.stat(&at(&mock, "/x")), Err(VfsError::Io { message, .. }) if message.contains("kind of login"))
    );
}

#[test]
fn basic_and_bearer_connections_log_in_with_the_first_request() {
    let expected = basic(USER, PASSWORD);
    let mock = Mock::start(move |seen, _| match seen.header("authorization") {
        Some(sent) if sent == expected || sent == "Bearer tok-123" => ok_listing(seen),
        _ => Reply::new(401).header("WWW-Authenticate", "Basic realm=\"x\""),
    });
    let p = at(&mock, "/x");
    let basic_provider = WebDavProvider::dav(
        config(Some(Logins::with(USER, PASSWORD)))
            .with_options(options().with_auth(AuthMode::Basic)),
    );
    list(&basic_provider, &p).unwrap();
    let bearer_provider = WebDavProvider::dav(
        config(Some(Logins::token("tok-123"))).with_options(options().with_auth(AuthMode::Bearer)),
    );
    list(&bearer_provider, &p).unwrap();
    assert_eq!(mock.requests().len(), 2, "no challenge, no second request");

    // Without a login to send, nothing is sent: the call says what to ask for.
    let asking =
        WebDavProvider::dav(config(None).with_options(options().with_auth(AuthMode::Bearer)));
    match asking.stat(&p) {
        Err(VfsError::AuthRequired { prompt, .. }) => {
            assert_eq!(
                *prompt,
                AuthPrompt::Passphrase {
                    subject: "access token".to_owned()
                }
            )
        }
        other => panic!("{other:?}"),
    }
    let asking =
        WebDavProvider::dav(config(None).with_options(options().with_auth(AuthMode::Basic)));
    assert!(matches!(
        asking.stat(&p),
        Err(VfsError::AuthRequired { .. })
    ));
    assert_eq!(mock.requests().len(), 2);
}

#[test]
fn a_listing_reaches_the_caller_before_the_server_has_finished_sending_it() {
    let (release, released) = mpsc::channel::<()>();
    let released = Mutex::new(Some(released));
    let mock = Mock::start(move |_, _| {
        let released = released.lock().unwrap().take().unwrap();
        Reply::new(207).header("Content-Type", "application/xml").tap(move |reply| {
            reply.body = Body::Stream(Box::new(move |out| {
                out.write_all(b"<?xml version=\"1.0\"?><D:multistatus xmlns:D=\"DAV:\">")?;
                out.write_all(
                    b"<D:response><D:href>/big/</D:href><D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>",
                )?;
                for n in 0..5_000 {
                    out.write_all(support::mock::entry_xml("/big/", &format!("f{n:05}")).as_bytes())?;
                }
                out.flush()?;
                // The rest waits until the caller has seen the first rows.
                released
                    .recv_timeout(Duration::from_secs(8))
                    .map_err(|_| std::io::Error::other("the first batch never arrived"))?;
                for n in 5_000..6_000 {
                    out.write_all(support::mock::entry_xml("/big/", &format!("f{n:05}")).as_bytes())?;
                }
                out.write_all(b"</D:multistatus>")
            }));
        })
    });
    let provider = WebDavProvider::dav(config(None).with_options(options().with_batch(2_000)));
    let mut batches = Vec::new();
    let mut released_once = false;
    provider
        .list_batches(&at(&mock, "/big"), &CancelToken::new(), 0, &mut |batch| {
            batches.push(batch.len());
            if !released_once {
                released_once = true;
                release.send(()).unwrap();
            }
        })
        .unwrap();
    assert_eq!(batches.iter().sum::<usize>(), 6_000);
    assert_eq!(
        batches[0], 2_000,
        "rows are handed over in batches as they are read: {batches:?}"
    );
}

trait Tap: Sized {
    fn tap(self, f: impl FnOnce(&mut Self)) -> Self;
}

impl Tap for Reply {
    fn tap(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}

#[test]
fn a_listing_cut_short_is_an_error_and_a_cancelled_one_stops_at_once() {
    let mock = Mock::start(|_, n| {
        if n == 0 {
            let full = multistatus("/x/", (0..100).map(|n| format!("f{n}")));
            Reply::new(207).tap(|reply| {
                reply.body = Body::Truncated {
                    announced: full.len(),
                    sent: full.as_bytes()[..full.len() / 2].to_vec(),
                }
            })
        } else {
            Reply::new(207).tap(|reply| {
                reply.body = Body::Stream(Box::new(|out| {
                    out.write_all(b"<multistatus xmlns=\"DAV:\">")?;
                    out.flush()?;
                    std::thread::sleep(Duration::from_secs(8));
                    Ok(())
                }))
            })
        }
    });
    let provider = plain(None);
    let cut = list(&provider, &at(&mock, "/x"));
    assert!(
        matches!(
            cut,
            Err(VfsError::Disconnected { .. }) | Err(VfsError::Io { .. })
        ),
        "{cut:?}"
    );
    let cancel = CancelToken::new();
    let stopper = cancel.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        stopper.cancel();
    });
    let started = Instant::now();
    let result = provider.list(&at(&mock, "/y"), &cancel, 0, &mut |_| {});
    canceller.join().unwrap();
    assert!(matches!(result, Err(VfsError::Cancelled)), "{result:?}");
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[test]
fn a_server_that_stops_answering_times_out() {
    let mock = Mock::start(|_, _| {
        std::thread::sleep(Duration::from_secs(4));
        Reply::new(207)
    });
    let provider = WebDavProvider::dav(
        config(None).with_options(options().with_timeout(Duration::from_millis(500))),
    );
    let started = Instant::now();
    assert!(matches!(
        provider.stat(&at(&mock, "/x")),
        Err(VfsError::Timeout { .. })
    ));
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn a_read_that_breaks_off_continues_where_it_stopped_if_the_file_is_the_same() {
    let content: Vec<u8> = (0..200_000u32).map(|n| (n % 251) as u8).collect();
    let served = content.clone();
    let mock = Mock::start(move |seen, n| {
        if seen.method != "GET" {
            return Reply::new(500);
        }
        match n {
            0 => Reply::new(200).header("ETag", "\"v1\"").tap(|reply| {
                reply.body = Body::Truncated {
                    announced: served.len(),
                    sent: served[..70_000].to_vec(),
                }
            }),
            _ => {
                let range = seen.header("range").unwrap_or_default();
                assert!(
                    range.starts_with("bytes=") && range.ends_with('-'),
                    "{range}"
                );
                assert_eq!(seen.header("if-match"), Some("\"v1\""));
                let from: usize = range["bytes=".len()..range.len() - 1].parse().unwrap();
                Reply::new(206)
                    .header("ETag", "\"v1\"")
                    .header(
                        "Content-Range",
                        &format!("bytes {from}-{}/{}", served.len() - 1, served.len()),
                    )
                    .body(served[from..].to_vec())
            }
        }
    });
    let provider = plain(None);
    let mut got = Vec::new();
    provider
        .open_read(&at(&mock, "/data.bin"))
        .unwrap()
        .read_to_end(&mut got)
        .unwrap();
    assert!(got == content, "the bytes are the file's, each once");
    assert!(mock.requests().len() >= 2);

    // With a weak tag there is nothing to say the bytes are the same ones, so the failure shows.
    let served = content.clone();
    let weak = Mock::start(move |_, _| {
        Reply::new(200).header("ETag", "W/\"v1\"").tap(|reply| {
            reply.body = Body::Truncated {
                announced: served.len(),
                sent: served[..70_000].to_vec(),
            }
        })
    });
    let mut got = Vec::new();
    let result = provider
        .open_read(&at(&weak, "/data.bin"))
        .unwrap()
        .read_to_end(&mut got);
    assert!(result.is_err());
    assert_eq!(weak.requests().len(), 1);
}

#[test]
fn a_server_that_ignores_a_range_is_read_from_the_start_and_skipped() {
    let mock = Mock::start(|_, _| Reply::new(200).body(b"0123456789".to_vec()));
    let provider = plain(None);
    let mut got = Vec::new();
    provider
        .open_read_at(&at(&mock, "/f"), 4)
        .unwrap()
        .read_to_end(&mut got)
        .unwrap();
    assert_eq!(got, b"456789");
    assert_eq!(mock.requests()[0].header("range"), Some("bytes=4-"));
    assert_eq!(
        mock.requests()[0].header("accept-encoding"),
        Some("identity")
    );
}

fn put_mock() -> Mock {
    Mock::start(|seen, _| match seen.method.as_str() {
        "PROPFIND" => not_found(),
        "PUT" => Reply::new(201),
        _ => Reply::new(500),
    })
}

fn upload(provider: &WebDavProvider, path: &VfsPath, bytes: &[u8]) {
    let mut stream = provider
        .create_write(path, WriteOptions::exclusive())
        .unwrap();
    stream.write_all(bytes).unwrap();
    stream.finish(false).unwrap();
}

fn sha256(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn an_upload_carries_its_length_and_refuses_to_replace() {
    let mock = put_mock();
    let provider = plain(None);
    upload(&provider, &at(&mock, "/new file.txt"), b"hello");
    // Large enough to wait on disk.
    let big = vec![9u8; 6 * 1024 * 1024];
    upload(&provider, &at(&mock, "/big.bin"), &big);
    let puts: Vec<_> = mock
        .requests()
        .into_iter()
        .filter(|r| r.method == "PUT")
        .collect();
    assert_eq!(puts.len(), 2);
    assert_eq!(puts[0].target, "/new%20file.txt");
    assert_eq!(puts[0].header("if-none-match"), Some("*"));
    assert_eq!(puts[0].header("content-length"), Some("5"));
    assert!(puts[0].header("transfer-encoding").is_none());
    assert_eq!(puts[0].body, b"hello");
    assert_eq!(puts[1].body.len(), big.len());
    assert!(puts[1].body == big);
    assert!(puts
        .iter()
        .all(|r| r.header("x-oc-mtime").is_none() && r.header("oc-checksum").is_none()));
}

#[test]
fn an_upload_is_sent_again_whole_after_a_login_challenge() {
    let mock = Mock::start(
        |seen, _| match (seen.method.as_str(), seen.header("authorization")) {
            ("PROPFIND", _) => not_found(),
            (_, None) => Reply::new(401).header(
                "WWW-Authenticate",
                "Digest realm=\"r\", nonce=\"n\", qop=\"auth\"",
            ),
            _ => Reply::new(201),
        },
    );
    let provider = plain(Some(Logins::with(USER, PASSWORD)));
    let big = vec![5u8; 6 * 1024 * 1024];
    upload(&provider, &at(&mock, "/big.bin"), &big);
    let puts: Vec<_> = mock
        .requests()
        .into_iter()
        .filter(|r| r.method == "PUT")
        .collect();
    assert_eq!(puts.len(), 2);
    assert!(
        puts.iter().all(|r| r.body == big),
        "the body goes with each attempt"
    );
}

#[test]
fn nextcloud_gets_the_checksum_and_the_time_it_can_keep() {
    let mock = put_mock();
    let provider = plain(None);
    let hinted = at(&mock, "/remote.php/dav/files/alice/Report.txt");
    provider.hint_modified(&hinted, UNIX_EPOCH + Duration::from_secs(1_600_000_000));
    upload(&provider, &hinted, b"hello");
    // The same server with another dialect, chosen by the connection and not guessed from the path.
    let generic =
        WebDavProvider::dav(config(None).with_options(options().with_preset(Preset::Generic)));
    upload(
        &generic,
        &at(&mock, "/remote.php/dav/files/alice/Other.txt"),
        b"hello",
    );
    let forced =
        WebDavProvider::dav(config(None).with_options(options().with_preset(Preset::Nextcloud)));
    upload(&forced, &at(&mock, "/plain/Forced.txt"), b"hello");
    let puts: Vec<_> = mock
        .requests()
        .into_iter()
        .filter(|r| r.method == "PUT")
        .collect();
    assert_eq!(puts[0].header("x-oc-mtime"), Some("1600000000"));
    assert_eq!(
        puts[0].header("oc-checksum").map(str::to_owned),
        Some(format!("SHA256:{}", sha256(b"hello")))
    );
    assert!(puts[1].header("oc-checksum").is_none() && puts[1].header("x-oc-mtime").is_none());
    assert!(puts[2].header("oc-checksum").is_some() && puts[2].header("x-oc-mtime").is_none());
    // The hint was for that upload alone.
    upload(
        &provider,
        &at(&mock, "/remote.php/dav/files/alice/Next.txt"),
        b"x",
    );
    assert!(mock
        .requests()
        .last()
        .unwrap()
        .header("x-oc-mtime")
        .is_none());
}

#[test]
fn nextcloud_listings_ask_for_checksums_and_report_quota() {
    let mock = Mock::start(|seen, _| {
        let body = String::from_utf8_lossy(&seen.body).into_owned();
        if body.contains("quota-available-bytes") {
            Reply::xml(207, include_str!("fixtures/nextcloud-depth1.xml"))
        } else {
            Reply::xml(207, file_with_checksums())
        }
    });
    let provider = plain(None);
    let root = at(&mock, "/nextcloud/remote.php/dav/files/alice/");
    assert_eq!(
        provider.free_space(&root),
        Some(VolumeSpace {
            free_bytes: 1_073_693_611,
            total_bytes: 1_073_693_611 + 48_213
        })
    );
    let file = at(&mock, "/nextcloud/remote.php/dav/files/alice/Report.pdf");
    assert_eq!(
        provider.checksums(&file).unwrap().as_deref(),
        Some("SHA1:abc MD5:def")
    );
    let asked = mock.requests();
    assert!(String::from_utf8_lossy(&asked[1].body).contains("<oc:checksums/>"));
    // A server with no quota to report, or one that says "unlimited" with a negative number.
    let none = Mock::start(|_, _| {
        Reply::xml(
            207,
            "<multistatus xmlns=\"DAV:\"><response><href>/</href><propstat><prop><quota-available-bytes>-3</quota-available-bytes><quota-used-bytes>5</quota-used-bytes></prop><status>HTTP/1.1 200 OK</status></propstat></response></multistatus>",
        )
    });
    assert_eq!(provider.free_space(&at(&none, "/")), None);
}

fn file_with_checksums() -> String {
    "<d:multistatus xmlns:d=\"DAV:\" xmlns:oc=\"http://owncloud.org/ns\"><d:response>\
     <d:href>/nextcloud/remote.php/dav/files/alice/Report.pdf</d:href><d:propstat><d:prop>\
     <d:resourcetype/><d:getcontentlength>3</d:getcontentlength>\
     <oc:checksums><oc:checksum>SHA1:abc MD5:def</oc:checksum></oc:checksums></d:prop>\
     <d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"
        .to_owned()
}

#[test]
fn failures_of_writes_are_the_states_the_page_shows() {
    let mock = Mock::start(
        |seen, _| match (seen.method.as_str(), seen.target.as_str()) {
            ("PROPFIND", "/file") => Reply::xml(207, file_xml("/file")),
            ("PROPFIND", _) => not_found(),
            ("MKCOL", "/full/") => Reply::new(507),
            ("MKCOL", "/denied/") => Reply::new(403),
            ("DELETE", "/file") => Reply::new(423),
            ("PUT", "/quota") => Reply::new(507),
            ("PUT", "/big") => Reply::new(413),
            ("MOVE", _) => Reply::new(403),
            _ => Reply::new(500),
        },
    );
    let provider = plain(None);
    assert!(matches!(
        provider.create_dir(&at(&mock, "/full")),
        Err(VfsError::StorageFull { .. })
    ));
    assert!(matches!(
        provider.create_dir(&at(&mock, "/denied")),
        Err(VfsError::PermissionDenied { .. })
    ));
    assert!(matches!(
        provider.remove_file(&at(&mock, "/file")),
        Err(VfsError::InUse { .. })
    ));
    assert!(matches!(
        provider.rename(&at(&mock, "/file"), &at(&mock, "/to"), false),
        Err(VfsError::PermissionDenied { .. })
    ));
    let mut stream = provider
        .create_write(&at(&mock, "/quota"), WriteOptions::exclusive())
        .unwrap();
    stream.write_all(b"x").unwrap();
    assert!(matches!(
        stream.finish(false),
        Err(VfsError::StorageFull { .. })
    ));
    let mut stream = provider
        .create_write(&at(&mock, "/big"), WriteOptions::exclusive())
        .unwrap();
    stream.write_all(b"x").unwrap();
    assert!(matches!(stream.finish(false), Err(VfsError::Io { .. })));
    // A name no server could hold is refused before anything is sent.
    let before = mock.requests().len();
    let long = at(&mock, &format!("/{}", "x".repeat(300)));
    assert!(matches!(
        provider.create_dir(&long),
        Err(VfsError::InvalidName { .. })
    ));
    assert_eq!(mock.requests().len(), before);
}

#[test]
fn a_delete_that_fails_inside_what_it_removes_is_an_error() {
    let mock = Mock::start(|seen, _| match seen.method.as_str() {
        "PROPFIND" if seen.header("depth") == Some("0") => Reply::xml(207, file_xml("/file")),
        "DELETE" => Reply::xml(207, "<multistatus xmlns=\"DAV:\"/>"),
        _ => Reply::new(500),
    });
    let provider = plain(None);
    assert!(matches!(
        provider.remove_file(&at(&mock, "/file")),
        Err(VfsError::Io { .. })
    ));
}

#[test]
fn a_server_without_copy_leaves_the_caller_to_copy() {
    let mock = Mock::start(|seen, _| match seen.method.as_str() {
        "PROPFIND" => Reply::xml(207, file_xml("/a")),
        "COPY" => Reply::new(501),
        _ => Reply::new(500),
    });
    let provider = plain(None);
    let result = provider.copy_file_within(
        &at(&mock, "/a"),
        &at(&mock, "/b"),
        &mut |_| {},
        &CancelToken::new(),
    );
    assert!(result.is_none());
    let copy = mock
        .requests()
        .into_iter()
        .find(|r| r.method == "COPY")
        .unwrap();
    assert_eq!(copy.header("overwrite"), Some("F"));
    assert_eq!(
        copy.header("destination"),
        Some(format!("http://127.0.0.1:{}/b", mock.port).as_str())
    );
    // Another connection is not this fast path's either.
    let other = VfsPath::from_uri("dav://elsewhere.invalid/b").unwrap();
    assert!(provider
        .copy_file_within(&at(&mock, "/a"), &other, &mut |_| {}, &CancelToken::new())
        .is_none());
}

#[test]
fn connection_state_follows_what_the_server_did() {
    let mock = Mock::start(|seen, n| match n {
        0 => ok_listing(seen),
        _ => Reply::new(200),
    });
    let provider = plain(None);
    let p = at(&mock, "/x");
    let key = p.connection_key().unwrap();
    assert_eq!(provider.connection_state(&key), ConnectionState::Idle);
    list(&provider, &p).unwrap();
    assert_eq!(provider.connection_state(&key), ConnectionState::Connected);
    provider.connect(&key, None, &CancelToken::new()).unwrap();
    assert_eq!(provider.connection_state(&key), ConnectionState::Connected);
    provider.disconnect(&key);
    assert_eq!(provider.connection_state(&key), ConnectionState::Idle);
    list(&provider, &p).unwrap_or_default();
    drop(mock);
}

#[test]
fn the_provider_serves_its_own_scheme_only() {
    let provider = plain(None);
    assert_eq!(provider.scheme(), "dav");
    assert_eq!(WebDavProvider::davs(WebDavConfig::new()).scheme(), "davs");
    let other = VfsPath::from_uri("davs://h/x").unwrap();
    assert!(matches!(
        provider.stat(&other),
        Err(VfsError::Unsupported { .. })
    ));
    assert!(provider.connection_key(&other).is_none());
    let sftp = VfsPath::from_uri("sftp://h/x").unwrap();
    assert!(matches!(
        provider.stat(&sftp),
        Err(VfsError::Unsupported { .. })
    ));
}
