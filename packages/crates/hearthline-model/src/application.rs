use core::net::Ipv4Addr;

use crate::{ServiceKind, Text};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpDocument {
    pub title: Text<96>,
    pub heading: Text<128>,
    pub body: Text<256>,
}

// Fixed text buffers intentionally keep packet data allocator-free.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationData {
    None,
    DnsQuery {
        name: Text<128>,
    },
    DnsAnswer {
        name: Text<128>,
        address: Option<Ipv4Addr>,
    },
    HttpRequest {
        method: HttpMethod,
        host: Text<128>,
        path: Text<192>,
        body: Option<Text<256>>,
        body_bytes: usize,
    },
    HttpResponse {
        status: u16,
        document: Option<HttpDocument>,
    },
    Service(ServiceKind),
}
