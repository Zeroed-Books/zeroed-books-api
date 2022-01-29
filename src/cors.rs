use rocket::{
    fairing::{Fairing, Info, Kind},
    http::{Header, Method, Status},
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

    async fn on_response<'r>(&self, req: &'r Request<'_>, response: &mut Response<'r>) {
        if response.status() == Status::NotFound && req.method() == Method::Options {
            response.set_status(Status::Ok);
        }

        // Credentialed requests require the allowed origin to match the request
        // origin.
        response.set_header(Header::new(
            "Access-Control-Allow-Origin",
            req.headers().get_one("Origin").unwrap_or("*").to_owned(),
        ));
        response.set_header(Header::new(
            "Access-Control-Allow-Methods",
            "GET, OPTIONS, POST, PUT",
        ));
        response.set_header(Header::new(
            "Access-Control-Allow-Headers",
            "Accept, Content-Type, Origin",
        ));
        response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
    }
}
