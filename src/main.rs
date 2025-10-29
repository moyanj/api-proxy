use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, Responder,
    http::header::{HeaderName, HeaderValue},
    web,
};
use once_cell::sync::Lazy;
use reqwest::{Client, Method};
use std::{collections::HashMap, str::FromStr, time::Duration};
use url::Url;

// API 映射配置 - 使用 HashMap 提高查找性能
static API_MAPPING: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert("/discord", "https://discord.com/api");
    map.insert("/telegram", "https://api.telegram.org");
    map.insert("/openai", "https://api.openai.com");
    map.insert("/claude", "https://api.anthropic.com");
    map.insert("/gemini", "https://generativelanguage.googleapis.com");
    map.insert("/meta", "https://www.meta.ai/api");
    map.insert("/groq", "https://api.groq.com/openai");
    map.insert("/xai", "https://api.x.ai");
    map.insert("/cohere", "https://api.cohere.ai");
    map.insert("/huggingface", "https://api-inference.huggingface.co");
    map.insert("/together", "https://api.together.xyz");
    map.insert("/novita", "https://api.novita.ai");
    map.insert("/portkey", "https://api.portkey.ai");
    map.insert("/fireworks", "https://api.fireworks.ai");
    map.insert("/openrouter", "https://openrouter.ai/api");
    map.insert("/cerebras", "https://api.cerebras.ai");
    map.insert("/test", "http://127.0.0.1:8078");
    map
});

// 允许转发的请求头 - 使用 HashSet 提高查找性能
static ALLOWED_HEADERS: Lazy<std::collections::HashSet<&'static str>> = Lazy::new(|| {
    [
        "accept",
        "content-type",
        "authorization",
        "x-goog-api-key",
        "x-api-key",
        "user-agent",
        "cache-control",
    ]
    .iter()
    .cloned()
    .collect()
});

// 预先生成的 HTML 内容
static HTML_CONTENT: Lazy<String> = Lazy::new(generate_html_content);

// 自定义错误类型
#[derive(Debug)]
enum ProxyError {
    InvalidUrl,
    ReqwestError(reqwest::Error),
    HeaderError,
}

impl std::fmt::Display for ProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxyError::InvalidUrl => write!(f, "Invalid URL"),
            ProxyError::ReqwestError(e) => write!(f, "Request error: {}", e),
            ProxyError::HeaderError => write!(f, "Header processing error"),
        }
    }
}

impl From<reqwest::Error> for ProxyError {
    fn from(err: reqwest::Error) -> Self {
        ProxyError::ReqwestError(err)
    }
}

// 生成 HTML 内容
fn generate_html_content() -> String {
    let links_html: String = API_MAPPING
        .iter()
        .map(|(path, url)| format!(r#"<li><a href="{}">{}</a> → {}</li>"#, path, path, url))
        .collect::<Vec<_>>()
        .join("\n      ");

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>API Proxy Service</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
            line-height: 1.6;
            background: #f5f5f5;
        }}
        .container {{
            background: white;
            border-radius: 8px;
            padding: 30px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }}
        h1 {{
            color: #333;
            border-bottom: 2px solid #007acc;
            padding-bottom: 10px;
            margin-top: 0;
        }}
        ul {{
            list-style-type: none;
            padding: 0;
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
            gap: 10px;
        }}
        li {{
            margin: 5px 0;
            padding: 15px;
            background: #f8f9fa;
            border-radius: 5px;
            border-left: 4px solid #007acc;
            transition: transform 0.2s;
        }}
        li:hover {{
            transform: translateX(5px);
            background: #e9ecef;
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
        .url {{
            color: #666;
            font-size: 0.9em;
            display: block;
            margin-top: 5px;
        }}
        footer {{
            margin-top: 30px;
            text-align: center;
            color: #666;
            font-size: 0.9em;
        }}
        @media (max-width: 768px) {{
            ul {{
                grid-template-columns: 1fr;
            }}
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🚀 API Proxy Service</h1>
        <p>Available API endpoints:</p>
        <ul>
            {}
        </ul>
        <footer>
            <p><small>Service is running smoothly! • Built with Rust & Actix Web</small></p>
        </footer>
    </div>
</body>
</html>"#,
        links_html
    )
}

// 提取路径前缀和剩余部分 - 优化性能
fn extract_prefix_and_rest(pathname: &str) -> Option<(&'static str, &str)> {
    // 按长度降序排序，优先匹配更长的路径
    let mut sorted_paths: Vec<&&str> = API_MAPPING.keys().collect();
    sorted_paths.sort_by(|a, b| b.len().cmp(&a.len()));

    for &prefix in sorted_paths {
        if pathname.starts_with(prefix) {
            let rest = &pathname[prefix.len()..];
            return Some((prefix, rest));
        }
    }
    None
}

// 创建 HTTP 客户端 - 使用连接池和超时配置
fn create_http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .tcp_keepalive(Duration::from_secs(60))
        .pool_max_idle_per_host(20)
        .build()
        .expect("Failed to create HTTP client")
}

// 根路径处理器
async fn root() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(HTML_CONTENT.as_str())
}

// robots.txt 处理器
async fn robots() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("User-agent: *\nDisallow: /")
}

// 构建目标 URL
fn build_target_url(prefix: &str, rest_path: &str) -> Result<Url, ProxyError> {
    let base_url = API_MAPPING.get(prefix).ok_or(ProxyError::InvalidUrl)?;

    let full_url = if rest_path.is_empty() {
        base_url.to_string()
    } else {
        format!("{}{}", base_url, rest_path)
    };

    Url::parse(&full_url).map_err(|_| ProxyError::InvalidUrl)
}

