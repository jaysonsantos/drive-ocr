# Drive OCR

IFTTT webhook to ocr pdf files and push to google drive.

```
Usage: drive-ocr --secret-key <SECRET_KEY> --redis-dsn <REDIS_DSN> --google-credentials <GOOGLE_CREDENTIALS> <COMMAND>

Commands:
  generate-key  Generate a key to be used on IFTT's webhook, you will need to open a link in your browser and authorize the app.
  serve         Start a webserver to answer for IFTT's webhooks.
  help          Print this message or the help of the given subcommand(s)

Options:
  -s, --secret-key <SECRET_KEY>
          Secret key to sign generated token ids [env: SECRET_KEY=xxx]
  -r, --redis-dsn <REDIS_DSN>
          Redis connection to persist google credentials [env: REDIS_DSN=redis://10.43.24.13/2]
  -g, --google-credentials <GOOGLE_CREDENTIALS>
          Path to google's credentials JSON generated on google's dev console. [env: GOOGLE_CREDENTIALS=client_secret_xxxx-xxxxx.apps.googleusercontent.com.json]
  -h, --help
          Print help

```
