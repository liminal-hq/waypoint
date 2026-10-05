// Reading a `207 Multi-Status` body as it streams: one `Multi` per `response`, handed over as soon
// as its end tag is read, so a 100 000-entry folder never sits in memory as a document.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::BufRead;

use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use waypoint_vfs::CancelToken;

const DAV: &str = "DAV:";
const OWNCLOUD: &str = "http://owncloud.org/ns";

/// What the properties of one `response` said, for the ones that came back with a success status.
/// A property a server left out, or answered `404` for, stays `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Props {
    /// `Some(true)` when `resourcetype` holds `collection`, `Some(false)` when it is there and
    /// does not; `None` when the server did not say.
    pub collection: Option<bool>,
    pub length: Option<u64>,
    /// `getlastmodified`, as the server wrote it.
    pub modified: Option<String>,
    pub etag: Option<String>,
    pub content_type: Option<String>,
    pub quota_available: Option<u64>,
    pub quota_used: Option<u64>,
    /// Nextcloud's `oc:checksums`, such as `SHA1:abc MD5:def`.
    pub checksums: Option<String>,
}

/// One `response` of a multistatus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Multi {
    pub href: String,
    /// The status of the response as a whole, when it has one instead of `propstat`s.
    pub status: Option<u16>,
    pub props: Props,
}

