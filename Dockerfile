FROM debian:unstable-slim
RUN apt update && apt install ocrmypdf tesseract-ocr-eng tesseract-ocr-por tesseract-ocr-deu -y
ARG TARGETARCH
COPY ${TARGETARCH}/drive-ocr /usr/bin/
