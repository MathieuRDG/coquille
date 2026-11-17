# Nouveautés de Coquille

Chaque section `## <version>` devient la note de la release GitHub, et s'affiche dans Coquille
(⚙ › Mises à jour › Nouveautés) quand la version est proposée.

## 0.4.0 — 2026-11-17

- **Mises à jour automatiques.** Coquille vérifie au démarrage puis toutes les 6 heures s'il existe une nouvelle version, la télécharge en arrière-plan et l'installe quand sa fenêtre est fermée et que l'ordinateur n'est pas utilisé (2 minutes sans clavier ni souris sous Windows). Elle redémarre seule et garde tes réglages. Cette installation est donc la dernière à faire à la main.
- Réglage ⚙ › Mises à jour : installation automatique (activée par défaut), bouton « Rechercher » et bouton « Installer et redémarrer ».
- Paquets `.deb` et `.rpm` : l'installation demande le mot de passe administrateur, elle se fait donc sur ton clic. Paquet Arch compilé : Coquille signale seulement la nouvelle version.
- Les mises à jour sont signées : Coquille refuse un fichier dont la signature ne correspond pas.
- **Diagnostic de la bulle** (⚙ › Bulle de correction en direct › Diagnostic) : indique ce que Coquille voit du champ actif (application, type de contrôle, rôle ARIA, lecture seule, nombre de mots) et pourquoi la bulle ne s'affiche pas. « Copier le diagnostic » permet de le transmettre.

## 0.3.0

- Grammalecte comme second moteur de correction (français), en plus de LanguageTool, sans doublons.

## 0.2.2

- Bulle : retrouve le mot à corriger par recherche dans les éditeurs riches.

## 0.2.1

- Garde l'analyse d'origine après chaque correction.

## 0.2.0

- Bulle de correction en direct (Windows) et raccourci global réparé.

## 0.1.0

- Première version : correction de la sélection dans n'importe quelle application avec un raccourci global, LanguageTool public ou auto-hébergé.