/// Why a body could not be read as a multistatus.
#[derive(Debug)]
pub(crate) enum XmlError {
    /// The XML is malformed or ends early.
    Malformed(String),
    /// The caller stopped the parse.
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prop {
    ContentLength,
    LastModified,
    ETag,
    ContentType,
    ResourceType,
    QuotaAvailable,
    QuotaUsed,
    Checksums,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ctx {
    Outside,
    Response,
    Href,
    ResponseStatus,
    Propstat,
    PropstatStatus,
    Prop,
    /// A property element, or something nested in one, whose text belongs to the property.
    Value(Prop),
    Other,
}

#[derive(Default)]
struct State {
    stack: Vec<Ctx>,
    href: String,
    status: String,
    propstat_status: String,
    props: Props,
    propstat_props: Props,
    value: String,
    resourcetype_seen: bool,
    collection: bool,
}

fn status_code(text: &str) -> Option<u16> {
    text.split_whitespace().nth(1)?.parse().ok()
}

fn kind(ns: &ResolveResult, local: &str) -> Prop {
    let dav = matches!(ns, ResolveResult::Bound(n) if n.as_ref() == DAV);
    let oc = matches!(ns, ResolveResult::Bound(n) if n.as_ref() == OWNCLOUD);
    match (dav, oc, local) {
        (true, _, "getcontentlength") => Prop::ContentLength,
        (true, _, "getlastmodified") => Prop::LastModified,
        (true, _, "getetag") => Prop::ETag,
        (true, _, "getcontenttype") => Prop::ContentType,
        (true, _, "resourcetype") => Prop::ResourceType,
        (true, _, "quota-available-bytes") => Prop::QuotaAvailable,
        (true, _, "quota-used-bytes") => Prop::QuotaUsed,
        (_, true, "checksums") => Prop::Checksums,
        _ => Prop::Ignored,
    }
}

impl State {
    fn child(&self, ns: &ResolveResult, local: &str) -> Ctx {
        let parent = self.stack.last().copied().unwrap_or(Ctx::Outside);
        let dav = matches!(ns, ResolveResult::Bound(n) if n.as_ref() == DAV);
        match (parent, dav, local) {
            (Ctx::Outside, true, "response") => Ctx::Response,
            (Ctx::Outside, _, _) => Ctx::Outside,
            (Ctx::Response, true, "href") => Ctx::Href,
            (Ctx::Response, true, "status") => Ctx::ResponseStatus,
            (Ctx::Response, true, "propstat") => Ctx::Propstat,
            (Ctx::Propstat, true, "prop") => Ctx::Prop,
            (Ctx::Propstat, true, "status") => Ctx::PropstatStatus,
            (Ctx::Prop, _, _) => Ctx::Value(kind(ns, local)),
            (Ctx::Value(Prop::ResourceType), true, "collection") => Ctx::Other,
            (Ctx::Value(prop), _, _) => Ctx::Value(prop),
            _ => Ctx::Other,
        }
    }

    fn start(&mut self, ns: &ResolveResult, local: &str) {
        let ctx = self.child(ns, local);
        let parent = self.stack.last().copied();
        match ctx {
            Ctx::Response => self.reset_response(),
            Ctx::Propstat => {
                self.propstat_props = Props::default();
                self.propstat_status.clear();
                self.resourcetype_seen = false;
                self.collection = false;
            }
            Ctx::Href => self.href.clear(),
            Ctx::ResponseStatus => self.status.clear(),
            Ctx::PropstatStatus => self.propstat_status.clear(),
            Ctx::Value(prop) if parent == Some(Ctx::Prop) => {
                self.value.clear();
                if prop == Prop::ResourceType {
                    self.resourcetype_seen = true;
                }
            }
            _ => {}
        }
        if matches!(parent, Some(Ctx::Value(Prop::ResourceType)))
            && matches!(ns, ResolveResult::Bound(n) if n.as_ref() == DAV)
            && local == "collection"
        {
            self.collection = true;
        }
        self.stack.push(ctx);
    }

    fn reset_response(&mut self) {
        self.href.clear();
        self.status.clear();
        self.props = Props::default();
    }

    fn text(&mut self, text: &str) {
        match self.stack.last() {
            Some(Ctx::Href) => self.href.push_str(text),
            Some(Ctx::ResponseStatus) => self.status.push_str(text),
            Some(Ctx::PropstatStatus) => self.propstat_status.push_str(text),
            Some(Ctx::Value(_)) => self.value.push_str(text),
            _ => {}
        }
    }

    /// Ends the current element; returns the finished response when it was one.
    fn end(&mut self) -> Option<Multi> {
        let ctx = self.stack.pop()?;
        match ctx {
            Ctx::Value(prop) if self.stack.last() == Some(&Ctx::Prop) => {
                self.commit(prop);
                None
            }
            Ctx::Propstat => {
                let ok = status_code(&self.propstat_status).is_none_or(|c| (200..300).contains(&c));
                if ok {
                    let mut got = std::mem::take(&mut self.propstat_props);
                    if self.resourcetype_seen {
                        got.collection = Some(self.collection);
                    }
                    macro_rules! merge {
                        ($($field:ident),*) => { $( if got.$field.is_some() { self.props.$field = got.$field; } )* };
                    }
                    merge!(
                        collection,
                        length,
                        modified,
                        etag,
                        content_type,
                        quota_available,
                        quota_used,
                        checksums
                    );
                }
                None
            }
            Ctx::Response => Some(Multi {
                href: self.href.trim().to_owned(),
                status: status_code(&self.status),
                props: std::mem::take(&mut self.props),
            }),
            _ => None,
        }
    }

    fn commit(&mut self, prop: Prop) {
        let value = self.value.trim();
        let props = &mut self.propstat_props;
        match prop {
            Prop::ContentLength => props.length = value.parse().ok(),
            Prop::LastModified => props.modified = non_empty(value),
            Prop::ETag => props.etag = non_empty(value),
            Prop::ContentType => props.content_type = non_empty(value),
            Prop::QuotaAvailable => props.quota_available = value.parse().ok(),
            Prop::QuotaUsed => props.quota_used = value.parse().ok(),
            Prop::Checksums => props.checksums = non_empty(value),
            Prop::ResourceType | Prop::Ignored => {}
        }
    }
}

fn non_empty(text: &str) -> Option<String> {
    (!text.is_empty()).then(|| text.to_owned())
}

fn entity(name: &str, out: &mut String) {
    match name {
        "lt" => out.push('<'),
        "gt" => out.push('>'),
        "amp" => out.push('&'),
        "quot" => out.push('"'),
        "apos" => out.push('\''),
        _ => {}
    }
}

/// Reads `reader` as a multistatus and hands each `response` to `on`, which returns whether to
/// go on. `cancel` is checked at each response.
pub(crate) fn parse_multistatus<R: BufRead>(
    reader: R,
    cancel: &CancelToken,
    on: &mut dyn FnMut(Multi) -> bool,
) -> Result<(), XmlError> {
    let mut xml = NsReader::from_reader(reader);
    let mut buf = Vec::new();
    let mut state = State::default();
    let malformed = |error: &dyn std::fmt::Display| XmlError::Malformed(error.to_string());
    let mut saw_root = false;
    loop {
        buf.clear();
        let (ns, event) = xml
            .read_resolved_event_into(&mut buf)
            .map_err(|e| malformed(&e))?;
        match event {
            Event::Start(start) => {
                saw_root = true;
                state.start(&ns, start_local(&start));
            }
            Event::Empty(start) => {
                saw_root = true;
                state.start(&ns, start_local(&start));
                if let Some(multi) = state.end() {
                    if !on(multi) {
                        return Err(XmlError::Stopped);
                    }
                }
            }
            Event::End(_) => {
                if let Some(multi) = state.end() {
                    if cancel.is_cancelled() || !on(multi) {
                        return Err(XmlError::Stopped);
                    }
                }
            }
            Event::Text(text) => state.text(&text.xml10_content()),
            Event::CData(data) => {
                state.text(&data.into_inner());
            }
            Event::GeneralRef(reference) => {
                let mut out = String::new();
                match reference.resolve_char_ref() {
                    Ok(Some(c)) => out.push(c),
                    Ok(None) => entity(&reference.xml10_content(), &mut out),
                    Err(_) => {}
                }
                state.text(&out);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !saw_root || !state.stack.is_empty() {
        return Err(XmlError::Malformed("the body ends early".to_owned()));
    }
    Ok(())
}

fn start_local<'a>(start: &'a BytesStart<'_>) -> &'a str {
    start.local_name().into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(xml: &str) -> Vec<Multi> {
        let mut all = Vec::new();
        parse_multistatus(xml.as_bytes(), &CancelToken::new(), &mut |multi| {
            all.push(multi);
            true
        })
        .unwrap();
        all
    }

    fn by_href<'a>(all: &'a [Multi], href: &str) -> &'a Multi {
        all.iter()
            .find(|multi| multi.href == href)
            .unwrap_or_else(|| panic!("no response for {href}: {all:?}"))
    }

    // Recorded from Apache 2.4.68 `mod_dav` (`Dav On`), answering the request this provider sends.
    #[test]
    fn apache_mod_dav_lists_folders_and_files() {
        let all = parse(include_str!("../tests/fixtures/apache-mod_dav-depth1.xml"));
        assert_eq!(all.len(), 8);
        let folder = by_href(&all, "/dav/sub/");
        assert_eq!(folder.props.collection, Some(true));
        assert_eq!(folder.props.length, None, "the 404 propstat gives nothing");
        assert_eq!(folder.props.etag.as_deref(), Some("\"12-65d054c25f6b0\""));
        let file = by_href(&all, "/dav/a%20file.txt");
        assert_eq!(file.props.collection, Some(false));
        assert_eq!(file.props.length, Some(5));
        assert_eq!(
            file.props.modified.as_deref(),
            Some("Sun, 04 Oct 2026 15:19:54 GMT")
        );
        assert_eq!(file.props.content_type, None);
        // A name that holds an escape is escaped again in its href.
        by_href(&all, "/dav/caf%25C3%25A9%20100%25.dat");
        by_href(&all, "/dav/semi;colon,comma");
        let one = parse(include_str!(
            "../tests/fixtures/apache-mod_dav-depth0-file.xml"
        ));
        assert_eq!((one.len(), one[0].props.length), (1, Some(5)));
    }

    // Recorded from rclone 1.75 `serve webdav`, which writes empty elements as open and close tags
    // and answers `404` propstats with empty values.
    #[test]
    fn rclone_serve_webdav_answers_with_prefixed_empty_values() {
        let all = parse(include_str!("../tests/fixtures/rclone-depth1.xml"));
        let root = by_href(&all, "/");
        assert_eq!(root.props.collection, Some(true));
        assert_eq!(root.props.etag, None);
        let file = by_href(&all, "/a%20file.txt");
        assert_eq!(file.props.length, Some(5));
        assert_eq!(
            file.props.content_type.as_deref(),
            Some("text/plain; charset=utf-8")
        );
        assert_eq!(file.props.collection, Some(false));
        by_href(&all, "/caf%25C3%25A9%20100%25.dat");
        assert_eq!(by_href(&all, "/empty.bin").props.length, Some(0));
    }

    // Written from the output format of `nginx-dav-ext` (nginx has no `PROPFIND` of its own): no
    // `getetag`, no `getcontentlength` for folders, and an entry with no properties at all. Not a
    // recording.
    #[test]
    fn nginx_dav_ext_omits_properties() {
        let all = parse(include_str!("../tests/fixtures/nginx-dav-ext-depth1.xml"));
        assert_eq!(all.len(), 4);
        let file = by_href(&all, "/files/caf%C3%A9.txt");
        assert_eq!(
            (file.props.length, file.props.etag.clone()),
            (Some(1500), None)
        );
        assert_eq!(by_href(&all, "/files/docs/").props.collection, Some(true));
        assert_eq!(
            by_href(&all, "/files/no%20properties.bin").props,
            Props::default()
        );
    }

    // Written from Nextcloud's documented `PROPFIND` output (Sabre/DAV with the `oc:` and `nc:`
    // namespaces, entity-escaped etags, and `checksums` nested two deep); not a recording.
    #[test]
    fn nextcloud_gives_quota_checksums_and_escaped_etags() {
        let all = parse(include_str!("../tests/fixtures/nextcloud-depth1.xml"));
        assert_eq!(all.len(), 4);
        let root = by_href(&all, "/nextcloud/remote.php/dav/files/alice/");
        assert_eq!(root.props.quota_available, Some(1_073_693_611));
        assert_eq!(root.props.quota_used, Some(48_213));
        assert_eq!(root.props.etag.as_deref(), Some("\"651a8c5e4b2f1\""));
        let report = by_href(
            &all,
            "/nextcloud/remote.php/dav/files/alice/Report%20%231%20%28final%29.pdf",
        );
        assert_eq!(report.props.length, Some(48_213));
        assert_eq!(
            report.props.checksums.as_deref(),
            Some("SHA1:0f1d2c3b4a5968778695a4b3c2d1e0f102132435 MD5:00112233445566778899aabbccddeeff")
        );
        assert_eq!(
            by_href(
                &all,
                "/nextcloud/remote.php/dav/files/alice/Fish%20%26%20Chips.txt"
            )
            .props
            .checksums,
            None
        );
    }

    #[test]
    fn a_status_beside_the_response_and_entities_in_values_are_read() {
        let all = parse(
            r#"<multistatus xmlns="DAV:"><response><href>/gone</href><status>HTTP/1.1 404 Not Found</status></response>
               <response><href>http://h:8080/a&amp;b</href><propstat><prop><getetag>&#34;x&#x22;&lt;</getetag>
               <getcontentlength><![CDATA[ 12 ]]></getcontentlength></prop><status>HTTP/1.1 200 OK</status></propstat></response></multistatus>"#,
        );
        assert_eq!(all[0].status, Some(404));
        assert_eq!(all[1].href, "http://h:8080/a&b");
        assert_eq!(all[1].props.etag.as_deref(), Some("\"x\"<"));
        assert_eq!(all[1].props.length, Some(12));
    }

    #[test]
    fn any_prefix_or_default_namespace_for_dav_reads() {
        let all = parse(
            r#"<ns0:multistatus xmlns:ns0="DAV:"><ns0:response><ns0:href>/x/</ns0:href>
               <ns0:propstat><ns0:prop><ns0:resourcetype><ns0:collection/></ns0:resourcetype></ns0:prop>
               <ns0:status>HTTP/1.1 200 OK</ns0:status></ns0:propstat></ns0:response></ns0:multistatus>"#,
        );
        assert_eq!(all[0].props.collection, Some(true));
        // The same names in another namespace are not DAV's.
        let other = parse(
            r#"<multistatus xmlns="DAV:" xmlns:x="urn:other"><response><href>/y</href><propstat><prop>
               <x:getcontentlength>9</x:getcontentlength></prop><status>HTTP/1.1 200 OK</status></propstat></response></multistatus>"#,
        );
        assert_eq!(other[0].props.length, None);
    }

    #[test]
    fn truncated_and_empty_bodies_are_errors_and_cancelling_stops() {
        let none = CancelToken::new();
        for xml in [
            "",
            "<multistatus xmlns=\"DAV:\"><response><href>/a",
            "<multistatus xmlns=\"DAV:\"><response></multistatus>",
        ] {
            assert!(
                matches!(
                    parse_multistatus(xml.as_bytes(), &none, &mut |_| true),
                    Err(XmlError::Malformed(_))
                ),
                "{xml:?}"
            );
        }
        let cancel = CancelToken::new();
        cancel.cancel();
        let xml = include_str!("../tests/fixtures/nginx-dav-ext-depth1.xml");
        assert!(matches!(
            parse_multistatus(xml.as_bytes(), &cancel, &mut |_| true),
            Err(XmlError::Stopped)
        ));
        let mut seen = 0;
        assert!(matches!(
            parse_multistatus(xml.as_bytes(), &none, &mut |_| {
                seen += 1;
                false
            }),
            Err(XmlError::Stopped)
        ));
        assert_eq!(seen, 1);
    }
}
