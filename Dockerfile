# Node 20 - bypasses glibc issues on older host (Amazon Linux 1)
FROM node:20-bookworm-slim

WORKDIR /app
COPY . .

WORKDIR /app/web
RUN npm install

RUN npm install -g pm2

WORKDIR /app
EXPOSE 3001

# Run PM2 with remote config (serves at /graph/, port 3001)
CMD ["pm2-runtime", "start", "ecosystem.config.cjs", "--only", "scip-callgraph-remote"]
