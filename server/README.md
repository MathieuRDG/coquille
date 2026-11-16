# Serveurs de correction auto-hébergés (Proxmox + Tailscale)

Sans serveur, Coquille utilise le serveur public `https://api.languagetool.org`. Il suffit pour tester, mais il est limité (environ 20 requêtes par minute, d'où une bulle moins réactive), et tes textes partent chez un tiers. Ton propre serveur supprime ces limites, garde tes textes chez toi, ajoute **Grammalecte** (qui n'a pas de serveur public) et sert aussi l'extension navigateur LanguageTool.

## Installation express

### Sur Proxmox (recommandé)

Dans le shell d'un nœud (Datacenter › nœud › **Shell**, en root) :

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/MathieuRDG/coquille/main/server/proxmox-lxc.sh)"
```

Le script [proxmox-lxc.sh](proxmox-lxc.sh) :

1. télécharge le modèle Debian adapté au nœud : **Debian 13 sur Proxmox VE 9, Debian 12 sur Proxmox VE 8** (PVE 8 ne sait pas démarrer Debian 13), toujours dans l'**architecture du nœud** (amd64 sur un PC x86) ;
2. crée un conteneur **non privilégié** : 2 vCPU, 3 Go de RAM, 512 Mo de swap, 12 Go de disque, options `nesting` et `keyctl` (nécessaires à Docker), démarrage avec le nœud, tag `coquille` ;
3. attend le réseau, puis lance [install.sh](install.sh) dans le conteneur : Docker, LanguageTool (port **8010**), Grammalecte (port **8011**) ;
4. vérifie que les deux serveurs répondent et affiche les adresses à saisir dans Coquille.

Compte quelques minutes, surtout pour télécharger LanguageTool. Testé sur Proxmox VE 9.2 (nœud x86).

Valeurs par défaut : prochain CTID libre, stockage `local-lvm` ou `local-zfs` (sinon le premier stockage de conteneurs actif), pont `vmbr0`, DHCP. Pour les changer, préfixe la commande :

```bash
CTID=150 STORAGE=ceph-vm IP=192.168.1.50/24,gw=192.168.1.1 bash -c "$(curl -fsSL https://raw.githubusercontent.com/MathieuRDG/coquille/main/server/proxmox-lxc.sh)"
```

| Variable | Défaut | Rôle |
|----------|--------|------|
| `CTID` | prochain libre | Numéro du conteneur |
| `CT_HOSTNAME` | `coquille` | Nom d'hôte |
| `CORES` / `MEMORY` / `DISK` | `2` / `3072` (Mo) / `12` (Go) | Ressources |
| `STORAGE` | `local-lvm` ou `local-zfs` | Stockage du disque du conteneur (un stockage partagé comme Ceph permet la haute disponibilité) |
| `TEMPLATE_STORAGE` | premier stockage de modèles actif | Où télécharger le modèle Debian |
| `BRIDGE` | `vmbr0` | Pont réseau |
| `IP` | `dhcp` | Ou une IP fixe : `192.168.1.50/24,gw=192.168.1.1` |
| `DEBIAN` | `13` (PVE 9) ou `12` (PVE 8) | Version de Debian |

### Dans un conteneur ou une VM Debian/Ubuntu existants

En root :

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/MathieuRDG/coquille/main/server/install.sh)"
```

Le script installe Docker si besoin, clone le dépôt dans `/opt/coquille`, démarre les deux serveurs, vérifie qu'ils répondent et affiche les adresses. Il prévient si la machine a moins de 3 Go de RAM.

### Dans Coquille

Dans ⚙ Réglages :

1. **Serveur LanguageTool** : `http://<ip>:8010`, puis **Tester** ;
2. **Second moteur : Grammalecte** : `http://<ip>:8011`, puis **Tester** ;
3. **Enregistrer**.

Réserve l'IP du conteneur dans ton DHCP, ou passe une IP fixe au script (`IP=…`), car Coquille garde l'adresse en mémoire. Pour un accès hors de chez toi, voir la section 4 (Tailscale).

### Entretien

Depuis le nœud Proxmox (remplace `<CTID>` par le numéro du conteneur) :

