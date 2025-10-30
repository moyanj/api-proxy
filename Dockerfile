# 多阶段构建：构建阶段
FROM rust:1.90-alpine as builder

# 安装构建依赖
RUN apk add --no-cache musl-dev pkgconfig openssl-dev

# 创建工作目录
WORKDIR /app

# 复制源文件
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# 构建应用（使用 --release 优化）
RUN cargo build --release
# 多阶段构建：运行阶段
FROM alpine:3.18

# 安装运行时依赖
RUN apk add --no-cache ca-certificates openssl && \
    addgroup -S app && adduser -S app -G app

# 设置工作目录
WORKDIR /app

# 从构建阶段复制二进制文件
COPY --from=builder /app/target/release/api-proxy ./api-proxy

# 设置所有权
RUN chown -R app:app /app

# 切换到非root用户
USER app

# 暴露端口
EXPOSE 8080

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

# 设置环境变量默认值
ENV PROXY_HOST=0.0.0.0
ENV PROXY_PORT=8080
ENV PROXY_WORKERS=4
ENV MAX_BODY_SIZE_MB=10
ENV REQUEST_TIMEOUT=30
ENV CONNECT_TIMEOUT=10
ENV RUST_LOG=info

# 启动应用
CMD ["./api-proxy"]