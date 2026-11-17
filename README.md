# Coquille

**Correcteur d'orthographe, de grammaire et de typographie pour Linux, Windows et macOS.** Il fonctionne dans toutes les applications : tu sélectionnes du texte, tu appuies sur un raccourci, tu corriges, et le texte corrigé remplace ta sélection.

![Éditeur de Coquille](docs/capture-editeur.png)

- Fonctionne **dans n'importe quelle application** : navigateur, Discord, Word, LibreOffice, VS Code, client mail…
- **Éditeur intégré** avec vérification pendant la frappe, fautes soulignées par type (orthographe, grammaire, typographie, style).
- **Bulle de correction en direct** sous Windows : une pastille à côté du champ où tu écris, sans ouvrir l'application.
- Suggestions en un clic, **Tout corriger**, annulation avec Ctrl+Z, dictionnaire personnel, règles désactivables.
- Moteur **[LanguageTool](https://languagetool.org)** : le serveur public, ou **ton propre serveur**, installé **en une commande sur Proxmox** (ou sur toute machine Debian/Ubuntu) et accessible via Tailscale.
- Second moteur facultatif, **[Grammalecte](https://grammalecte.net)**, spécialisé dans le français : il complète LanguageTool sur les accords, la conjugaison, les participes passés et le subjonctif.
- Binaire léger (≈ 6 Mo sous Linux) grâce à [Tauri](https://tauri.app) (Rust + WebView du système).

## Téléchargement

Dernière version : **[page des releases](https://github.com/MathieuRDG/coquille/releases/latest)**.

| Système | Fichier à télécharger |
|---------|-----------------------|
| Windows 10 / 11 | `Coquille_…_x64-setup.exe` (ou le `.msi`) |
| macOS Apple Silicon (M1 et suivants) | `Coquille_…_aarch64.dmg` |
| macOS Intel | `Coquille_…_x64.dmg` |
| Linux | `.AppImage`, `.deb` ou `.rpm` ; sous Arch, le PKGBUILD (voir « Installation ») |

Les binaires ne sont pas signés : Windows et macOS affichent un avertissement au premier lancement (voir « Installation »).

**Mises à jour automatiques** depuis la 0.4.0 : une fois installée, Coquille se met à jour seule (Windows, macOS, AppImage). Voir « Mises à jour ».

## Est-ce de l'IA ?

Non, pas au sens « ChatGPT ». LanguageTool est un moteur **à règles** : plusieurs milliers de règles grammaticales écrites par des linguistes, un dictionnaire orthographique et, en option, des statistiques de fréquence de mots (les « n-grammes »). C'est la même famille de technologie que le cœur d'**Antidote** (analyse linguistique symbolique). **Grammalecte**, le second moteur facultatif, fonctionne de la même façon, avec des règles écrites spécifiquement pour le français. **QuillBot**, lui, repose surtout sur des modèles d'IA, en particulier pour la reformulation.

Conséquences :

- les corrections sont **déterministes** et chaque remarque explique la règle appliquée ;
- tout tourne **en local** si tu héberges le serveur, sans GPU ;
- en revanche, Coquille ne **reformule** pas des phrases entières comme QuillBot. Une fonction de reformulation par IA (avec un modèle local comme Ollama, par exemple) pourrait s'ajouter plus tard.

## Fonctionnement

```
 Application quelconque          Coquille (barre système)            Serveurs de correction
 ┌──────────────────────┐  raccourci  ┌──────────────────────┐  HTTP  ┌──────────────────┐
 │ texte sélectionné    │ ──────────▶ │ capture la sélection │ ─────▶ │ LanguageTool     │
 │                      │             │ affiche les fautes   │ ◀───── │ (public ou perso)│
 │ texte corrigé ◀──────┼─ Ctrl+Entrée│ recolle le résultat  │        │ + Grammalecte    │
 └──────────────────────┘             └──────────────────────┘        │   (facultatif)   │
                                                                      └──────────────────┘
```

| Système | Lecture de la sélection | Remplacement |
|---------|-------------------------|--------------|
| Linux X11 | sélection primaire (texte surligné), presse-papiers intact | Ctrl+V simulé |
| Linux Wayland | `wl-paste --primary` | `wl-copy` + `wtype` ou `ydotool` |
| Windows | Ctrl+C simulé, puis restauration du presse-papiers | Ctrl+V simulé, puis restauration |
| macOS | Cmd+C simulé, puis restauration du presse-papiers | Cmd+V simulé, puis restauration |

## Installation

### Arch Linux

```bash
git clone https://github.com/MathieuRDG/coquille.git
```

```bash
cd coquille/packaging/arch
```

```bash
makepkg -si
```

Sous **Wayland**, installe aussi les outils de presse-papiers et de frappe :

```bash
sudo pacman -S wl-clipboard wtype
```

- `wl-clipboard` est obligatoire sous Wayland.
- `wtype` fonctionne sous Hyprland, Sway et les autres compositeurs wlroots.
- Sous KDE Plasma ou GNOME, `wtype` n'est pas pris en charge : utilise `ydotool` (démon `ydotoold` requis, voir le wiki Arch) ou colle toi-même avec Ctrl+V. Coquille te prévient si le collage automatique est impossible.

Pour les autres distributions : **AppImage**, `.deb` ou `.rpm` dans les [releases](https://github.com/MathieuRDG/coquille/releases/latest).

### Windows

Récupère le `-setup.exe` (ou le `.msi`) dans les [releases](https://github.com/MathieuRDG/coquille/releases/latest). Le binaire n'est pas signé : SmartScreen affiche « Windows a protégé votre ordinateur ». Clique sur **Informations complémentaires**, puis **Exécuter quand même**.

### macOS

Récupère le `.dmg` dans les [releases](https://github.com/MathieuRDG/coquille/releases/latest) (`aarch64` pour Apple Silicon, `x64` pour Intel). L'application n'est pas signée par Apple, alors au premier lancement fais clic droit › **Ouvrir**, ou exécute :

```bash
xattr -dr com.apple.quarantine /Applications/Coquille.app
```

Au premier raccourci, macOS demande l'autorisation **Accessibilité** (Réglages Système › Confidentialité et sécurité › Accessibilité). Elle est nécessaire pour simuler Cmd+C et Cmd+V. Relance Coquille après l'avoir accordée.

## Mises à jour

À partir de la **0.4.0**, plus besoin de réinstaller : Coquille vérifie au démarrage puis toutes les 6 heures s'il existe une nouvelle release sur GitHub.

| Installation | Comportement |
|--------------|--------------|
| Windows (`-setup.exe` ou `.msi`), macOS (`.dmg`), Linux `.AppImage` | Téléchargement en arrière-plan, puis installation quand la fenêtre de Coquille est fermée et que l'ordinateur n'est pas utilisé (sous Windows : 2 minutes sans clavier ni souris). Coquille redémarre seule et garde ses réglages. |
| Linux `.deb` ou `.rpm` | Téléchargement automatique, installation sur ton clic (« Installer et redémarrer ») : elle demande le mot de passe administrateur. |
| Arch (PKGBUILD) | Coquille signale la nouvelle version ; mets à jour avec `git pull` puis `makepkg -si` dans `packaging/arch`. |

- ⚙ › **Mises à jour** : désactiver l'installation automatique, « Rechercher », « Installer et redémarrer », notes de version.
- Chaque mise à jour est **signée** ; Coquille refuse un fichier dont la signature ne correspond pas à la clé publique intégrée.
- Après une mise à jour, un bandeau l'indique à la prochaine ouverture de la fenêtre.
- Les versions 0.3.x et antérieures n'ont pas ce mécanisme : installe la 0.4.0 une dernière fois à la main.

## Utilisation

1. Sélectionne du texte dans n'importe quelle application.
2. Appuie sur le raccourci : **Ctrl+Alt+Espace** (Windows, Linux X11) ou **⌘+Maj+Espace** (macOS). Si une autre application l'utilise déjà, Coquille choisit automatiquement une combinaison libre (par exemple **Ctrl+Alt+K**) et te l'indique. Pour le changer : ⚙ › Raccourci global › clique sur la touche affichée, puis appuie sur la combinaison voulue.
3. Corrige : clique sur un mot souligné ou sur une carte à droite, puis sur une suggestion. **Tout corriger** applique la première suggestion de chaque faute, sauf les remarques de style.
4. Appuie sur **Ctrl+Entrée** (⌘+Entrée) ou sur **Remplacer** : la fenêtre se masque et le texte corrigé remplace ta sélection.

Échap ferme la fenêtre. Coquille reste dans la barre système ; un clic sur l'icône ouvre l'éditeur.

### Bulle de correction en direct (Windows)

Pas besoin d'ouvrir Coquille ni d'utiliser le raccourci : dès que tu écris au moins 3 mots dans un champ (navigateur, Discord, Outlook, Word, Bloc-notes…), une **pastille** apparaît au bord droit du champ, à la hauteur du curseur :

- **rouge avec un chiffre** : nombre de fautes ;
- **violette** : seulement des remarques de style ;
- **verte ✓** : aucune faute ;
- **grise** : vérification en cours.

Un clic sur la pastille ouvre la liste des corrections. Un clic sur une suggestion **corrige directement le mot dans ton champ**, puis le curseur revient où il était. **Éditeur** ouvre tout le texte du champ dans la fenêtre principale.

Les corrections s'appuient sur l'analyse de la phrase d'origine, gardée en mémoire. Corriger un mot ne relance pas la vérification : les autres suggestions restent en place et tu peux les enchaîner. La phrase n'est réanalysée que si tu tapes toi-même du texte, ou quand toutes les corrections sont faites (vérification de contrôle). Le bouton **↻** force une nouvelle analyse. L'éditeur principal fonctionne de la même façon : clique sur le compteur de fautes pour revérifier.

- La bulle ne prend jamais le focus : tu continues à taper normalement.
- Elle ignore les **champs de mot de passe**, les barres d'adresse et les applications exclues (gestionnaires de mots de passe, terminaux, VS Code… ; liste modifiable dans ⚙).
- Elle se désactive dans ⚙ › Bulle de correction en direct.
- Elle n'apparaît pas dans un champ ? ⚙ › Bulle de correction en direct › **Diagnostic**, puis clique dans le champ : Coquille affiche ce qu'il voit du champ (application, type de contrôle…) et la raison exacte. **Copier le diagnostic** permet de le joindre à une [issue](https://github.com/MathieuRDG/coquille/issues).
- Elle s'appuie sur l'API d'accessibilité de Windows (UI Automation). Quelques applications n'exposent pas leur texte ; dans ce cas, utilise le raccourci.
- Le texte des champs est envoyé aux serveurs configurés (LanguageTool, et Grammalecte s'il est activé). Le serveur public limite le nombre de requêtes : la bulle attend alors au moins 3,2 s entre deux vérifications, contre 0,4 s avec ton propre serveur.

Linux et macOS : prévu plus tard. Sous Wayland, une application ne peut pas placer une fenêtre où elle veut, ce qui complique la chose.

### Raccourci sous Wayland (Hyprland, KDE, GNOME, Sway)

Sous Wayland, une application ne peut pas intercepter un raccourci global. Associe donc la commande `coquille --corriger` à une touche dans ton bureau. Si Coquille tourne déjà, la commande réveille l'instance existante.

```ini
# Hyprland (~/.config/hypr/hyprland.conf)
bind = CTRL ALT, space, exec, coquille --corriger
# Fenêtre flottante et centrée (facultatif)
windowrulev2 = float, class:^(coquille)$
windowrulev2 = size 1000 640, class:^(coquille)$
windowrulev2 = center, class:^(coquille)$
```

- **KDE Plasma** : Configuration du système › Clavier › Raccourcis › Ajouter un nouveau › Commande ou script.
- **GNOME** : Paramètres › Clavier › Raccourcis personnalisés.
- **Sway** : `bindsym Ctrl+Alt+space exec coquille --corriger`

Autres options de ligne de commande : `--cache` (démarrer en arrière-plan) et `--parametres` (ouvrir les réglages).

## Serveurs de correction

Par défaut, Coquille utilise le serveur public `https://api.languagetool.org`. Il est gratuit, mais limité (environ 20 requêtes par minute et 20 Ko par texte), et tes textes partent chez un tiers. Pour un usage quotidien, héberge tes propres serveurs : plus de limite, une bulle plus réactive, tes textes restent chez toi, et tu peux ajouter Grammalecte (voir plus bas).

### Installation sur Proxmox (une commande)

Dans le shell d'un nœud Proxmox (Datacenter › nœud › **Shell**, en root) :

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/MathieuRDG/coquille/main/server/proxmox-lxc.sh)"
```

Le script crée un conteneur LXC Debian non privilégié (2 vCPU, 3 Go de RAM, 12 Go de disque, démarrage avec le nœud). Il y installe Docker, **LanguageTool** (port 8010) et **Grammalecte** (port 8011), vérifie qu'ils répondent, puis affiche les adresses à saisir dans Coquille. Il choisit seul le modèle Debian adapté à ta version de Proxmox et à l'architecture du nœud. Testé sur Proxmox VE 9.2.

Ensuite, dans Coquille, va dans ⚙ Réglages : saisis `http://<ip>:8010` dans **Serveur LanguageTool** et `http://<ip>:8011` dans **Second moteur : Grammalecte**, clique sur **Tester** pour chacun, puis sur **Enregistrer**. Réserve l'IP du conteneur dans ton DHCP.

Pour mettre à jour les serveurs, depuis le nœud :

```bash
pct exec <CTID> -- /usr/local/sbin/coquille-update
```

Hors Proxmox (VM ou conteneur Debian/Ubuntu), lance [server/install.sh](server/install.sh) de la même façon, ou `docker compose up -d` dans `server/`. Le **[README du serveur](server/README.md)** détaille les options (CTID, stockage, IP fixe…), l'entretien, le dépannage, l'accès via Tailscale et les n-grammes.

### Grammalecte, second moteur (facultatif)

Pour aller plus loin sur la conjugaison et les accords, ajoute **Grammalecte**, un correcteur libre spécialisé dans le français. Il repère par exemple :

- « il faut que tu viens » → « viennes » (subjonctif) ;
- « la voiture que j'ai acheté » → « achetée » (participe passé et complément placé avant) ;
- « je t'ai renvoyer » → « renvoyé ».

Coquille interroge les deux moteurs en même temps et fusionne leurs remarques sans doublons ; celles de Grammalecte portent la mention « Grammalecte ». Il n'existe pas de serveur Grammalecte public : l'installation ci-dessus le construit à partir du paquet officiel (environ 220 Mo de RAM). Indique ensuite son adresse dans ⚙ › Second moteur : Grammalecte, puis clique sur **Tester**. Si le serveur ne répond pas, Coquille continue avec LanguageTool seul et le signale.

### Extension navigateur

Avec ton serveur, tu peux aussi brancher l'**extension navigateur officielle LanguageTool** dessus. Tu obtiens alors la correction **en direct pendant la frappe** dans Gmail, les formulaires web, etc. (voir la section « Bonus » du README du serveur).

## Limites connues

- **Correction en direct (bulle) : Windows uniquement pour l'instant.** Les mots ne sont pas soulignés à l'intérieur des autres applications (Grammarly y consacre des années de développement, application par application). La bulle indique les fautes et les corrige. Sous Linux et macOS, utilise le raccourci, ou l'extension LanguageTool branchée sur ton serveur pour le navigateur.
- **Bulle et mise en forme** : une correction remplace seulement le mot visé. En revanche, « Éditeur » puis « Remplacer » recolle tout le champ en texte brut, ce qui fait perdre gras, liens, etc. dans les éditeurs riches.
- **Terminaux** : ils collent avec Ctrl+Maj+V, donc le remplacement automatique n'y fonctionne pas. Utilise **Copier**.
- **Presse-papiers** : seul son contenu *texte* est restauré après un remplacement. Une image copiée juste avant serait perdue.
- **macOS** : les paquets sont compilés automatiquement, mais n'ont pas encore été testés sur une vraie machine. Linux (X11 et Wayland) et Windows sont testés et utilisés au quotidien.
- **Serveurs** : l'installation en une commande est testée sur Proxmox VE 9.2 avec un nœud x86. `install.sh` vise Debian et Ubuntu ; ailleurs, utilise `docker compose` directement.
- Un problème, une idée ? Ouvre une [issue](https://github.com/MathieuRDG/coquille/issues).

## Dépannage

| Problème | Solution |
|----------|----------|
| Fenêtre blanche (Linux, NVIDIA) | Déjà contourné (`WEBKIT_DISABLE_DMABUF_RENDERER=1`). Sinon, essaie `GDK_BACKEND=x11 coquille`. |
| « … est déjà réservé par une autre application » | Un autre logiciel l'utilise déjà (lanceur, PowerToys, logiciel de clavier ou de capture…). Choisis-en un autre dans ⚙ › Raccourci global. |
| La bulle n'apparaît pas | Lance ⚙ › Bulle de correction en direct › **Diagnostic**, puis clique dans le champ : la raison s'affiche. Causes courantes : moins de 3 mots, application exclue ou lancée en administrateur, Google Docs (active *Outils › Accessibilité › Prise en charge du lecteur d'écran*), navigateur qui n'expose pas la page (`chrome://accessibility` › cocher *Native accessibility API support* et *Web accessibility*). |
| « Aucun texte sélectionné » (Wayland) | Vérifie que `wl-paste --primary` affiche bien ta sélection dans un terminal. |
| « Serveur injoignable » | Teste l'adresse dans ⚙. Sur Proxmox, vérifie les serveurs avec `pct exec <CTID> -- docker compose -f /opt/coquille/server/docker-compose.yml ps`, et Tailscale si tu es hors de chez toi. |
| Le script Proxmox échoue | Voir « Dépannage de l'installation » dans le [README du serveur](server/README.md). |
| « Grammalecte injoignable (…) : texte vérifié par LanguageTool seul » | Le serveur Grammalecte ne répond pas : vérifie son état comme ci-dessus. Coquille continue avec LanguageTool en attendant. |
| Code 429 | Le serveur public te limite : passe sur ton propre serveur. |
| « Mise à jour : vérification des mises à jour impossible » | Pas d'accès à `github.com` (proxy d'entreprise, pare-feu) : Coquille réessaiera dans 6 heures. Sinon, installe la dernière version à la main. |
| Pas d'icône dans la barre (GNOME) | Installe l'extension *AppIndicator and KStatusNotifierItem Support*, ou relance `coquille` pour rouvrir la fenêtre. |

La configuration est stockée dans `~/.config/org.coquille.correcteur/config.json` (Linux), `%APPDATA%\org.coquille.correcteur\` (Windows) ou `~/Library/Application Support/org.coquille.correcteur/` (macOS).

## Compilation

Prérequis : Rust stable et Node 18+. Sous Arch : `webkit2gtk-4.1 libayatana-appindicator`.

```bash
npm ci
```

```bash
npm run build
```

Ou, sans Node :

```bash
cd src-tauri && cargo build --release
```

Tests :

```bash
cd src-tauri && cargo test
```

```bash
npm run test:ui
```

### Publier une version

1. Incrémente la version dans `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `package.json` et `packaging/arch/PKGBUILD`.
2. Ajoute une section `## X.Y.Z` en tête de `CHANGELOG.md` : elle devient la note de version, affichée aussi dans Coquille.
3. Lance le workflow **Release** (onglet Actions › Release › Run workflow), ou pousse un tag `vX.Y.Z`.

Le workflow `.github/workflows/release.yml` :

1. relance les tests ;
2. compile pour Windows, macOS (Apple Silicon et Intel) et Linux ;
3. signe les fichiers de mise à jour et génère `latest.json` ;
4. **publie** la release une fois tous les builds réussis : les copies installées se mettent alors à jour.

Si un build échoue, rien n'est publié et la release reste en brouillon.

**Clé de signature (une seule fois).** Le secret `TAURI_SIGNING_PRIVATE_KEY` du dépôt contient la clé privée ; la clé publique correspondante est dans `tauri.conf.json` (`plugins.updater.pubkey`). Pour en créer une :

```bash
npx tauri signer generate -w ~/.tauri/coquille.key
```

Copie le contenu de `~/.tauri/coquille.key` dans *Settings › Secrets and variables › Actions › New repository secret* (nom : `TAURI_SIGNING_PRIVATE_KEY`). Si tu as choisi un mot de passe, ajoute-le dans `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Mets ensuite le contenu de `coquille.key.pub` dans `pubkey`.

> Garde une copie de la clé privée (gestionnaire de mots de passe). Sans elle, plus aucune mise à jour automatique n'est possible : il faudrait une nouvelle clé et réinstaller Coquille à la main partout.

### Structure

```
src/                 Interface (HTML/CSS/JS sans framework) ; bubble.* = fenêtre de la bulle
src-tauri/src/
  main.rs            Fenêtre, icône système, raccourci global, commandes
  langtool.rs        Client de l'API LanguageTool (/v2/check)
  grammalecte.rs     Client du serveur Grammalecte (/gc_text/fr)
  engines.rs         Interroge les deux moteurs en parallèle et fusionne leurs remarques
  selection.rs       Capture et remplacement de la sélection, par OS
  bubble.rs          Bulle en direct (Windows, UI Automation)
  config.rs          Paramètres (JSON)
  updates.rs         Mises à jour automatiques (vérification, téléchargement, installation)
server/              docker-compose LanguageTool + Grammalecte, scripts d'installation (Proxmox/Debian), n-grammes FR
packaging/arch/      PKGBUILD + fichier .desktop
tests/ui/            Tests de l'interface (Playwright, backend simulé)
CHANGELOG.md         Notes de version (une section par version, reprise dans la release)
```

Licence MIT.
