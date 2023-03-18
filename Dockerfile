FROM debian:slim
RUN apt update && apt install ocrmypdf tesseract-ocr-eng tesseract-ocr-pob tesseract-ocr-deu -y
ARG TARGETARCH
COPY ${TARGETARCH}/drive-ocr /usr/bin/
