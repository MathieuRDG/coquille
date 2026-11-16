# Serveur Grammalecte : correcteur grammatical spécialisé dans le français (GPL 3, https://grammalecte.net).
# Construit à partir du paquet Python officiel ; version et empreinte SHA-256 épinglées :
# une archive modifiée fait échouer la construction.
FROM python:3.13-alpine AS download
ARG VERSION=2.3.0
ARG SHA256=aaa4219704857778038ecc1db18f6a907994c0125b8e762dcecc69130280684f
# wget, sha256sum et unzip sont fournis par BusyBox (image Alpine de base) : rien à installer.
RUN wget -q -O /tmp/grammalecte.zip "https://www.grammalecte.net/zip/Grammalecte-fr-v${VERSION}.zip" \
 && echo "${SHA256}  /tmp/grammalecte.zip" | sha256sum -c - \
 && mkdir -p /srv/grammalecte \
 && unzip -q /tmp/grammalecte.zip -d /srv/grammalecte

FROM python:3.13-alpine
COPY --from=download /srv/grammalecte /srv/grammalecte
WORKDIR /srv/grammalecte
ENV PYTHONUNBUFFERED=1
RUN adduser -D -H -s /sbin/nologin grammalecte
USER grammalecte
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s CMD wget -q -O /dev/null http://127.0.0.1:8080/get_options/fr || exit 1
CMD ["python3", "grammalecte-server.py", "-ht", "0.0.0.0", "-p", "8080"]
