#!/usr/bin/env bash
set -euo pipefail

NAME="${ES_CONTAINER_NAME:-rag-es}"
IMAGE="${ES_IMAGE:-docker.elastic.co/elasticsearch/elasticsearch:8.15.3}"
PORT="${ES_PORT:-9200}"

if docker ps -a --format '{{.Names}}' | grep -qx "${NAME}"; then
  docker rm -f "${NAME}" >/dev/null
fi

docker run -d \
  --name "${NAME}" \
  -p "${PORT}:9200" \
  -e discovery.type=single-node \
  -e xpack.security.enabled=false \
  -e ES_JAVA_OPTS="-Xms1g -Xmx1g" \
  "${IMAGE}"

echo "waiting for elasticsearch on http://127.0.0.1:${PORT}"
for _ in $(seq 1 60); do
  if curl -fsS "http://127.0.0.1:${PORT}" >/dev/null; then
    echo "elasticsearch is ready"
    exit 0
  fi
  sleep 2
done

echo "elasticsearch did not become ready in time" >&2
docker logs "${NAME}" >&2 || true
exit 1
