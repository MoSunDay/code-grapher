# 本地镜像：宿主机 cargo build --release + npm run build 后构建
# 内网无 Docker Hub，走 daocloud 源；纯 COPY，构建期零网络
FROM docker.m.daocloud.io/library/ubuntu:22.04
# reqwest 构建客户端需要系统 CA store（从宿主机拷入，避免 apt）
COPY certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY target/release/cg-server /usr/local/bin/cg-server
COPY web/dist /app/web/dist
ENV CG_WEB_DIST=/app/web/dist \
    CG_DB_PATH=/data2/cg-data/code-grapher.db \
    OPENCODER_SERVER_URL=http://127.0.0.1:34096 \
    RUST_LOG=info,cg_server=debug
EXPOSE 7800
ENTRYPOINT ["/usr/local/bin/cg-server", "--port", "7800"]
