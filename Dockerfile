FROM debian:unstable-slim
RUN apt update && apt install ocrmypdf tesseract-ocr-eng tesseract-ocr-por tesseract-ocr-deu netcat-traditional -y
ARG TARGETARCH
COPY ${TARGETARCH}/drive-ocr /usr/bin/
RUN chmod +x /usr/bin/drive-ocr

EXPOSE 12345
EXPOSE 12346

ENV LISTEN_ADDRESS 0.0.0.0:12345
ENTRYPOINT ["/usr/bin/drive-ocr"]
CMD ["serve"]