| Action | Commande |
|--------|----------|
| Mettre à jour (dépôt, LanguageTool, Grammalecte) | `pct exec <CTID> -- /usr/local/sbin/coquille-update` |
| État des serveurs | `pct exec <CTID> -- docker compose -f /opt/coquille/server/docker-compose.yml ps` |
| Journaux | `pct exec <CTID> -- docker compose -f /opt/coquille/server/docker-compose.yml logs --tail 50` |
| Console du conteneur | `pct enter <CTID>` |
| Tout supprimer | `pct stop <CTID> && pct destroy <CTID>` |

Dans un conteneur installé avant l'ajout de `coquille-update`, lance une fois `pct exec <CTID> -- bash /root/coquille-install.sh` : la commande sera créée au passage.

### Dépannage de l'installation

| Erreur | Cause et solution |
|--------|-------------------|
| `Exec format error - Failed to exec "/sbin/init"` au démarrage du conteneur | Le modèle n'est pas de l'architecture du nœud (arm64 sur un nœud x86). Les anciennes versions du script pouvaient le choisir. Supprime le conteneur (`pct destroy <CTID>`) et le modèle (`pveam remove local:vztmpl/…_arm64.tar.zst`), puis relance le script. |
| `unsupported debian version` | Modèle Debian 13 sur Proxmox VE 8. Le script choisit maintenant Debian 12 sur PVE 8 ; sinon, force-le avec `DEBIAN=12`. |
| `startup for container '…' failed` (sans autre détail) | La vraie cause est la première ligne `ERROR` de `pct start <CTID> --debug 2>&1 \| grep -iE 'error\|unsupported\|denied'`. |
| `open sysctl net.ipv4.ip_unprivileged_port_start … permission denied` | Régression runc/containerd dans les LXC Proxmox : voir l'encadré de la section 1. |
| Docker ne démarre pas dans le conteneur | Options manquantes : `pct set <CTID> --features nesting=1,keyctl=1`, puis redémarre le conteneur. |
| Coquille affiche « Serveur injoignable » | Vérifie l'IP du conteneur (`pct exec <CTID> -- hostname -I`), puis l'état des serveurs (tableau ci-dessus). |

Les sections suivantes détaillent l'installation manuelle.

## 1. Conteneur LXC sur Proxmox

Les valeurs suivantes conviennent pour un usage personnel. LanguageTool est réglé pour utiliser jusqu'à 2 Go de mémoire Java, et Grammalecte prend environ 220 Mo :

| Ressource | Sans n-grammes | Avec n-grammes FR |
|-----------|----------------|-------------------|
| vCPU      | 2              | 2                 |
| RAM       | 3 Go           | 4 Go              |
| Disque    | 12 Go          | 25 Go             |

Utilise un modèle Debian 12 ou 13, en conteneur non privilégié. Docker dans un LXC nécessite les options **nesting** et **keyctl** :

```bash
pct set <CTID> --features nesting=1,keyctl=1
```

Dans le conteneur :

```bash
apt update && apt install -y curl git unzip
curl -fsSL https://get.docker.com | sh
```

> Si Docker refuse de démarrer les conteneurs avec l'erreur `open sysctl net.ipv4.ip_unprivileged_port_start file: … permission denied`, c'est une régression connue de runc/containerd dans les LXC Proxmox, apparue fin 2025. Mets à jour le nœud Proxmox, puis redémarre le conteneur. Si l'erreur persiste, consulte le [fil du forum Proxmox](https://forum.proxmox.com/threads/docker-inside-lxc-net-ipv4-ip_unprivileged_port_start-error.175437/).

## 2. Lancement

Clone le dépôt (`git clone https://github.com/MathieuRDG/coquille /opt/coquille`) ou copie le dossier `server/` dans le conteneur, puis :

```bash
cd server
docker compose up -d
curl -s -d "language=fr&text=Bonjoru les amis est là" http://localhost:8010/v2/check | head -c 300
```

Le premier démarrage prend environ 30 secondes.

### Grammalecte (second moteur, facultatif)

Le même `docker compose up -d` construit et démarre aussi **Grammalecte** (licence GPL 3, téléchargé à la construction et non inclus dans Coquille), sur le port **8011**. La première construction télécharge le paquet officiel depuis grammalecte.net et vérifie son empreinte SHA-256. Ressources mesurées : environ 220 Mo de RAM, une image de 100 Mo, quelques dixièmes de seconde par analyse. Pour vérifier qu'il répond :

