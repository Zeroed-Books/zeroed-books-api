use rocket::{
    fairing::{Fairing, Info, Kind},
    http::Header,
    Request, Response,
};

/// A fairing that attaches CORS headers to all responses.
pub struct CorsHeaders;

#[rocket::async_trait]
impl Fairing for CorsHeaders {
    fn info(&self) -> Info {
        Info {
            name: "CORS Headers",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, _req: &'r Request<'_>, response: &mut Response<'r>) {
        response.set_header(Header::new("Access-Control-Allow-Origin", "*"));
        response.set_header(Header::new(
            "Access-Control-Allow-Methods",
            "GET, OPTIONS, POST, PUT",
        ));
        response.set_header(Header::new("Access-Control-Allow-Headers", "*"));
        response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
    }
}
