use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, Responder,
    http::header::{HeaderName, HeaderValue},
    web,
};
use reqwest::{Client, Method};
use std::str::FromStr;
use url::Url;

// API 映射配置
const API_MAPPING: &[(&str, &str)] = &[
    ("/discord", "https://discord.com/api"),
    ("/telegram", "https://api.telegram.org"),
    ("/openai", "https://api.openai.com"),
    ("/claude", "https://api.anthropic.com"),
    ("/gemini", "https://generativelanguage.googleapis.com"),
    ("/meta", "https://www.meta.ai/api"),
    ("/groq", "https://api.groq.com/openai"),
    ("/xai", "https://api.x.ai"),
    ("/cohere", "https://api.cohere.ai"),
    ("/huggingface", "https://api-inference.huggingface.co"),
    ("/together", "https://api.together.xyz"),
    ("/novita", "https://api.novita.ai"),
    ("/portkey", "https://api.portkey.ai"),
    ("/fireworks", "https://api.fireworks.ai"),
    ("/openrouter", "https://openrouter.ai/api"),
    ("/cerebras", "https://api.cerebras.ai"),
    ("/test", "http://127.0.0.1:8078"),
];

// 允许转发的请求头
const ALLOWED_HEADERS: &[&str] = &[
    "accept",
    "content-type",
    "authorization",
    "x-goog-api-key",
    "x-api-key",
];

// 生成 HTML 内容
fn generate_html_content() -> String {
    let links_html: String = API_MAPPING
        .iter()
        .map(|(path, url)| format!(r#"<li><a href="{}">{}</a></li>"#, url, path))
        .collect::<Vec<_>>()
        .join("\n      ");

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Service is Running</title>
    <style>
        body {{
            font-family: Arial, sans-serif;
            max-width: 800px;
            margin: 0 auto;
            padding: 20px;
            line-height: 1.6;
        }}
        h1 {{
            color: #333;
            border-bottom: 2px solid #eee;
            padding-bottom: 10px;
        }}
        ul {{
            list-style-type: none;
            padding: 0;
        }}
        li {{
            margin: 10px 0;
            padding: 10px;
            background: #f9f9f9;
            border-radius: 5px;
            border-left: 4px solid #007acc;
        }}
        a {{
            text-decoration: none;
            color: #007acc;
            font-weight: bold;
        }}
        a:hover {{
            color: #005a9e;
            text-decoration: underline;
        }}
    </style>
</head>
<body>
    <h1>Service is running!</h1>
    <p>Available API endpoints:</p>
    <ul>
        {}
    </ul>
    <footer>
        <p><small>Generated dynamically at request time</small></p>
    </footer>
</body>
</html>"#,
        links_html
    )
}

// 提取路径前缀和剩余部分
fn extract_prefix_and_rest(pathname: &str) -> Option<(&'static str, &str)> {
    for (prefix, _) in API_MAPPING {
        if pathname.starts_with(prefix) {
            let rest = &pathname[prefix.len()..];
            return Some((prefix, rest));
        }
    }
    None
}

// 根路径处理器
async fn root() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(generate_html_content())
}

// robots.txt 处理器
async fn robots() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("User-agent: *\nDisallow: /")
}

// 代理请求处理器
async fn proxy_request(req: HttpRequest, body: web::Bytes) -> impl Responder {
    let path = req.path();

    // 提取前缀和剩余路径
    let (prefix, rest_path) = match extract_prefix_and_rest(path) {
        Some((prefix, rest)) => (prefix, rest),
        None => {
            return HttpResponse::NotFound().body("Not Found");
        }
    };

    // 构建目标 URL
    let base_url = API_MAPPING
        .iter()
        .find(|(p, _)| *p == prefix)
        .map(|(_, url)| *url)
        .unwrap();

    let target_url = match Url::parse(&format!("{}{}", base_url, rest_path)) {
        Ok(url) => url,
        Err(_) => {
            return HttpResponse::BadRequest().body("Invalid URL");
        }
    };

    // 创建 HTTP 客户端
    let client = Client::new();

    // 构建请求方法
    let method = match req.method().as_str() {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "DELETE" => Method::DELETE,
        "PATCH" => Method::PATCH,
        "OPTIONS" => Method::OPTIONS,
        _ => Method::GET,
    };

    // 构建请求
    let mut request_builder = client.request(method, target_url.as_str());

    // 添加允许的请求头
    for header_name in ALLOWED_HEADERS {
        if let Some(header_value) = req.headers().get(*header_name) {
            if let Ok(value_str) = header_value.to_str() {
                request_builder = request_builder.header(*header_name, value_str);
            }
        }
    }

    // 发送请求
    match request_builder
        .body(body.to_vec())
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();

            // 转换状态码
            let actix_status = match actix_web::http::StatusCode::from_u16(status.as_u16()) {
                Ok(status) => status,
                Err(_) => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            };

            let mut client_resp = HttpResponse::build(actix_status);

            // 复制响应头
            for (name, value) in response.headers() {
                if let Ok(value_str) = value.to_str() {
                    if let Ok(header_name) = HeaderName::from_str(name.as_str()) {
                        if let Ok(header_value) = HeaderValue::from_str(value_str) {
                            client_resp.insert_header((header_name, header_value));
                        }
                    }
                }
            }

            // 添加安全头
            client_resp
                .insert_header(("X-Content-Type-Options", "nosniff"))
                .insert_header(("X-Frame-Options", "DENY"))
                .insert_header(("Referrer-Policy", "no-referrer"));

            // 返回响应体
            match response.bytes().await {
                Ok(bytes) => client_resp.body(bytes),
                Err(_) => HttpResponse::InternalServerError().body("Failed to read response body"),
            }
        }
        Err(e) => {
            eprintln!("Failed to fetch: {}", e);
            if e.is_timeout() {
                HttpResponse::GatewayTimeout().body("Gateway Timeout")
            } else {
                HttpResponse::InternalServerError().body("Internal Server Error")
            }
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting server on 0.0.0.0:8000");

    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(root))
            .route("/index.html", web::get().to(root))
            .route("/robots.txt", web::get().to(robots))
            .default_service(web::route().to(proxy_request))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