```bash
curl -s --data-urlencode "text=Il faut que tu viens." http://localhost:8011/gc_text/fr
```

Dans Coquille, va dans ⚙ › Second moteur : Grammalecte, saisis `http://<ip-du-lxc>:8011`, puis clique sur **Tester**. Si tu n'en veux pas, supprime le service `grammalecte` du `docker-compose.yml`, ou lance seulement `docker compose up -d languagetool`.

## 3. N-grammes français (facultatif, recommandé)

Les n-grammes permettent de repérer les confusions de mots (a/à, ses/ces, quand/quant…). Le téléchargement pèse plusieurs Go. Avec l'installation express, agrandis d'abord le disque et la mémoire depuis le nœud (`pct resize <CTID> rootfs +15G` et `pct set <CTID> --memory 4096`), puis travaille dans `/opt/coquille/server` :

```bash
./get-ngrams-fr.sh
# puis décommente « langtool_languageModel=/ngrams » dans docker-compose.yml
docker compose up -d
```

## 4. Accès depuis tes PC via Tailscale

Sur ton réseau local, l'IP locale du conteneur suffit. Pour y accéder hors de chez toi, deux cas :

- **Tailscale tourne déjà en *subnet router*** et annonce ton réseau local : l'IP locale du conteneur fonctionne telle quelle à travers Tailscale, il n'y a rien à faire.
- **Sinon, installe Tailscale dans le conteneur.** Un LXC non privilégié a besoin du périphérique TUN. Ajoute ces deux lignes à `/etc/pve/lxc/<CTID>.conf` sur le nœud, puis redémarre le conteneur :

  ```
  lxc.cgroup2.devices.allow: c 10:200 rwm
  lxc.mount.entry: /dev/net/tun dev/net/tun none bind,create=file
  ```

  Ensuite, dans le conteneur, lance `curl -fsSL https://tailscale.com/install.sh | sh && tailscale up`.

Une fois Tailscale installé dans le conteneur, choisis une des deux options ci-dessous. Le principe est le même pour Grammalecte, sur le port 8011.

**Option simple (HTTP sur le tailnet)** : dans Coquille, indique `http://<ip-tailscale-du-lxc>:8010`, ou le nom MagicDNS du conteneur.

**Option HTTPS avec certificat valide** : à préférer pour l'extension navigateur.

```bash
tailscale serve --bg 8010
# → https://<machine>.<tailnet>.ts.net
```

Dans Coquille, va dans ⚙ › Serveur LanguageTool, colle l'URL, puis clique sur **Tester**.

> **Cloudflared** : n'expose pas ce serveur publiquement avec un tunnel sans protection. LanguageTool n'a pas d'authentification. Si tu le protèges avec Cloudflare Access, Coquille ne pourra pas s'y connecter, car il n'envoie pas de jeton Access. Tailscale est la bonne solution ici.

## 5. Bonus : correction en direct dans le navigateur

L'extension officielle LanguageTool (Firefox, Chrome, Edge) peut utiliser **ton** serveur, ce qui donne la correction en direct dans Gmail, les formulaires, etc. :

1. Ouvre les paramètres de l'extension, puis **Paramètres avancés** › **Serveur LanguageTool**.
2. Choisis **Autre serveur** et saisis l'URL de ton serveur suivie de `/v2`, par exemple `https://lt.<tailnet>.ts.net/v2`.

Si Firefox refuse la connexion (problème CORS connu), consulte le [contournement fourni avec l'image Docker](https://github.com/Erikvl87/docker-languagetool/tree/main/cors-workaround).

LibreOffice sait aussi utiliser un serveur LanguageTool distant : Outils › Options › Paramètres linguistiques › Serveur distant LanguageTool.

## Ajouter des mots au dictionnaire du serveur

Le dictionnaire personnel de Coquille est local à chaque PC. Pour un dictionnaire commun à tous tes appareils (y compris l'extension), suis la section « Improving the spell checker » du [README de l'image](https://github.com/Erikvl87/docker-languagetool#improving-the-spell-checker). Pour le français, le fichier est `org/languagetool/resource/fr/hunspell/spelling.txt`.
