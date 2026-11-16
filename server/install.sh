#!/usr/bin/env bash
# Installe ou met à jour les serveurs de correction de Coquille avec Docker :
#   - LanguageTool, port 8010 (moteur principal) ;
#   - Grammalecte, port 8011 (second moteur, français).
#
# À lancer en root dans un conteneur LXC ou une VM Debian/Ubuntu :
#   bash -c "$(curl -fsSL https://raw.githubusercontent.com/MathieuRDG/coquille/main/server/install.sh)"
#
# Relancer le script met tout à jour (dépôt, image LanguageTool, image Grammalecte).
# Variables facultatives : COQUILLE_DIR (défaut /opt/coquille), COQUILLE_REPO.
set -euo pipefail

REPO="${COQUILLE_REPO:-https://github.com/MathieuRDG/coquille.git}"
DIR="${COQUILLE_DIR:-/opt/coquille}"
FORUM="https://forum.proxmox.com/threads/docker-inside-lxc-net-ipv4-ip_unprivileged_port_start-error.175437/"

say()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m /!\\\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31mErreur :\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(id -u)" -eq 0 ] || die "lance ce script en root (ou avec sudo)."
command -v apt-get >/dev/null || die "ce script vise Debian/Ubuntu. Ailleurs : « docker compose up -d » dans le dossier server/."

in_lxc=false
if [ "$(systemd-detect-virt -c 2>/dev/null || true)" = lxc ]; then in_lxc=true; fi

mem_mb=$(awk '/^MemTotal/ {print int($2 / 1024)}' /proc/meminfo)
if [ "$mem_mb" -lt 2800 ]; then
  warn "${mem_mb} Mo de RAM seulement : LanguageTool (jusqu'à 2 Go) et Grammalecte en demandent 3 Go."
fi

say "Paquets de base"
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq ca-certificates curl git >/dev/null

if ! docker compose version >/dev/null 2>&1; then
  say "Installation de Docker (script officiel get.docker.com)"
  curl -fsSL https://get.docker.com | sh
fi
systemctl enable --now docker >/dev/null 2>&1 || true
if ! docker info >/dev/null 2>&1; then
  if $in_lxc; then
    die "Docker ne démarre pas. Sur le nœud Proxmox : pct set <CTID> --features nesting=1,keyctl=1, puis redémarre le conteneur."
  fi
  die "Docker ne démarre pas : systemctl status docker"
fi

# Dossier server/ : celui du dépôt qui contient ce script, sinon un clone dans $DIR.
here="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd || true)"
if [ -n "$here" ] && [ -f "$here/docker-compose.yml" ] && [ -f "$here/grammalecte/Dockerfile" ]; then
  server="$here"
  say "Dépôt local : $server"
elif [ -d "$DIR/.git" ]; then
  say "Mise à jour du dépôt dans $DIR"
  git -C "$DIR" pull --ff-only -q || warn "mise à jour impossible (modifications locales ?) : on garde la version actuelle."
  server="$DIR/server"
else
  say "Téléchargement du dépôt dans $DIR"
  git clone -q --depth 1 "$REPO" "$DIR"
  server="$DIR/server"
fi
cd "$server"
mapfile -t services < <(docker compose config --services)
has_service() { local s; for s in "${services[@]}"; do [ "$s" = "$1" ] && return 0; done; return 1; }

say "Téléchargement de LanguageTool et construction de Grammalecte (quelques minutes la première fois)"
log=$(mktemp)
trap 'rm -f "$log"' EXIT
if ! { docker compose pull --quiet --ignore-buildable && docker compose up -d --build --remove-orphans; } 2>&1 | tee "$log"; then
  if grep -q ip_unprivileged_port_start "$log"; then
    warn "Régression connue de runc/containerd avec Docker dans un LXC Proxmox."
    warn "Mets à jour le nœud Proxmox (apt update && apt full-upgrade), redémarre ce conteneur, puis relance ce script."
    warn "Détails et contournements : $FORUM"
  fi
  die "le démarrage des conteneurs a échoué (voir ci-dessus)."
fi

wait_for() { # $1 = nom, $2 = URL, $3 = délai max en secondes
  local waited=0
  printf '    attente de %s' "$1"
  until curl -fsS -o /dev/null --max-time 3 "$2" 2>/dev/null; do
    if [ "$waited" -ge "$3" ]; then
      printf ' : pas de réponse.\n'
      return 1
    fi
    printf '.'
    sleep 3
    waited=$((waited + 3))
  done
  printf ' : OK\n'
}

say "Vérification"
sample="Bonjoru, il faut que tu viens demain."
if has_service languagetool; then
  wait_for LanguageTool http://127.0.0.1:8010/v2/languages 300 \
    || die "LanguageTool ne répond pas : docker compose -f $server/docker-compose.yml logs languagetool"
  curl -fsS --max-time 30 -d language=fr --data-urlencode "text=$sample" http://127.0.0.1:8010/v2/check | grep -q '"matches"' \
    || die "LanguageTool répond mal à /v2/check : docker compose -f $server/docker-compose.yml logs languagetool"
fi
if has_service grammalecte; then
  wait_for Grammalecte http://127.0.0.1:8011/get_options/fr 120 \
    || die "Grammalecte ne répond pas : docker compose -f $server/docker-compose.yml logs grammalecte"
  curl -fsS --max-time 30 --data-urlencode "text=$sample" http://127.0.0.1:8011/gc_text/fr | grep -q '"lGrammarErrors"' \
    || die "Grammalecte répond mal à /gc_text/fr : docker compose -f $server/docker-compose.yml logs grammalecte"
fi

ip=$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{for (i = 1; i < NF; i++) if ($i == "src") {print $(i + 1); exit}}' || true)
[ -n "$ip" ] || ip=$(hostname -I 2>/dev/null | awk '{print $1}' || true)
ts_ip=$(tailscale ip -4 2>/dev/null | head -n1 || true)

cat <<EOF

$(printf '\033[1;32m')Serveurs prêts.$(printf '\033[0m')

Dans Coquille, ⚙ Réglages :
  Serveur LanguageTool          http://${ip:-<ip>}:8010
  Second moteur : Grammalecte   http://${ip:-<ip>}:8011
puis « Tester » sur chacun, et « Enregistrer ».
EOF
if [ -n "$ts_ip" ]; then
  echo "Via Tailscale : remplace l'adresse par ${ts_ip}."
fi
cat <<EOF

- Fixe l'IP de cette machine (bail DHCP réservé ou IP statique) : Coquille garde l'adresse en mémoire.
- Aucune authentification : n'expose pas ces ports sur Internet (Tailscale oui, tunnel public non).
- Mise à jour : relance ce script. Journaux : docker compose -f $server/docker-compose.yml logs -f
EOF
