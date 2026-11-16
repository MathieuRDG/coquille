#!/usr/bin/env bash
# Crée sur un nœud Proxmox VE un conteneur LXC Debian dédié à Coquille, puis y installe
# LanguageTool (port 8010) et Grammalecte (port 8011) avec server/install.sh.
#
# À lancer en root dans le shell d'un nœud Proxmox :
#   bash -c "$(curl -fsSL https://raw.githubusercontent.com/MathieuRDG/coquille/main/server/proxmox-lxc.sh)"
#
# Réglages facultatifs, en variables d'environnement (valeurs par défaut entre parenthèses) :
#   CTID (prochain libre)   CT_HOSTNAME (coquille)   CORES (2)   MEMORY en Mo (3072)   DISK en Go (12)
#   STORAGE (local-lvm ou local-zfs, sinon le premier stockage de conteneurs actif)
#   TEMPLATE_STORAGE (premier stockage de modèles actif, souvent « local »)
#   BRIDGE (vmbr0)   IP (dhcp, ou par exemple « 192.168.1.50/24,gw=192.168.1.1 »)
#   DEBIAN (13 sur Proxmox VE 9, 12 sur Proxmox VE 8 : PVE 8 ne sait pas démarrer Debian 13)
# Exemple : CTID=150 IP=192.168.1.50/24,gw=192.168.1.1 bash -c "$(curl -fsSL …/proxmox-lxc.sh)"
set -euo pipefail

RAW="${COQUILLE_RAW:-https://raw.githubusercontent.com/MathieuRDG/coquille/main}"

say()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
die()  { printf '\033[1;31mErreur :\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(id -u)" -eq 0 ] || die "lance ce script en root sur un nœud Proxmox."
if ! command -v pct >/dev/null || ! command -v pveam >/dev/null; then
  die "pct/pveam introuvables : ce script se lance sur un nœud Proxmox VE."
fi

CTID="${CTID:-$(pvesh get /cluster/nextid)}"
CT_HOSTNAME="${CT_HOSTNAME:-coquille}"
CORES="${CORES:-2}"
MEMORY="${MEMORY:-3072}"
DISK="${DISK:-12}"
BRIDGE="${BRIDGE:-vmbr0}"
IP="${IP:-dhcp}"

active_storages() { # stockages actifs acceptant le type de contenu $1
  pvesm status --content "$1" 2>/dev/null | awk 'NR > 1 && $3 == "active" {print $1}'
}
if [ -z "${STORAGE:-}" ]; then
  mapfile -t rootdir < <(active_storages rootdir)
  [ "${#rootdir[@]}" -gt 0 ] || die "aucun stockage actif n'accepte les conteneurs (contenu « rootdir »)."
  STORAGE="${rootdir[0]}"
  for preferred in local-zfs local-lvm; do
    for s in "${rootdir[@]}"; do
      if [ "$s" = "$preferred" ]; then STORAGE="$s"; fi
    done
  done
fi
if [ -z "${TEMPLATE_STORAGE:-}" ]; then
  TEMPLATE_STORAGE="$(active_storages vztmpl | head -n1)"
  [ -n "$TEMPLATE_STORAGE" ] || die "aucun stockage actif n'accepte les modèles de conteneur (contenu « vztmpl »)."
fi
if pct status "$CTID" >/dev/null 2>&1; then die "le conteneur $CTID existe déjà : choisis un autre CTID."; fi

say "Modèle Debian"
pveam update >/dev/null || true
pve_major="$(pveversion 2>/dev/null | sed -n 's#^pve-manager/\([0-9]*\)\..*#\1#p')"
if [ -z "${DEBIAN:-}" ]; then
  if [ "${pve_major:-0}" -ge 9 ]; then DEBIAN=13; else DEBIAN=12; fi
fi
# Architecture du nœud : l'index des modèles peut aussi proposer de l'arm64, qui ne démarre pas
# sur un nœud x86 (« Exec format error » sur /sbin/init).
arch="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
template=""
for version in "$DEBIAN" 12; do
  template="$(pveam available --section system \
    | awk -v p="^debian-$version-standard_.*_${arch}[.]tar[.]" '$2 ~ p {print $2}' | sort -V | tail -n1)"
  [ -n "$template" ] && break
done
[ -n "$template" ] || die "aucun modèle Debian 12/13 pour l'architecture $arch dans « pveam available »."
if ! pveam list "$TEMPLATE_STORAGE" | grep -F "$template" >/dev/null; then
  pveam download "$TEMPLATE_STORAGE" "$template"
fi

say "Création du conteneur $CTID ($CT_HOSTNAME) : $CORES vCPU, $MEMORY Mo, $DISK Go sur $STORAGE, réseau $BRIDGE ($IP)"
pct create "$CTID" "$TEMPLATE_STORAGE:vztmpl/$template" \
  --hostname "$CT_HOSTNAME" \
  --cores "$CORES" --memory "$MEMORY" --swap 512 \
  --rootfs "$STORAGE:$DISK" \
  --net0 "name=eth0,bridge=$BRIDGE,ip=$IP" \
  --arch "$arch" --unprivileged 1 --features nesting=1,keyctl=1 \
  --onboot 1 --tags coquille \
  --description "Coquille : LanguageTool (port 8010) + Grammalecte (port 8011). Mise à jour : bash /root/coquille-install.sh"
if ! pct start "$CTID"; then
  printf '\n' >&2
  die "le conteneur $CTID ne démarre pas. La vraie cause est la première ligne d'erreur de :
    pct start $CTID --debug 2>&1 | grep -iE 'error|unsupported|denied' | head -n 20
  Une fois corrigé : pct destroy $CTID, puis relance ce script."
fi

say "Attente du réseau dans le conteneur"
for _ in $(seq 1 30); do
  if pct exec "$CTID" -- getent hosts deb.debian.org >/dev/null 2>&1; then break; fi
  sleep 2
done
pct exec "$CTID" -- getent hosts deb.debian.org >/dev/null 2>&1 \
  || die "pas d'accès réseau ou DNS dans le conteneur $CTID (vérifie le pont $BRIDGE et l'IP)."

say "Installation des serveurs dans le conteneur (plusieurs minutes)"
pct exec "$CTID" -- bash -c "export DEBIAN_FRONTEND=noninteractive \
  && apt-get update -qq && apt-get install -y -qq curl ca-certificates >/dev/null \
  && curl -fsSL '$RAW/server/install.sh' -o /root/coquille-install.sh \
  && bash /root/coquille-install.sh" </dev/null

echo
say "Conteneur $CTID prêt. Console : pct enter $CTID — mise à jour : pct exec $CTID -- bash /root/coquille-install.sh"