// 处理请求头 - 现在返回 Reqwest 的 header 类型
fn process_headers(
    req: &HttpRequest,
) -> Vec<(reqwest::header::HeaderName, reqwest::header::HeaderValue)> {
    req.headers()
        .iter()
        .filter(|(name, _)| ALLOWED_HEADERS.contains(name.as_str().to_lowercase().as_str()))
        .filter_map(|(name, value)| {
            // 将 Actix Web 的 HeaderName 转换为字符串，然后创建 Reqwest 的 HeaderName
            let header_name_str = name.as_str();
            let value_str = match value.to_str() {
                Ok(s) => s,
                Err(_) => return None,
            };

            match (
                reqwest::header::HeaderName::from_str(header_name_str),
                reqwest::header::HeaderValue::from_str(value_str),
            ) {
                (Ok(header_name), Ok(header_value)) => Some((header_name, header_value)),
                _ => None,
            }
        })
        .collect()
}

// 处理代理响应
async fn handle_proxy_response(response: reqwest::Response) -> Result<HttpResponse, ProxyError> {
    let status = response.status();

    // 转换状态码
    let actix_status = actix_web::http::StatusCode::from_u16(status.as_u16())
        .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);

    let mut client_resp = HttpResponse::build(actix_status);

    // 复制响应头 - 将 Reqwest 的 header 转换为 Actix Web 的 header
    for (name, value) in response.headers() {
        if let (Ok(header_name), Ok(value_str)) =
            (HeaderName::from_str(name.as_str()), value.to_str())
        {
            if let Ok(header_value) = HeaderValue::from_str(value_str) {
                client_resp.insert_header((header_name, header_value));
            }
        }
    }

    // 添加安全头
    client_resp
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .insert_header(("X-Frame-Options", "DENY"))
        .insert_header(("Referrer-Policy", "strict-origin-when-cross-origin"))
        .insert_header(("X-XSS-Protection", "1; mode=block"));

    // 返回响应体
    let body_bytes = response.bytes().await?;
    Ok(client_resp.body(body_bytes))
}

// 代理请求处理器
async fn proxy_request(
    req: HttpRequest,
    body: web::Bytes,
    client: web::Data<Client>,
) -> impl Responder {
    let path = req.path();

    // 提取前缀和剩余路径
    let (prefix, rest_path) = match extract_prefix_and_rest(path) {
        Some((prefix, rest)) => (prefix, rest),
        None => {
            return HttpResponse::NotFound()
                .content_type("application/json")
                .body(r#"{"error": "Endpoint not found", "code": 404}"#);
        }
    };

    // 构建目标 URL
    let target_url = match build_target_url(prefix, rest_path) {
        Ok(url) => url,
        Err(_) => {
            return HttpResponse::BadRequest()
                .content_type("application/json")
                .body(r#"{"error": "Invalid target URL", "code": 400}"#);
        }
    };

    // 构建请求方法
    let method = match req.method().as_str() {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "DELETE" => Method::DELETE,
        "PATCH" => Method::PATCH,
        "OPTIONS" => Method::OPTIONS,
        "HEAD" => Method::HEAD,
        _ => {
            return HttpResponse::MethodNotAllowed()
                .content_type("application/json")
                .body(r#"{"error": "Method not allowed", "code": 405}"#);
        }
    };

    // 处理请求头 - 现在返回 Reqwest 的 header 类型
    let headers = process_headers(&req);

    // 构建并发送请求
    let mut request_builder = client.request(method, target_url.as_str());

    for (name, value) in headers {
        request_builder = request_builder.header(name, value);
    }

    match request_builder.body(body).send().await {
        Ok(response) => match handle_proxy_response(response).await {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!("Error handling response: {}", e);
                HttpResponse::InternalServerError()
                    .content_type("application/json")
                    .body(r#"{"error": "Failed to process response", "code": 500}"#)
            }
        },
        Err(e) => {
            eprintln!("Request error for {}: {}", target_url, e);

            let (status, error_msg) = if e.is_timeout() {
                (
                    actix_web::http::StatusCode::GATEWAY_TIMEOUT,
                    "Gateway Timeout",
                )
            } else if e.is_connect() {
                (
                    actix_web::http::StatusCode::BAD_GATEWAY,
                    "Connection Failed",
                )
            } else {
                (
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error",
                )
            };

            HttpResponse::build(status)
                .content_type("application/json")
                .body(format!(
                    r#"{{"error": "{}", "code": {}}}"#,
                    error_msg,
                    status.as_u16()
                ))
        }
    }
}

// 健康检查端点
async fn health_check() -> impl Responder {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(r#"{"status": "healthy", "service": "api-proxy"}"#)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 设置日志
    unsafe {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    println!("🚀 Starting API Proxy Server on 0.0.0.0:8080");
    println!("📊 Available endpoints:");
    for (path, url) in API_MAPPING.iter() {
        println!("   {} -> {}", path, url);
    }

    let client = create_http_client();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(client.clone()))
            .route("/", web::get().to(root))
            .route("/index.html", web::get().to(root))
            .route("/robots.txt", web::get().to(robots))
            .route("/health", web::get().to(health_check))
            .default_service(web::route().to(proxy_request))
    })
    .bind("0.0.0.0:8080")?
    .workers(4) // 根据 CPU 核心数调整
    .backlog(1024)
    .max_connection_rate(1000)
    .run()
    .await
}
