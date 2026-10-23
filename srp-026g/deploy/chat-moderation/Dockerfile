# The chat gate's local classifier (server.py), as a small image. The models
# are not in it: they are mounted at /models (models.json and one folder per
# model, made by export.py).
#   docker build -t srp-chat-gate deploy/chat-moderation
FROM python:3.12-slim-bookworm
RUN useradd --system --uid 10002 --no-create-home --shell /usr/sbin/nologin gate
WORKDIR /srv/gate
COPY requirements.txt server.py ./
RUN pip install --no-cache-dir -r requirements.txt
USER gate
ENV MODERATION_THREADS=1
EXPOSE 8099
CMD ["python", "server.py", "--models", "/models", "--bind", "0.0.0.0", "--port", "8099"]
