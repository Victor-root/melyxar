#!/usr/bin/env bash
set -Eeuo pipefail
umask 022

# ──────────────────────────────────────────────────────────────────────────────
# Melyxar media server: install, update and management.
# The script speaks the language of the system, English or French.
# ──────────────────────────────────────────────────────────────────────────────

REPO_URL="https://github.com/Victor-root/melyxar.git"
BRANCH="develop"
# This script on its own, without the repository around it. Running it from
# here is what keeps it current: a copy saved on the machine is a copy that
# stops being the published one the day it is changed, and nothing says so.
SCRIPT_URL="https://raw.githubusercontent.com/Victor-root/melyxar/${BRANCH}/scripts/melyxar.sh"

APP_USER="melyxar"
APP_GROUP="melyxar"
SOURCE_DIR="/opt/melyxar/source"
BINARY_PATH="/usr/local/bin/melyxar"
# Where the server reads the interface from, which is its own default: the
# server only reads it, and serves what is put there from the next request.
INTERFACE_DIR="/usr/local/share/melyxar/web"
# The commit each part was last installed from, and the files the interface
# in place is made of. Written only once a part is in place, so an update
# that stopped halfway is redone rather than taken for done.
INSTALLED_DIR="/opt/melyxar/installed"
CONFIG_DIR="/etc/melyxar"
CONFIG_FILE="${CONFIG_DIR}/melyxar.toml"
DATA_DIR="/var/lib/melyxar"
CACHE_DIR="/var/cache/melyxar"
BACKUP_DIR="${DATA_DIR}/backups"
UNIT_FILE="/etc/systemd/system/melyxar.service"
# Present only when the media folders were opened for writing.
WRITES_FILE="/etc/systemd/system/melyxar.service.d/media-write.conf"
SERVICE="melyxar"

DEFAULT_PORT="2100"
# Backups kept before the oldest is dropped.
BACKUP_KEEP="7"

# How long the Rust toolchain is given to arrive before it is called dead.
#
# It is a couple of hundred megabytes over one connection, so several minutes
# is ordinary and a quarter of an hour is not alarming. Past that it is not
# slow, it is stopped, and saying so beats a screen that waits for ever.
RUST_MINUTES="20"

# Which line of Node the interface is built with.
#
# The long term line, and its newest release rather than a version written
# down here: a version written down is a version that stops being current the
# week after, without anything saying so. Raised only when that line is
# retired in its turn.
NODE_MAJOR="24"

# Where Node is unpacked. Under /opt because it belongs to nothing the
# distribution manages, and its commands are linked into /usr/local/bin,
# which every shell already looks in before /usr/bin.
NODE_PREFIX="/opt/node"

# Whether every command shows its own output as it goes.
#
# On while Melyxar is being built, because "it did not work" with nothing to
# show is not a bug report, and a step that succeeds can still have said
# something worth reading. To be turned off once the thing is finished, when a
# tidy screen is what somebody installing it wants. --quiet does it for one
# run, MELYXAR_VERBOSE=0 in the environment for good.
VERBOSE="${MELYXAR_VERBOSE:-1}"

# ── Language ──────────────────────────────────────────────────────────────────
# The script speaks the language of the system, English by default. Same
# mechanism as the other scripts of the author: the locale decides, and English
# is the fallback for an unknown locale as well as for a missing key.

detect_lang() {
  local raw="${LC_ALL:-${LC_MESSAGES:-${LANG:-en}}}"
  raw="${raw,,}"
  case "$raw" in
    fr* ) APP_LANG="fr" ;;
    * ) APP_LANG="en" ;;
  esac
}

APP_LANG="en"
detect_lang

declare -A I18N

load_i18n() {
  local lang key text
  while IFS='|' read -r lang key text; do
    [[ -z "${lang:-}" || -z "${key:-}" ]] && continue
    I18N["$lang:$key"]="$text"
  done <<'I18N_DATA'
en|app_name|Melyxar server installation and management
fr|app_name|Installation et gestion du serveur Melyxar
en|err_need_root|Run this script as root.
fr|err_need_root|Exécutez ce script avec les droits root.
en|err_command_output|Command output:
fr|err_command_output|Sortie de la commande :
en|verbose_on|Detailed output enabled. Each command will display its output.
fr|verbose_on|Affichage détaillé activé. La sortie de chaque commande sera affichée.
en|err_aborted|Operation stopped at line %s. Review the output above to identify any completed steps.
fr|err_aborted|Opération interrompue à la ligne %s. Consultez la sortie ci-dessus pour identifier les étapes déjà effectuées.
en|err_missing_command|Missing command: %s
fr|err_missing_command|Commande manquante : %s
en|hint_yes_default|Enter = yes / no
fr|hint_yes_default|Entrée = oui / non
en|hint_no_default|yes / Enter = no
fr|hint_no_default|oui / Entrée = non
en|menu_title|Choose an action
fr|menu_title|Choisissez une action
en|menu_install|Install Melyxar
fr|menu_install|Installer Melyxar
en|menu_update|Update Melyxar
fr|menu_update|Mettre Melyxar à jour
en|menu_status|Show server status
fr|menu_status|Afficher l’état du serveur
en|menu_backup|Back up the database now
fr|menu_backup|Sauvegarder la base de données
en|menu_restore|Restore from a backup
fr|menu_restore|Restaurer depuis une sauvegarde
en|menu_uninstall|Uninstall
fr|menu_uninstall|Désinstaller
en|menu_bench|Benchmark a large library
fr|menu_bench|Tester les performances d’une grande bibliothèque
en|menu_lines|Count the lines of code
fr|menu_lines|Compter les lignes de code
en|menu_accounts|View accounts or reset a password
fr|menu_accounts|Voir les comptes ou réinitialiser un mot de passe
en|section_accounts|Accounts
fr|section_accounts|Comptes
en|accounts_notice|Reset the password of an existing account from this server. Root access is required; no additional password is requested.
fr|accounts_notice|Réinitialisez le mot de passe d’un compte depuis ce serveur. Les droits root sont nécessaires ; aucun autre mot de passe n’est demandé.
en|accounts_none|No account exists yet. Open Melyxar in a browser to create the first one.
fr|accounts_none|Aucun compte n’existe encore. Ouvrez Melyxar dans un navigateur pour créer le premier compte.
en|prompt_accounts_which|Account to reset (leave empty to cancel)
fr|prompt_accounts_which|Compte à réinitialiser (laisser vide pour annuler)
en|prompt_accounts_password|New password
fr|prompt_accounts_password|Nouveau mot de passe
en|accounts_changed|Password updated. All devices signed in to this account have been signed out.
fr|accounts_changed|Mot de passe modifié. Tous les appareils connectés à ce compte ont été déconnectés.
en|menu_writes|Manage write access to media folders
fr|menu_writes|Gérer l’accès en écriture aux dossiers multimédias
en|section_writes|Media folder write access
fr|section_writes|Accès en écriture aux dossiers multimédias
en|writes_notice|Melyxar has read-only access to media by default. Grant write access to enable deletion of media files from disk. Access to other folders is unchanged.
fr|writes_notice|Melyxar accède aux médias en lecture seule par défaut. Autorisez l’écriture pour permettre la suppression des fichiers du disque. L’accès aux autres dossiers reste inchangé.
en|writes_half_open|Write access is enabled in Melyxar, but %s folders still deny it.
fr|writes_half_open|L’écriture est autorisée dans Melyxar, mais %s dossiers refusent encore l’accès.
en|writes_now_shut|Melyxar currently has read-only access to media folders.
fr|writes_now_shut|Melyxar dispose actuellement d’un accès en lecture seule aux dossiers multimédias.
en|prompt_writes_open|Allow Melyxar to write to media folders?
fr|prompt_writes_open|Autoriser Melyxar à écrire dans les dossiers multimédias ?
en|prompt_writes_shut|Restore read-only access to media folders?
fr|prompt_writes_shut|Rétablir l’accès en lecture seule aux dossiers multimédias ?
en|writes_no_folder|No media folder is configured. Add a library first.
fr|writes_no_folder|Aucun dossier multimédia n’est configuré. Ajoutez d’abord une bibliothèque.
en|writes_opened|Melyxar now has write access to these folders.
fr|writes_opened|Melyxar peut maintenant écrire dans ces dossiers.
en|writes_shut|Melyxar now has read-only access to media folders.
fr|writes_shut|Melyxar dispose désormais d’un accès en lecture seule aux dossiers multimédias.
en|writes_all_fine|Melyxar can write to every media folder.
fr|writes_all_fine|Melyxar peut écrire dans tous les dossiers multimédias.
en|writes_group_offer|These folders grant write access to group %s. Adding the melyxar account to that group does not change folder ownership or permissions.
fr|writes_group_offer|Ces dossiers accordent l’écriture au groupe %s. Ajouter le compte melyxar à ce groupe ne modifie ni les propriétaires ni les droits des dossiers.
en|prompt_writes_group|Add the melyxar account to this group?
fr|prompt_writes_group|Ajouter le compte melyxar à ce groupe ?
en|step_writes_group|Adding melyxar to group %s
fr|step_writes_group|Ajout de melyxar au groupe %s
en|writes_unmapped|%s belongs to a group unavailable in this container. Configure access on the Proxmox host.
fr|writes_unmapped|%s appartient à un groupe inaccessible depuis ce conteneur. Configurez l’accès sur l’hôte Proxmox.
en|writes_group_shut|%s: group write access is denied (owner %s, group %s, permissions %s). No changes were made to this folder.
fr|writes_group_shut|%s : le groupe ne dispose pas de l’accès en écriture (propriétaire %s, groupe %s, droits %s). Ce dossier n’a pas été modifié.
en|writes_partly|Write access enabled for %s of %s folders that denied it.
fr|writes_partly|Accès en écriture activé pour %s des %s dossiers qui le refusaient.
en|writes_still_refused|%s still denies write access to Melyxar.
fr|writes_still_refused|%s refuse toujours l’accès en écriture à Melyxar.
en|step_writes|Enabling write access to media folders
fr|step_writes|Activation de l’accès en écriture aux dossiers multimédias
en|menu_quit|Quit
fr|menu_quit|Quitter
en|prompt_choice|Your choice
fr|prompt_choice|Votre choix
en|err_bad_choice|Unknown choice: %s
fr|err_bad_choice|Choix inconnu : %s
en|section_checks|Checking the system
fr|section_checks|Vérification du système
en|check_debian_ok|Debian-based system detected
fr|check_debian_ok|Système basé sur Debian détecté
en|check_debian_warn|This script is designed for Debian-based systems. Continuing anyway.
fr|check_debian_warn|Ce script est conçu pour les systèmes basés sur Debian. Poursuite malgré cet avertissement.
en|check_container|Running inside a container
fr|check_container|Exécution dans un conteneur
en|check_memory|Memory available: %s MB
fr|check_memory|Mémoire disponible : %s Mo
en|warn_memory_low|Building may require 6 to 8 GB of memory. The process may fail if less is available.
fr|warn_memory_low|La compilation peut nécessiter 6 à 8 Go de mémoire. Elle risque d’échouer si la mémoire disponible est insuffisante.
en|check_disk|Free space on the system disk: %s GB
fr|check_disk|Espace libre sur le disque système : %s Go
en|warn_disk_low|Building requires approximately 20 GB of free space for temporary files.
fr|warn_disk_low|La compilation nécessite environ 20 Go d’espace libre pour les fichiers temporaires.
en|check_port_free|Port %s is free
fr|check_port_free|Le port %s est libre
en|check_port_busy|Port %s is already in use
fr|check_port_busy|Le port %s est déjà utilisé
en|check_ffmpeg_ok|Media tools available: %s
fr|check_ffmpeg_ok|Outils multimédias disponibles : %s
en|check_ffmpeg_missing|Media tools not found. They will be installed.
fr|check_ffmpeg_missing|Outils multimédias absents. Ils seront installés.
en|check_dri_ok|Graphics device detected. Hardware acceleration may be available.
fr|check_dri_ok|Périphérique graphique détecté. L’accélération matérielle pourrait être disponible.
en|check_dri_missing|No graphics device detected
fr|check_dri_missing|Aucun périphérique graphique détecté
en|dri_title|Graphics device required for hardware acceleration
fr|dri_title|Périphérique graphique nécessaire à l’accélération matérielle
en|dri_line1|A GPU can accelerate video transcoding.
fr|dri_line1|Une carte graphique peut accélérer le transcodage vidéo.
en|dri_line2|This reduces CPU use compared with software transcoding.
fr|dri_line2|Elle réduit l’utilisation du processeur par rapport au transcodage logiciel.
en|dri_line3|For an unprivileged container, pass the graphics device through from the host.
fr|dri_line3|Dans un conteneur non privilégié, le périphérique graphique doit être transmis depuis l’hôte.
en|dri_line4|Configure this on the host, outside the container.
fr|dri_line4|Effectuez cette configuration sur l’hôte, hors du conteneur.
en|dri_commands|Commands to run on the Proxmox host:
fr|dri_commands|Commandes à exécuter sur l’hôte Proxmox :
en|dri_replace|Replace <CTID> with the container ID.
fr|dri_replace|Remplacez <CTID> par l’identifiant du conteneur.
en|dri_later|You can complete installation now and configure the graphics device later.
fr|dri_later|Vous pouvez terminer l’installation maintenant et configurer le périphérique graphique plus tard.
en|section_packages|Installing dependencies
fr|section_packages|Installation des dépendances
en|step_apt_update|Updating package lists
fr|step_apt_update|Mise à jour de la liste des paquets
en|step_apt_install|Installing build and media tools
fr|step_apt_install|Installation des outils de compilation et multimédias
en|step_rust|Installing the Rust toolchain
fr|step_rust|Installation de la chaîne d'outils Rust
en|step_node|Installing Node.js %s
fr|step_node|Installation de Node.js %s
en|step_node_distribution|Installing Node.js from distribution packages
fr|step_node_distribution|Installation de Node.js depuis les paquets de la distribution
en|node_present|Node.js is up to date: %s
fr|node_present|Node.js est à jour : %s
en|node_arch_unknown|Unsupported processor architecture: %s. Node.js cannot be downloaded.
fr|node_arch_unknown|Architecture de processeur non prise en charge : %s. Impossible de télécharger Node.js.
en|node_unreachable|Could not reach nodejs.org.
fr|node_unreachable|Impossible de joindre nodejs.org.
en|node_bad_sum|The downloaded archive failed checksum verification and was discarded.
fr|node_bad_sum|L’archive téléchargée ne correspond pas à la somme de contrôle publiée et a été supprimée.
en|node_unpack_failed|Could not extract the archive. The existing installation was preserved.
fr|node_unpack_failed|Impossible d’extraire l’archive. L’installation existante a été conservée.
en|node_installed|Node.js %s installed
fr|node_installed|Node.js %s installé
en|node_kept|Keeping the installed Node.js version: %s
fr|node_kept|Version de Node.js déjà installée conservée : %s
en|node_fallback|Using the distribution’s Node.js package instead. It may be older than the version required by the web build tools, which can produce warnings during compilation.
fr|node_fallback|Utilisation de la version de Node.js fournie par Debian. Elle peut être plus ancienne que celle requise par les outils de compilation web, d’où d’éventuels avertissements.
en|step_npm_install|Installing web dependencies
fr|step_npm_install|Installation des dépendances web
en|step_npm_build|Building the web interface
fr|step_npm_build|Compilation de l'interface web
en|rust_present|Rust toolchain already present: %s
fr|rust_present|Chaîne d'outils Rust déjà présente : %s
en|rust_long|The Rust toolchain download is several hundred megabytes and may take a few minutes. Progress appears below.
fr|rust_long|Le téléchargement des outils Rust représente plusieurs centaines de mégaoctets et peut durer quelques minutes. La progression s’affiche ci-dessous.
en|rust_unreachable|Could not reach rustup. Check the container’s network connection and try again.
fr|rust_unreachable|Impossible de joindre rustup. Vérifiez la connexion réseau du conteneur, puis réessayez.
en|rust_too_long|Rust toolchain download timed out after %s minutes. Retry this action; the incomplete download will be discarded.
fr|rust_too_long|Le téléchargement des outils Rust a dépassé le délai de %s minutes. Relancez cette action ; le téléchargement incomplet sera supprimé.
en|section_account|Preparing the system account and folders
fr|section_account|Préparation du compte système et des dossiers
en|step_user|Creating the system account
fr|step_user|Création du compte système
en|user_present|System account already present
fr|user_present|Compte système déjà présent
en|step_dirs|Creating the folders
fr|step_dirs|Création des dossiers
en|section_source|Retrieving source code
fr|section_source|Récupération du code source
en|step_clone|Cloning the complete repository
fr|step_clone|Clonage du dépôt complet
en|step_pull|Fetching the latest changes
fr|step_pull|Récupération des dernières modifications
en|step_unshallow|Completing the repository history
fr|step_unshallow|Récupération de l’historique complet du dépôt
en|section_update_diff|Pending changes
fr|section_update_diff|Modifications à installer
en|update_diff_up_to_date|Already up to date.
fr|update_diff_up_to_date|Déjà à jour.
en|section_lines|Code statistics
fr|section_lines|Statistiques du code
en|step_fetch_main|Fetching branch %s
fr|step_fetch_main|Récupération de la branche %s
en|lines_on|%s: %s lines
fr|lines_on|%s : %s lignes
en|lines_ahead|%s has %s more lines than %s
fr|lines_ahead|%s compte %s lignes de plus que %s
en|lines_behind|%s has %s fewer lines than %s
fr|lines_behind|%s compte %s lignes de moins que %s
en|lines_same|%s and %s have the same number of lines
fr|lines_same|%s et %s ont le même nombre de lignes
en|lines_changed|%s lines added and %s removed in %s files
fr|lines_changed|%s lignes ajoutées et %s supprimées dans %s fichiers
en|section_bench|Performance benchmark
fr|section_bench|Test de performances
en|bench_notice|The benchmark creates a temporary library of generated titles in the database, measures typical browsing requests, then removes the test data. Your media files are not modified.
fr|bench_notice|Le test crée temporairement une bibliothèque de titres générés dans la base de données, mesure des requêtes de navigation courantes, puis supprime ces données. Vos fichiers multimédias ne sont pas modifiés.
en|bench_takes|Allow about five minutes. The temporary data may use roughly 1 GB in the database.
fr|bench_takes|Prévoyez environ cinq minutes. Les données temporaires peuvent occuper près de 1 Go dans la base de données.
en|prompt_bench_works|Number of titles to generate
fr|prompt_bench_works|Nombre de titres à générer
en|prompt_bench_account|Account used for the benchmark
fr|prompt_bench_account|Compte utilisé pour le test
en|prompt_bench_password|Account password
fr|prompt_bench_password|Mot de passe du compte
en|bench_signs_in|The benchmark signs in like a browser to measure authenticated requests.
fr|bench_signs_in|Le test se connecte comme un navigateur pour mesurer les requêtes authentifiées.
en|err_bench_account|Select an account for the benchmark.
fr|err_bench_account|Sélectionnez un compte pour le test.
en|err_bench_works|Invalid number of titles: %s
fr|err_bench_works|Nombre de titres invalide : %s
en|bench_needs_server|The server must be running to measure its performance. Start it first.
fr|bench_needs_server|Le serveur doit être en cours d’exécution pour mesurer ses performances. Démarrez-le d’abord.
en|step_bench_clear|Clearing data from an earlier benchmark
fr|step_bench_clear|Suppression des données d’un test précédent
en|step_bench_fill|Generating the test library
fr|step_bench_fill|Génération de la bibliothèque de test
en|bench_measuring|Measuring performance. The table below compares each request with its target time.
fr|bench_measuring|Mesure des performances en cours. Le tableau ci-dessous compare chaque requête à son temps cible.
en|step_bench_empty|Removing the test library
fr|step_bench_empty|Suppression de la bibliothèque de test
en|bench_held|All performance targets were met at this library size.
fr|bench_held|Tous les objectifs de performance ont été atteints pour cette taille de bibliothèque.
en|bench_missed|At least one performance target was missed. See the table above for details.
fr|bench_missed|Au moins un objectif de performance n’a pas été atteint. Consultez le tableau ci-dessus.
en|lines_counted|Lockfiles and images are excluded from the count.
fr|lines_counted|Les fichiers de verrouillage et les images sont exclus du décompte.
en|section_build|Building
fr|section_build|Compilation
en|build_notice|The first build usually takes 5 to 10 minutes. Later builds usually take 1 to 2 minutes.
fr|build_notice|La première compilation prend généralement 5 à 10 minutes, puis 1 à 2 minutes pour les suivantes.
en|build_nice|Building at high priority to reduce compilation time.
fr|build_nice|Compilation avec une priorité élevée pour réduire sa durée.
en|build_nice_denied|This container cannot raise process priority. Building at normal priority.
fr|build_nice_denied|Ce conteneur ne permet pas d’augmenter la priorité du processus. Compilation à la priorité normale.
en|step_build|Building the server
fr|step_build|Compilation du serveur
en|step_install_binary|Installing the server executable
fr|step_install_binary|Installation de l’exécutable du serveur
en|step_install_interface|Deploying the web interface
fr|step_install_interface|Déploiement de l’interface web
en|build_nothing|Server and web interface are already up to date. No build needed.
fr|build_nothing|Le serveur et l’interface web sont déjà à jour. Aucune compilation nécessaire.
en|build_interface_only|Only the web interface changed. It will be rebuilt and deployed without restarting the server.
fr|build_interface_only|Seule l’interface web a changé. Elle sera recompilée et déployée sans redémarrer le serveur.
en|build_engine_only|Only the server changed. The web interface will remain unchanged.
fr|build_engine_only|Seul le serveur a changé. L’interface web restera inchangée.
en|section_config|Configuration
fr|section_config|Configuration
en|config_present|Existing configuration file preserved
fr|config_present|Fichier de configuration existant conservé
en|step_config|Writing the initial configuration
fr|step_config|Écriture de la configuration initiale
en|prompt_port|Port for the Melyxar server
fr|prompt_port|Port du serveur Melyxar
en|prompt_add_library|Add a library now?
fr|prompt_add_library|Ajouter une bibliothèque maintenant ?
en|prompt_library_name|Library name
fr|prompt_library_name|Nom de la bibliothèque
en|prompt_library_kind|Library type (movies, series, anime, shows, home_media, music)
fr|prompt_library_kind|Type de bibliothèque (movies, series, anime, shows, home_media, music)
en|prompt_root_path|Media folder path (leave empty to finish)
fr|prompt_root_path|Chemin du dossier multimédia (laisser vide pour terminer)
en|prompt_root_label|Short folder label shown in logs
fr|prompt_root_label|Libellé court du dossier affiché dans les journaux
en|root_added|Folder added: %s
fr|root_added|Dossier ajouté : %s
en|root_missing|This folder does not exist. Add it anyway?
fr|root_missing|Ce dossier n’existe pas. L’ajouter quand même ?
en|root_unreadable|This folder exists, but the server account cannot read it.
fr|root_unreadable|Ce dossier existe, mais le compte du serveur ne peut pas le lire.
en|root_not_absolute|Enter an absolute folder path starting with /.
fr|root_not_absolute|Saisissez un chemin absolu commençant par /.
en|err_bad_kind|Unsupported library type. Choose: movies, series, anime, shows, home_media or music.
fr|err_bad_kind|Type de bibliothèque non pris en charge. Choisissez : movies, series, anime, shows, home_media ou music.
en|library_without_root|No folder was provided, so the library was not created. You can add it later from the web interface.
fr|library_without_root|Aucun dossier n’a été indiqué ; la bibliothèque n’a pas été créée. Vous pourrez l’ajouter depuis l’interface web.
en|section_service|Service
fr|section_service|Service
en|step_unit|Installing the service file
fr|step_unit|Installation du fichier de service
en|step_enable|Enabling the service at boot
fr|step_enable|Activation du service au démarrage
en|step_restart|Restarting the service
fr|step_restart|Redémarrage du service
en|step_stop|Stopping the service
fr|step_stop|Arrêt du service
en|section_done|Ready
fr|section_done|Prêt
en|done_title|Melyxar is running
fr|done_title|Melyxar est en cours d’exécution
en|done_open|Open Melyxar at:
fr|done_open|Ouvrez Melyxar à l’adresse :
en|done_config|Configuration file:
fr|done_config|Fichier de configuration :
en|done_logs|View live logs with:
fr|done_logs|Consultez le journal en direct avec :
en|done_report|View server status with:
fr|done_report|Consultez l’état du serveur avec :
en|done_again|Run the latest published version of this script with:
fr|done_again|Lancez la dernière version publiée de ce script avec :
en|section_backup|Backup
fr|section_backup|Sauvegarde
en|step_backup|Backing up the database
fr|step_backup|Sauvegarde de la base de données
en|backup_done|Database backup saved to %s
fr|backup_done|Sauvegarde de la base de données enregistrée dans %s
en|backup_none|No database is available to back up yet
fr|backup_none|Aucune base de données disponible à sauvegarder pour le moment
en|section_restore|Restore
fr|section_restore|Restauration
en|restore_none|No backup found in %s
fr|restore_none|Aucune sauvegarde trouvée dans %s
en|restore_pick|Select a backup to restore
fr|restore_pick|Sélectionnez la sauvegarde à restaurer
en|restore_confirm|This will replace the current database. A copy of it will be kept first. Continue?
fr|restore_confirm|La base de données actuelle sera remplacée. Une copie en sera d’abord conservée. Continuer ?
en|step_restore|Restoring the database backup
fr|step_restore|Restauration de la sauvegarde de la base de données
en|restore_done|Database restored. The previous version was saved to %s
fr|restore_done|Base de données restaurée. La version précédente a été enregistrée dans %s
en|restore_done_fresh|Database restored. No previous database was available to preserve.
fr|restore_done_fresh|Base de données restaurée. Aucune version précédente n’était disponible à conserver.
en|section_uninstall|Uninstall
fr|section_uninstall|Désinstallation
en|uninstall_confirm|Remove the Melyxar service, executable and source code?
fr|uninstall_confirm|Supprimer le service Melyxar, l’exécutable et le code source ?
en|uninstall_keep_data|Library data and configuration can be preserved. Remove them as well?
fr|uninstall_keep_data|Les données des bibliothèques et la configuration peuvent être conservées. Les supprimer également ?
en|uninstall_data_kept|Data preserved in %s; configuration preserved in %s
fr|uninstall_data_kept|Données conservées dans %s ; configuration conservée dans %s
en|uninstall_done|Melyxar uninstalled
fr|uninstall_done|Melyxar désinstallé
en|err_not_installed|Melyxar is not installed. Install it first.
fr|err_not_installed|Melyxar n’est pas installé. Lancez d’abord l’installation.
en|err_no_terminal|No interactive terminal is available. Specify an action: install, update, status, backup, restore or uninstall.
fr|err_no_terminal|Aucun terminal interactif disponible. Indiquez une action : install, update, status, backup, restore ou uninstall.
en|cancelled|Cancelled
fr|cancelled|Annulé
I18N_DATA
}

load_i18n

tr_msg() {
  local key="$1"
  printf '%s\n' "${I18N["$APP_LANG:$key"]:-${I18N["en:$key"]:-$key}}"
}

# Same, for a message carrying values: the text holds %s placeholders.
tr_fmt() {
  local key="$1"
  shift
  # shellcheck disable=SC2059
  printf "${I18N["$APP_LANG:$key"]:-${I18N["en:$key"]:-$key}}" "$@"
}

APP_NAME="$(tr_msg app_name)"

# ── Colours ───────────────────────────────────────────────────────────────────

if [[ -t 1 ]] && [[ -z "${NO_COLOR:-}" ]]; then
  RED=$'\033[38;5;160m'
  RED_DARK=$'\033[38;5;124m'
  RED_SOFT=$'\033[38;5;203m'
  AMBER=$'\033[38;5;214m'
  GREEN=$'\033[38;5;120m'
  CYAN=$'\033[38;5;117m'
  GRAY=$'\033[38;5;245m'
  DIM=$'\033[2m'
  BOLD=$'\033[1m'
  RESET=$'\033[0m'
else
  RED=""
  RED_DARK=""
  RED_SOFT=""
  AMBER=""
  GREEN=""
  CYAN=""
  GRAY=""
  DIM=""
  BOLD=""
  RESET=""
fi

SPINNER_FRAMES=("⠋" "⠙" "⠹" "⠸" "⠼" "⠴" "⠦" "⠧" "⠇" "⠏")

# ── UI ────────────────────────────────────────────────────────────────────────

term_width() {
  local cols
  cols="$(tput cols 2>/dev/null || echo 80)"
  [[ -z "$cols" || "$cols" -lt 50 ]] && cols=80
  [[ "$cols" -gt 92 ]] && cols=92
  echo "$cols"
}

hr() {
  local cols line
  cols="$(term_width)"
  # Bash substitution instead of tr: tr works on bytes and would turn a
  # multi byte box character into garbage.
  printf -v line "%*s" "$cols" ""
  printf "%b%s%b\n" "${RED_DARK}" "${line// /─}" "${RESET}"
}

success() { printf "%b✓%b %b\n" "${GREEN}" "${RESET}" "$*"; }
info()    { printf "%b›%b %b\n" "${RED_SOFT}" "${RESET}" "$*"; }
warn()    { printf "%b⚠%b %b\n" "${AMBER}" "${RESET}" "$*"; }
error()   { printf "%b✗%b %b\n" "${RED}" "${RESET}" "$*" >&2; }
die()     { error "$*"; exit 1; }

# A step that fails stops the script, and saying so beats a silent exit.
on_error() {
  local rc=$?
  trap - ERR
  error "$(tr_fmt err_aborted "${BASH_LINENO[0]}")"
  exit "$rc"
}
trap on_error ERR

section() {
  echo
  printf "%b%s%b\n" "${BOLD}${RED}" "$1" "${RESET}"
  hr
}

panel() {
  local color="$1"
  local title="$2"
  shift 2

  echo
  printf "%b┌%b %b%b%s%b\n" "$color" "$RESET" "$BOLD" "$color" "$title" "$RESET"
  while (($#)); do
    printf "%b│%b %b\n" "$color" "$RESET" "$1"
    shift
  done
  printf "%b└%b\n" "$color" "$RESET"
}

banner() {
  clear 2>/dev/null || true
  echo
  # Le Y est rapproché du L de trois colonnes. Le L est creux en haut à
  # droite, le Y l'est en bas à gauche, et laissés à leur chasse naturelle les
  # deux creux se font face : sept colonnes de vide au plus large, là où les
  # autres lettres se touchent. Trois est le maximum, au-delà le pied du Y
  # entre dans celui du L.
  printf "%b%s%b\n" "${RED}" "███╗   ███╗███████╗██╗  ██╗   ██╗██╗  ██╗ █████╗ ██████╗ " "${RESET}"
  printf "%b%s%b\n" "${RED}" "████╗ ████║██╔════╝██║  ╚██╗ ██╔╝╚██╗██╔╝██╔══██╗██╔══██╗" "${RESET}"
  printf "%b%s%b\n" "${RED}" "██╔████╔██║█████╗  ██║   ╚████╔╝  ╚███╔╝ ███████║██████╔╝" "${RESET}"
  printf "%b%s%b\n" "${RED}" "██║╚██╔╝██║██╔══╝  ██║    ╚██╔╝   ██╔██╗ ██╔══██║██╔══██╗" "${RESET}"
  printf "%b%s%b\n" "${RED}" "██║ ╚═╝ ██║███████╗███████╗██║   ██╔╝ ██╗██║  ██║██║  ██║" "${RESET}"
  printf "%b%s%b\n" "${RED}" "╚═╝     ╚═╝╚══════╝╚══════╝╚═╝   ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝" "${RESET}"
  echo
  printf "  %b%s%b %b· by Victor-root%b\n" "${BOLD}${RED_SOFT}" "$APP_NAME" "${RESET}" "${GRAY}" "${RESET}"
  hr
}

step() {
  local label="$1"
  shift

  local log
  log="$(mktemp)"
  local rc=0

  if [[ "$VERBOSE" -eq 1 ]]; then
    # Everything on screen as it happens, the command included. No spinner:
    # it would fight with the output for the same line.
    printf "%b▸%b %s\n" "${RED_SOFT}" "${RESET}" "$label"
    printf "%b  $ %s%b\n" "${GRAY}" "$*" "${RESET}"
    # Indented so what a command says is never mistaken for what the script
    # says. The status read is the command's own, not the one the pipe ends on.
    set +e
    "$@" 2>&1 | tee "$log" | sed -e 's/^/    /'
    rc="${PIPESTATUS[0]}"
    set -e
    if [[ "$rc" -eq 0 ]]; then
      printf "%b✓%b %s\n" "${GREEN}" "${RESET}" "$label"
    else
      printf "%b✗%b %s\n" "${RED}" "${RESET}" "$label"
    fi
    # Already on screen, so it is not printed a second time below.
    rm -f "$log"
    return "$rc"
  fi

  if [[ -t 1 ]]; then
    "$@" >"$log" 2>&1 &
    local pid=$!
    local i=0
    while kill -0 "$pid" 2>/dev/null; do
      printf "\r%b%s%b %b%s…%b" \
        "${RED_SOFT}" "${SPINNER_FRAMES[i]}" "${RESET}" \
        "${DIM}" "$label" "${RESET}"
      i=$(((i + 1) % ${#SPINNER_FRAMES[@]}))
      sleep 0.08
    done
    wait "$pid" || rc=$?
    if [[ "$rc" -eq 0 ]]; then
      printf "\r\033[2K%b✓%b %s\n" "${GREEN}" "${RESET}" "$label"
    else
      printf "\r\033[2K%b✗%b %s\n" "${RED}" "${RESET}" "$label"
    fi
  else
    printf "%b▸%b %s\n" "${RED_SOFT}" "${RESET}" "$label"
    "$@" >"$log" 2>&1 || rc=$?
    if [[ "$rc" -eq 0 ]]; then
      printf "%b✓%b %s\n" "${GREEN}" "${RESET}" "$label"
    else
      printf "%b✗%b %s\n" "${RED}" "${RESET}" "$label"
    fi
  fi

  if [[ "$rc" -ne 0 ]]; then
    echo
    error "$(tr_msg err_command_output)"
    sed -e 's/^/  /' "$log" >&2
    rm -f "$log"
    return "$rc"
  fi

  rm -f "$log"
  return 0
}

prompt_label() {
  local color="$1"
  local label="$2"
  local hint="${3:-}"

  if [[ -n "$hint" ]]; then
    printf "%b?%b %b%s%b %b[%s]%b : " \
      "$color" "${RESET}" \
      "${BOLD}" "$label" "${RESET}" \
      "${DIM}" "$hint" "${RESET}" >&2
  else
    printf "%b?%b %b%s%b : " \
      "$color" "${RESET}" \
      "${BOLD}" "$label" "${RESET}" >&2
  fi
}

has_terminal() {
  [[ -t 0 ]] && return 0
  if { exec 3</dev/tty; } 2>/dev/null; then
    exec 3<&-
    return 0
  fi
  return 1
}

# The answer is read from the terminal when the script itself arrives on the
# standard input, and from nowhere at all in a scheduled job, where an end of
# file gives an empty answer and the default applies instead of the run
# failing.
read_answer() {
  local value=""

  if [[ -t 0 ]]; then
    IFS= read -r value || value=""
  elif { exec 3</dev/tty; } 2>/dev/null; then
    IFS= read -r value <&3 || value=""
    exec 3<&-
  fi

  printf "%s" "$value"
}

# The same, without showing what is typed. Read the same two ways, because
# this script is as often piped in from the network as it is run from a file,
# and in that case the answers come from the terminal rather than the pipe.
read_secret() {
  local value=""

  if [[ -t 0 ]]; then
    IFS= read -rs value || value=""
  elif { exec 3</dev/tty; } 2>/dev/null; then
    IFS= read -rs value <&3 || value=""
    exec 3<&-
  fi

  echo >&2
  printf "%s" "$value"
}

is_yes() {
  local value="${1,,}"
  [[ "$value" == "oui" || "$value" == "o" || "$value" == "yes" || "$value" == "y" ]]
}

prompt_default() {
  local value
  prompt_label "${RED_SOFT}" "$1" "$2"
  value="$(read_answer)"
  printf "%s" "${value:-$2}"
}

prompt_free() {
  prompt_label "${RED_SOFT}" "$1"
  read_answer
}

confirm_default_yes() {
  local value
  prompt_label "${AMBER}" "$1" "$(tr_msg hint_yes_default)"
  value="$(read_answer)"
  [[ -z "$value" ]] || is_yes "$value"
}

confirm_default_no() {
  local value
  prompt_label "${AMBER}" "$1" "$(tr_msg hint_no_default)"
  value="$(read_answer)"
  is_yes "$value"
}

need_root() {
  [[ "${EUID}" -eq 0 ]] || die "$(tr_msg err_need_root)"
}

need_command() {
  command -v "$1" >/dev/null 2>&1 || die "$(tr_fmt err_missing_command "$1")"
}

is_installed() {
  [[ -x "$BINARY_PATH" ]]
}

# A name or a path typed by hand can carry a character the configuration
# format gives a meaning to, and a file that no longer parses is a server that
# no longer starts.
toml_string() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  printf '"%s"' "$value"
}

# ── Checks ────────────────────────────────────────────────────────────────────

check_system() {
  section "$(tr_msg section_checks)"

  if [[ -f /etc/debian_version ]]; then
    success "$(tr_msg check_debian_ok)"
  else
    warn "$(tr_msg check_debian_warn)"
  fi

  if [[ -f /run/systemd/container ]] || grep -qa 'container=' /proc/1/environ 2>/dev/null; then
    info "$(tr_msg check_container)"
  fi

  local memory_mb
  memory_mb="$(awk '/MemTotal/ {printf "%d", $2/1024}' /proc/meminfo)"
  info "$(tr_fmt check_memory "$memory_mb")"
  if [[ "$memory_mb" -lt 6000 ]]; then
    warn "$(tr_msg warn_memory_low)"
  fi

  local disk_gb
  disk_gb="$(df -BG --output=avail / | tail -1 | tr -dc '0-9')"
  info "$(tr_fmt check_disk "$disk_gb")"
  if [[ "$disk_gb" -lt 20 ]]; then
    warn "$(tr_msg warn_disk_low)"
  fi

  if command -v ffmpeg >/dev/null 2>&1; then
    success "$(tr_fmt check_ffmpeg_ok "$(ffmpeg -version 2>/dev/null | head -1 | cut -d' ' -f1-3)")"
  else
    info "$(tr_msg check_ffmpeg_missing)"
  fi

  if [[ -d /dev/dri ]]; then
    success "$(tr_msg check_dri_ok)"
  else
    warn "$(tr_msg check_dri_missing)"
    panel "${AMBER}" "$(tr_msg dri_title)" \
      "$(tr_msg dri_line1)" \
      "$(tr_msg dri_line2)" \
      "" \
      "$(tr_msg dri_line3)" \
      "${BOLD}$(tr_msg dri_line4)${RESET}" \
      "" \
      "$(tr_msg dri_commands)" \
      "${CYAN}  pct set <CTID> --dev0 /dev/dri/renderD128${RESET}" \
      "${CYAN}  pct set <CTID> --dev1 /dev/dri/card0${RESET}" \
      "" \
      "$(tr_msg dri_replace)" \
      "$(tr_msg dri_later)"
  fi
}

check_port() {
  local port="$1"
  if command -v ss >/dev/null 2>&1 && ss -ltn 2>/dev/null | awk '{print $4}' | grep -qE "[:.]${port}\$"; then
    warn "$(tr_fmt check_port_busy "$port")"
    return 1
  fi
  success "$(tr_fmt check_port_free "$port")"
  return 0
}

# ── Install ───────────────────────────────────────────────────────────────────

install_packages() {
  section "$(tr_msg section_packages)"

  export DEBIAN_FRONTEND=noninteractive
  step "$(tr_msg step_apt_update)" apt-get update -qq

  step "$(tr_msg step_apt_install)" apt-get install -y --no-install-recommends \
    build-essential pkg-config git curl ca-certificates xz-utils ffmpeg sqlite3

  # Node comes from its own source rather than from the distribution, for the
  # reason written above install_node. It gives up only when this machine ends
  # up with no Node at all, and a build needs one rather than none: the
  # distribution's is taken then, saying why the build is about to complain.
  if ! step "$(tr_fmt step_node "$NODE_MAJOR")" install_node; then
    warn "$(tr_msg node_fallback)"
    step "$(tr_msg step_node_distribution)" apt-get install -y --no-install-recommends nodejs npm
  fi

  if command -v cargo >/dev/null 2>&1; then
    success "$(tr_fmt rust_present "$(cargo --version)")"
  else
    info "$(tr_msg rust_long)"
    step "$(tr_msg step_rust)" install_rust
    export PATH="/root/.cargo/bin:${PATH}"
  fi
}

# The Rust toolchain, from rustup.
#
# This is the one step of the whole install that spends minutes saying
# nothing, and it looked stopped every time. Three reasons, all dealt with
# here.
#
# The installer draws a progress bar only for a person, and everything this
# script runs talks to the script rather than to the screen. So it is given a
# terminal of its own, and the bar it then draws is the one thing on screen
# that proves the download is moving.
#
# It is fetched to a file and then run, rather than piped straight into a
# shell: a download cut off halfway down the pipe is half a script already
# being obeyed, which is the one way this step could fail into something
# worse than not running at all.
#
# And it is given a deadline. A connection that has really died is not slow,
# and waiting for ever on it is what sent the maintainer to the interrupt
# key. Past the deadline it stops and says what happened.
install_rust() {
  local installer rc=0 run
  installer="$(mktemp)"

  if ! curl --proto '=https' --tlsv1.2 -sSfL \
    --connect-timeout 20 --retry 3 --retry-delay 2 --retry-all-errors \
    https://sh.rustup.rs -o "$installer"; then
    rm -f "$installer"
    error "$(tr_msg rust_unreachable)"
    return 1
  fi

  run="timeout ${RUST_MINUTES}m sh '$installer' -y --no-modify-path --profile minimal"
  if command -v script >/dev/null 2>&1; then
    # -e gives back what the command returned rather than what script did,
    # -f writes each line through at once so the bar moves as it happens.
    script -qefc "$run" /dev/null || rc=$?
  else
    eval "$run" || rc=$?
  fi

  rm -f "$installer"
  if [[ "$rc" -eq 124 ]]; then
    error "$(tr_fmt rust_too_long "$RUST_MINUTES")"
  fi
  return "$rc"
}

# Node, from nodejs.org rather than from the distribution.
#
# Debian carries the Node that was current the day Debian froze, and keeps it
# for years: on Debian 13 it is a line that has already reached its own end of
# life, and the interface's own tools say so on every single build. Rust is
# already taken from rustup for exactly that reason, so Node is taken from its
# own source in the same spirit, and the distribution's copy is left where it
# is rather than fought with. What is unpacked under /opt is linked into
# /usr/local/bin, which every shell looks in before /usr/bin.
#
# Returning without having installed anything is not always a failure: a Node
# already there still builds the interface, badly, and that beats an update
# that stops dead because nodejs.org had a bad minute. It is a failure only
# when the machine has no Node at all, and the caller then falls back to the
# distribution's.
install_node() {
  local running rc=0
  running="$(node --version 2>/dev/null || true)"

  node_from_source "$running" || rc=$?
  if [[ "$rc" -eq 0 ]]; then
    return 0
  fi

  if [[ -n "$running" ]]; then
    warn "$(tr_fmt node_kept "$running")"
    return 0
  fi
  return 1
}

# The newest release of the wanted line, fetched and put in place.
#
# The list of checksums published beside that line answers both questions at
# once: which version is current, and whether what arrived is really it. So
# no version is written down anywhere, and nothing is unpacked that has not
# been checked first.
node_from_source() {
  local running="$1"
  local machine arch sums line sum file version dir

  machine="$(uname -m)"
  case "$machine" in
    x86_64 ) arch="x64" ;;
    aarch64 | arm64 ) arch="arm64" ;;
    armv7l ) arch="armv7l" ;;
    * )
      error "$(tr_fmt node_arch_unknown "$machine")"
      return 1
      ;;
  esac

  sums="$(mktemp)"
  if ! node_fetch "https://nodejs.org/dist/latest-v${NODE_MAJOR}.x/SHASUMS256.txt" "$sums"; then
    rm -f "$sums"
    error "$(tr_msg node_unreachable)"
    return 1
  fi

  line="$(grep -E "  node-v[0-9.]+-linux-${arch}\.tar\.xz\$" "$sums" || true)"
  rm -f "$sums"
  if [[ -z "$line" ]]; then
    error "$(tr_fmt node_arch_unknown "$machine")"
    return 1
  fi

  sum="${line%% *}"
  file="${line##* }"
  version="${file#node-}"
  version="${version%%-linux*}"

  if [[ "$running" == "$version" ]]; then
    success "$(tr_fmt node_present "$version")"
    return 0
  fi

  dir="$(mktemp -d)"
  if ! node_fetch "https://nodejs.org/dist/${version}/${file}" "${dir}/${file}"; then
    rm -rf "$dir"
    error "$(tr_msg node_unreachable)"
    return 1
  fi

  if ! printf '%s  %s\n' "$sum" "$file" |
    (cd "$dir" && sha256sum --check --status -); then
    rm -rf "$dir"
    error "$(tr_msg node_bad_sum)"
    return 1
  fi

  # Unpacked beside the one in use, and only put in its place once it is whole:
  # an archive that stops halfway must never leave the machine with half a Node
  # where its Node used to be.
  rm -rf "${NODE_PREFIX}.new"
  if ! (mkdir -p "${NODE_PREFIX}.new" &&
    tar -xJf "${dir}/${file}" -C "${NODE_PREFIX}.new" --strip-components=1); then
    rm -rf "$dir" "${NODE_PREFIX}.new"
    error "$(tr_msg node_unpack_failed)"
    return 1
  fi
  rm -rf "$dir" "$NODE_PREFIX"
  mv "${NODE_PREFIX}.new" "$NODE_PREFIX"

  ln -sfn "${NODE_PREFIX}/bin/node" /usr/local/bin/node
  ln -sfn "${NODE_PREFIX}/bin/npm" /usr/local/bin/npm
  ln -sfn "${NODE_PREFIX}/bin/npx" /usr/local/bin/npx
  success "$(tr_fmt node_installed "$version")"
}

# One download, with the same manners as the rest of the script: https only,
# a connection that refuses to open is retried, and a transfer that has gone
# to sleep is dropped rather than waited on for ever.
node_fetch() {
  curl --proto '=https' --tlsv1.2 -sSfL \
    --connect-timeout 20 --retry 3 --retry-delay 2 --retry-all-errors \
    --speed-limit 1024 --speed-time 60 \
    "$1" -o "$2"
}

create_account_and_folders() {
  section "$(tr_msg section_account)"

  if id -u "$APP_USER" >/dev/null 2>&1; then
    success "$(tr_msg user_present)"
  else
    step "$(tr_msg step_user)" useradd --system --user-group --create-home \
      --home-dir "$DATA_DIR" --shell /usr/sbin/nologin "$APP_USER"
  fi

  step "$(tr_msg step_dirs)" bash -c "
    mkdir -p '$CONFIG_DIR' '$DATA_DIR' '$CACHE_DIR' '$BACKUP_DIR' '$SOURCE_DIR'
    chown -R '$APP_USER:$APP_GROUP' '$DATA_DIR' '$CACHE_DIR'
    chmod 750 '$DATA_DIR' '$CACHE_DIR'
  "
}

fetch_source() {
  section "$(tr_msg section_source)"

  if [[ -d "${SOURCE_DIR}/.git" ]]; then
    # An earlier install may have left a shallow copy behind, from before this
    # script always kept the full history. Completed once here rather than
    # left as it is: a shallow copy cannot show a diff against a commit it
    # never kept, and every update after this one depends on that history
    # still being there.
    if [[ "$(git -C "$SOURCE_DIR" rev-parse --is-shallow-repository 2>/dev/null)" == "true" ]]; then
      step "$(tr_msg step_unshallow)" git -C "$SOURCE_DIR" fetch --unshallow origin "$BRANCH"
    else
      step "$(tr_msg step_pull)" git -C "$SOURCE_DIR" fetch origin "$BRANCH"
    fi
    step "$(tr_msg step_pull)" git -C "$SOURCE_DIR" reset --hard "origin/${BRANCH}"
  else
    rm -rf "$SOURCE_DIR"
    # No --depth: the full history of the branch, kept from here on, is what
    # lets an update show what actually changed rather than only its result.
    step "$(tr_msg step_clone)" git clone --branch "$BRANCH" "$REPO_URL" "$SOURCE_DIR"
  fi
}

# ── What changed ──────────────────────────────────────────────────────────────
# Each part is rebuilt only when something it is made from changed since it
# was last installed. The interface is made from web/ alone; the server from
# everything else, apart from what neither is built from. Leaving a folder
# out of the server wrongly would install a stale server, so the list names
# what is left out rather than what is in: a folder added one day is counted
# as the server's until somebody says otherwise.
INTERFACE_SOURCES=(web)
ENGINE_SOURCES=(. ':(exclude)web' ':(exclude)docs' ':(exclude)scripts'
  ':(exclude)README.md' ':(exclude)CLAUDE.md' ':(exclude)LICENSE')

# Whether a part is still the one the source would build now.
part_is_current() {
  local part="$1"
  shift
  local record="${INSTALLED_DIR}/${part}"
  [[ -f "$record" ]] || return 1
  local installed
  installed="$(<"$record")"
  # A commit the source no longer holds says nothing about what changed.
  git -C "$SOURCE_DIR" cat-file -e "${installed}^{commit}" 2>/dev/null || return 1
  git -C "$SOURCE_DIR" diff --quiet "$installed" HEAD -- "$@"
}

mark_installed() {
  mkdir -p "$INSTALLED_DIR"
  git -C "$SOURCE_DIR" rev-parse HEAD > "${INSTALLED_DIR}/$1"
}

# Set by the build for whoever runs it next: what it put in place.
BUILT_ENGINE=0

build_and_install() {
  section "$(tr_msg section_build)"

  export PATH="/root/.cargo/bin:${PATH}"

  local engine_needed=1
  local interface_needed=1
  if [[ -x "$BINARY_PATH" ]] && part_is_current engine "${ENGINE_SOURCES[@]}"; then
    engine_needed=0
  fi
  if [[ -f "${INTERFACE_DIR}/index.html" ]] && part_is_current interface "${INTERFACE_SOURCES[@]}"; then
    interface_needed=0
  fi

  if [[ "$engine_needed" -eq 0 && "$interface_needed" -eq 0 ]]; then
    success "$(tr_msg build_nothing)"
    return 0
  fi
  if [[ "$engine_needed" -eq 0 ]]; then
    info "$(tr_msg build_interface_only)"
  elif [[ "$interface_needed" -eq 0 ]]; then
    info "$(tr_msg build_engine_only)"
  fi

  # Built before the server, and put in place after it: an interface put in
  # place first would be served for the minutes the server takes to build, to
  # a server that does not know yet what it asks for.
  if [[ "$interface_needed" -eq 1 ]]; then
    step "$(tr_msg step_npm_install)" npm --prefix "${SOURCE_DIR}/web" ci
    step "$(tr_msg step_npm_build)" npm --prefix "${SOURCE_DIR}/web" run build
  fi

  if [[ "$engine_needed" -eq 1 ]]; then
    info "$(tr_msg build_notice)"

    # Asked for, not assumed. Raising a priority needs a capability an
    # unprivileged container is not given, even as root inside it, and `nice`
    # then prints a refusal in the middle of the build and carries on at the
    # normal priority anyway. Trying it on something that does nothing tells us
    # which of the two messages to print, and leaves the build itself clean.
    local priority=()
    if nice -n -20 true 2>/dev/null; then
      priority=(nice -n -20)
      info "$(tr_msg build_nice)"
    else
      info "$(tr_msg build_nice_denied)"
    fi

    # Highest priority the processor allows, when this machine allows it at
    # all: this is still in active development, and waiting on a rebuild costs
    # more right now than a film playing at the same moment would.
    step "$(tr_msg step_build)" "${priority[@]}" cargo build --release --locked \
      --manifest-path "${SOURCE_DIR}/Cargo.toml"

    step "$(tr_msg step_install_binary)" bash -c "
      install -m 0755 '${SOURCE_DIR}/target/release/melyxar' '$BINARY_PATH'
    "
    BUILT_ENGINE=1
  fi

  if [[ "$interface_needed" -eq 1 ]]; then
    step "$(tr_msg step_install_interface)" install_interface
    # In place is live: the server reads it from the next request on.
    mark_installed interface
  fi
}

# Puts the built interface where the server reads it, while it serves.
#
# Each file is swapped in whole by a rename, never written over in place, so
# a file asked for in the middle of it is either the old one or the new one.
# The page goes last, so it never names a file that is not there yet. A file
# the update did not change is left alone with its date, which is what tells
# a browser it has not changed. The files of the interface before this one
# are kept one update more, for a tab still open on it that asks for the
# player only when somebody presses play.
install_interface() {
  local built="${SOURCE_DIR}/web/dist"
  # Beside the folder rather than in /tmp: a rename only swaps a file whole
  # within one file system.
  local staged="${INTERFACE_DIR}.incoming"
  local listed="${INSTALLED_DIR}/interface-files"

  [[ -f "${built}/index.html" ]] || return 1
  rm -rf "$staged" || return 1
  mkdir -p "$INTERFACE_DIR" "$INSTALLED_DIR" || return 1
  cp -R "$built" "$staged" || return 1
  chmod -R u=rwX,go=rX "$staged" || return 1

  local name
  while IFS= read -r -d '' name; do
    name="${name#./}"
    if [[ "$name" != "index.html" ]]; then
      swap_in "$staged" "$name" || return 1
    fi
  done < <(cd "$staged" && find . -type f -print0)
  swap_in "$staged" index.html || return 1

  # What this interface and the one before it are made of is kept, anything
  # older goes.
  local now kept
  now="$(mktemp)"
  kept="$(mktemp)"
  (cd "$built" && find . -type f) | sed 's|^\./||' > "$now" || return 1
  cat "$now" > "$kept"
  if [[ -f "$listed" ]]; then
    cat "$listed" >> "$kept"
  fi
  while IFS= read -r -d '' name; do
    name="${name#./}"
    if ! grep -Fxq -- "$name" "$kept"; then
      rm -f "${INTERFACE_DIR}/${name}" || return 1
    fi
  done < <(cd "$INTERFACE_DIR" && find . -type f -print0)
  find "$INTERFACE_DIR" -mindepth 1 -type d -empty -delete || return 1

  mv -f "$now" "$listed" || return 1
  chmod 0644 "$listed"
  rm -f "$kept"
  rm -rf "$staged"
}

# One file of the staged interface put in place, unless it is already there.
swap_in() {
  local staged="$1"
  local name="$2"
  local target="${INTERFACE_DIR}/${name}"
  if [[ -f "$target" ]] && cmp -s "${staged}/${name}" "$target"; then
    return 0
  fi
  mkdir -p "$(dirname "$target")" || return 1
  mv -f "${staged}/${name}" "$target"
}

write_configuration() {
  section "$(tr_msg section_config)"

  if [[ -f "$CONFIG_FILE" ]]; then
    success "$(tr_msg config_present)"
    return 0
  fi

  local port
  port="$(prompt_default "$(tr_msg prompt_port)" "$DEFAULT_PORT")"
  check_port "$port" || true

  local tmp
  tmp="$(mktemp)"
  "$BINARY_PATH" print-default-config > "$tmp"
  sed -i "s/^port = .*/port = ${port}/" "$tmp"

  if confirm_default_yes "$(tr_msg prompt_add_library)"; then
    append_library "$tmp"
  fi

  step "$(tr_msg step_config)" bash -c "
    install -m 0640 -o root -g '$APP_GROUP' '$tmp' '$CONFIG_FILE'
    rm -f '$tmp'
  "
}

prompt_library_kind() {
  local kind
  while true; do
    kind="$(prompt_default "$(tr_msg prompt_library_kind)" "movies")"
    case "$kind" in
      movies | series | anime | shows | home_media | music )
        printf "%s" "$kind"
        return 0
        ;;
      * ) warn "$(tr_msg err_bad_kind)" >&2 ;;
    esac
  done
}

# A library is written only once it holds at least one folder: the server
# refuses a library that points nowhere, and discovering that when the service
# fails to start is too late to be of any use.
append_library() {
  local target="$1"
  local name kind path label
  local labels=() paths=()

  name="$(prompt_default "$(tr_msg prompt_library_name)" "Films")"
  kind="$(prompt_library_kind)"

  while true; do
    path="$(prompt_free "$(tr_msg prompt_root_path)")"
    [[ -z "$path" ]] && break

    if [[ "$path" != /* ]]; then
      warn "$(tr_msg root_not_absolute)"
      continue
    fi

    if [[ ! -d "$path" ]]; then
      confirm_default_no "$(tr_msg root_missing)" || continue
    elif ! runuser -u "$APP_USER" -- test -r "$path" 2>/dev/null; then
      warn "$(tr_msg root_unreadable)"
    fi

    label="$(prompt_default "$(tr_msg prompt_root_label)" "$(basename "$(dirname "$path")")")"
    labels+=("$label")
    paths+=("$path")
    success "$(tr_fmt root_added "$path")"
  done

  if [[ "${#paths[@]}" -eq 0 ]]; then
    warn "$(tr_msg library_without_root)"
    return 0
  fi

  {
    echo
    echo "[[libraries]]"
    echo "name = $(toml_string "$name")"
    echo "kind = $(toml_string "$kind")"
    echo "metadata_language = $(toml_string "$APP_LANG")"
    local index
    for index in "${!paths[@]}"; do
      echo
      echo "[[libraries.roots]]"
      echo "label = $(toml_string "${labels[index]}")"
      echo "path = $(toml_string "${paths[index]}")"
    done
  } >> "$target"
}

install_service() {
  section "$(tr_msg section_service)"

  local tmp
  tmp="$(mktemp)"
  cat > "$tmp" <<UNIT
[Unit]
Description=Melyxar media server
Documentation=https://github.com/Victor-root/melyxar
After=network-online.target
Wants=network-online.target

[Service]
Type=exec
User=${APP_USER}
Group=${APP_GROUP}
ExecStart=${BINARY_PATH} --config ${CONFIG_FILE} serve
Restart=on-failure
RestartSec=5

# Playback sessions own external processes. Stopping has to be given time to
# close them, otherwise they are left behind, which is the failure this whole
# project set out to avoid.
KillSignal=SIGTERM
TimeoutStopSec=30
KillMode=mixed

# The server needs nothing beyond its own folders and the media it is given.
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=${DATA_DIR} ${CACHE_DIR}

[Install]
WantedBy=multi-user.target
UNIT

  step "$(tr_msg step_unit)" bash -c "
    install -m 0644 '$tmp' '$UNIT_FILE'
    rm -f '$tmp'
    systemctl daemon-reload
  "
  step "$(tr_msg step_enable)" systemctl enable "$SERVICE"
  step "$(tr_msg step_restart)" systemctl restart "$SERVICE"
}

show_done() {
  local port address
  port="$(awk -F' *= *' '/^port/ {print $2; exit}' "$CONFIG_FILE" 2>/dev/null || echo "$DEFAULT_PORT")"
  address="$(hostname -I 2>/dev/null | awk '{print $1}')"
  [[ -z "$address" ]] && address="127.0.0.1"

  section "$(tr_msg section_done)"
  panel "${GREEN}" "$(tr_msg done_title)" \
    "$(tr_msg done_open)" \
    "${BOLD}${CYAN}  http://${address}:${port}${RESET}" \
    "" \
    "$(tr_msg done_config)" \
    "${GRAY}  ${CONFIG_FILE}${RESET}" \
    "" \
    "$(tr_msg done_logs)" \
    "${GRAY}  journalctl -u ${SERVICE} -f${RESET}" \
    "" \
    "$(tr_msg done_report)" \
    "${GRAY}  melyxar --config ${CONFIG_FILE} doctor${RESET}" \
    "" \
    "$(tr_msg done_again)" \
    "${GRAY}  bash <(curl -fsSL ${SCRIPT_URL})${RESET}"
}

action_install() {
  check_system
  install_packages
  create_account_and_folders
  fetch_source
  build_and_install
  write_configuration
  install_service
  mark_installed engine
  show_done
}

# What changed between the commit that was running and the one about to
# replace it: the file list first, in git's own shape, then one sentence with
# the totals in it. Nothing here needs the branch's history to reach back to
# any particular point, only that both commits are still present, which a full
# clone guarantees and a shallow one used not to.
show_update_diff() {
  local before="$1"
  local after="$2"

  # Nothing to compare against: the source was not there before this run, so
  # what is about to be built is not a change from anything, it is the first
  # copy of it.
  [[ -z "$before" ]] && return 0

  section "$(tr_msg section_update_diff)"

  if [[ "$before" == "$after" ]]; then
    info "$(tr_msg update_diff_up_to_date)"
    return 0
  fi

  local color_stat=()
  [[ -t 1 ]] && [[ -z "${NO_COLOR:-}" ]] && color_stat=(--color=always)

  # git's own table of files and bars needs no translation, being names and
  # graphics rather than words. Only its last line, the English summary
  # sentence, is dropped, and replaced below with a translated one built from
  # the same numbers the line-counting menu entry already knows how to add up.
  git -C "$SOURCE_DIR" diff --stat "${color_stat[@]}" "$before" "$after" |
    sed -e '$d' -e 's/^/  /'

  local moved=()
  read -r -a moved < <(
    git -C "$SOURCE_DIR" diff --numstat "$before" "$after" |
      awk '$1 != "-" { added += $1; removed += $2; files += 1 }
           END { printf "%d %d %d\n", added + 0, removed + 0, files + 0 }'
  )
  echo
  success "$(tr_fmt lines_changed "${moved[@]}")"
}

action_update() {
  is_installed || die "$(tr_msg err_not_installed)"
  # A backup before any change, so a failed update is never a lost library.
  action_backup quiet

  # A dependency added after somebody's own install is a dependency their
  # machine has never had a reason to fetch: checking again here, the same
  # way a fresh install does, is what lets an update reach it too instead of
  # failing partway into a build with nothing rolled back.
  install_packages

  local before
  before="$(git -C "$SOURCE_DIR" rev-parse HEAD 2>/dev/null || echo "")"
  fetch_source
  local after
  after="$(git -C "$SOURCE_DIR" rev-parse HEAD)"
  show_update_diff "$before" "$after"

  build_and_install
  # Only a new server needs a restart. A new interface alone is already being
  # served, and a film playing through the update is not cut.
  if [[ -f "$WRITES_FILE" ]]; then
    write_media_writes
  fi
  if [[ "$BUILT_ENGINE" -eq 1 ]]; then
    step "$(tr_msg step_restart)" systemctl restart "$SERVICE"
    mark_installed engine
  fi
  show_done
}

action_status() {
  is_installed || die "$(tr_msg err_not_installed)"
  echo
  "$BINARY_PATH" --config "$CONFIG_FILE" doctor 2>/dev/null
}

# What the branch being deployed is measured against.
COUNTED_AGAINST="main"

# What is written by hand rather than produced. The lock files and the
# pictures are large enough to drown everything else, and a count they take
# part in says nothing about the work.
LINES_LEFT_OUT=(
  ':!*.lock'
  ':!*.png'
  ':!*.webp'
  ':!*.svg'
  ':!*.ico'
)

# How many lines one branch is made of.
lines_written_on() {
  local ref="$1"
  # One line per file, ending in its count. A path can hold a colon, so the
  # count is read from the end rather than from a field number.
  git -C "$SOURCE_DIR" grep -I -c '^' "$ref" -- "${LINES_LEFT_OUT[@]}" |
    awk -F: '{ total += $NF } END { print total + 0 }'
}

# Everything the bench does, in the order somebody would do it by hand.
#
# Run as the account the server runs as, so the files the database leaves
# behind belong to it: the same commands run as root would leave a journal the
# service can no longer write to, and a server that no longer starts.
run_bench() {
  runuser -u "$APP_USER" -- "$BINARY_PATH" --config "$CONFIG_FILE" bench "$@"
}

action_bench() {
  is_installed || die "$(tr_msg err_not_installed)"
  systemctl is-active --quiet "$SERVICE" || die "$(tr_msg bench_needs_server)"

  section "$(tr_msg section_bench)"
  info "$(tr_msg bench_notice)"
  info "$(tr_msg bench_takes)"

  local works
  works="$(prompt_default "$(tr_msg prompt_bench_works)" "100000")"
  [[ "$works" =~ ^[1-9][0-9]*$ ]] || die "$(tr_fmt err_bench_works "$works")"

  # The measuring asks this server the pages a browser asks for, and this
  # server answers nothing to somebody who is not signed in.
  info "$(tr_msg bench_signs_in)"
  local account password
  account="$(prompt_free "$(tr_msg prompt_bench_account)")"
  [[ -n "$account" ]] || die "$(tr_msg err_bench_account)"
  prompt_label "${RED_SOFT}" "$(tr_msg prompt_bench_password)"
  password="$(read_secret)"

  # A run somebody stopped halfway leaves the invented library behind, and
  # filling refuses while one is there. Taking away whatever is left first is
  # what makes this work the second time as well as the first; with nothing to
  # take away it costs a moment and says so.
  step "$(tr_msg step_bench_clear)" run_bench empty
  step "$(tr_msg step_bench_fill)" run_bench fill --works "$works"

  # Not wrapped in a step: what the measuring prints is the whole point of
  # running it, and a spinner would hide the table behind one line.
  echo
  info "$(tr_msg bench_measuring)"
  echo
  local held=0
  # Through the standard input, so it never lands in this shell's history nor
  # in the list of what is running on this machine.
  printf '%s\n' "$password" | run_bench run --as "$account" || held=1
  echo

  # Removed whatever the measuring said, including when it was interrupted:
  # an invented library left behind is half a gigabyte nobody asked for, and
  # the next run would refuse because of it.
  step "$(tr_msg step_bench_empty)" run_bench empty

  if ((held == 0)); then
    success "$(tr_msg bench_held)"
  else
    warn "$(tr_msg bench_missed)"
  fi
}

action_accounts() {
  is_installed || die "$(tr_msg err_not_installed)"

  section "$(tr_msg section_accounts)"
  info "$(tr_msg accounts_notice)"
  echo

  local named
  named="$(runuser -u "$APP_USER" -- "$BINARY_PATH" --config "$CONFIG_FILE" account list 2>/dev/null)"
  if [[ -z "$named" ]]; then
    warn "$(tr_msg accounts_none)"
    return 0
  fi
  printf '%s\n' "$named" | sed 's/^/  /'
  echo

  local name password
  name="$(prompt_free "$(tr_msg prompt_accounts_which)")"
  [[ -n "$name" ]] || { info "$(tr_msg cancelled)"; return 0; }

  prompt_label "${RED_SOFT}" "$(tr_msg prompt_accounts_password)"
  password="$(read_secret)"
  [[ -n "$password" ]] || { info "$(tr_msg cancelled)"; return 0; }

  # Through the standard input, so it never lands in this shell's history nor
  # in the list of what is running on this machine.
  printf '%s\n' "$password" |
    runuser -u "$APP_USER" -- "$BINARY_PATH" --config "$CONFIG_FILE" account password "$name" ||
    die "$(tr_msg accounts_none)"
  success "$(tr_msg accounts_changed)"
}

# The folders of the libraries, one a line, as the server knows them. The
# server's log goes elsewhere and is left out.
library_folders() {
  runuser -u "$APP_USER" -- "$BINARY_PATH" --config "$CONFIG_FILE" folders 2>/dev/null
}

# Opens every folder of the libraries for writing, and only them. Written
# again on each update, so a library declared since is opened as well. A
# folder that is not there is skipped rather than stopping the server.
write_media_writes() {
  local folders line tmp
  folders="$(library_folders)"
  tmp="$(mktemp)"
  {
    echo "# Written by the installation script: Melyxar may write to its media."
    echo "[Service]"
    while IFS= read -r line; do
      [[ -n "$line" ]] || continue
      line="${line//\\/\\\\}"
      line="${line//\"/\\\"}"
      line="${line//%/%%}"
      printf 'ReadWritePaths="-%s"\n' "$line"
    done <<< "$folders"
  } > "$tmp"
  install -D -m 0644 "$tmp" "$WRITES_FILE"
  rm -f "$tmp"
  systemctl daemon-reload
}

# Whether the melyxar account itself may write in a folder, by trying, the
# way the server does. Asked of the account alone, outside the service: what
# the service allows is the other half, handled by the drop-in.
account_may_write() {
  local probe="$1/.melyxar-write-probe"
  if runuser -u "$APP_USER" -- touch "$probe" 2>/dev/null; then
    rm -f "$probe"
    return 0
  fi
  return 1
}

# The folders the melyxar account may not write in, one a line.
folders_refusing() {
  local folder
  while IFS= read -r folder; do
    [[ -d "$folder" ]] || continue
    account_may_write "$folder" || printf '%s\n' "$folder"
  done <<< "$1"
}

# A name for a group number the container has no name for yet: one that
# says it was made for Melyxar, so nobody later wonders where it came from.
# Not "melyxar" itself, which is already the account's own group.
name_for_group() {
  local gid="$1" name
  # Nothing found is an answer here, not a failure.
  name="$(getent group "$gid" | cut -d: -f1 || true)"
  if [[ -z "$name" ]]; then
    name="melyxar-medias"
    getent group "$name" >/dev/null && name="melyxar-medias-$gid"
    groupadd -g "$gid" "$name"
  fi
  printf '%s' "$name"
}

# Makes every refusing folder writable for Melyxar where that needs nothing
# but joining a group that already has the right: no owner, no permission of
# any folder is ever changed. What cannot be settled that way is said, folder
# by folder, with the reason.
settle_refusals() {
  local refused="$1" folder gid mode gids=() explained=0
  while IFS= read -r folder; do
    [[ -n "$folder" ]] || continue
    gid="$(stat -c '%g' "$folder")"
    mode="$(stat -c '%a' "$folder")"
    if [[ "$gid" -eq 65534 ]]; then
      # Owned by a group the container cannot see: only the Proxmox host
      # can open it, by mapping that group into the container.
      warn "$(tr_fmt writes_unmapped "$folder")"
      explained=1
    elif (( (8#$mode & 8#020) != 0 )) && ! id -G "$APP_USER" | tr ' ' '\n' | grep -qx "$gid"; then
      [[ " ${gids[*]} " == *" $gid "* ]] || gids+=("$gid")
    else
      warn "$(tr_fmt writes_group_shut "$folder" "$(stat -c '%U' "$folder")" "$(stat -c '%G' "$folder")" "$(stat -c '%A' "$folder")")"
      explained=1
    fi
  done <<< "$refused"

  if [[ "${#gids[@]}" -gt 0 ]]; then
    info "$(tr_fmt writes_group_offer "$(IFS=,; echo "${gids[*]}")")"
    if confirm_default_yes "$(tr_msg prompt_writes_group)"; then
      for gid in "${gids[@]}"; do
        local name
        name="$(name_for_group "$gid")"
        step "$(tr_fmt step_writes_group "$name")" usermod -aG "$name" "$APP_USER"
      done
      # The service takes its groups when it starts.
      step "$(tr_msg step_restart)" systemctl restart "$SERVICE"
    fi
  fi

  local still
  still="$(folders_refusing "$refused")"
  if [[ -z "$still" ]]; then
    success "$(tr_msg writes_opened)"
    return 0
  fi
  if [[ "$explained" -eq 0 ]]; then
    while IFS= read -r folder; do
      warn "$(tr_fmt writes_still_refused "$folder")"
    done <<< "$still"
  fi
  local before after
  before="$(grep -c . <<< "$refused" || true)"
  after="$(grep -c . <<< "$still" || true)"
  if [[ "$after" -lt "$before" ]]; then
    info "$(tr_fmt writes_partly "$((before - after))" "$before")"
  fi
}

action_writes() {
  is_installed || die "$(tr_msg err_not_installed)"
  section "$(tr_msg section_writes)"
  info "$(tr_msg writes_notice)"
  echo

  local folders refused
  folders="$(library_folders)"

  if [[ -f "$WRITES_FILE" ]]; then
    # Open on the service's side but refused somewhere: what is asked for is
    # to finish opening it, not to close it. Said only once it was tried, so
    # the screen never claims a right the folders still refuse.
    refused="$(folders_refusing "$folders")"
    if [[ -n "$refused" ]]; then
      info "$(tr_fmt writes_half_open "$(grep -c . <<< "$refused" || true)")"
      settle_refusals "$refused"
      return 0
    fi
    success "$(tr_msg writes_all_fine)"
    confirm_default_no "$(tr_msg prompt_writes_shut)" || { info "$(tr_msg cancelled)"; return 0; }
    rm -f "$WRITES_FILE"
    systemctl daemon-reload
    step "$(tr_msg step_restart)" systemctl restart "$SERVICE"
    success "$(tr_msg writes_shut)"
    return 0
  fi

  info "$(tr_msg writes_now_shut)"
  if [[ -z "$folders" ]]; then
    warn "$(tr_msg writes_no_folder)"
    return 0
  fi
  printf '%s\n' "$folders" | sed 's/^/  /'
  echo
  confirm_default_no "$(tr_msg prompt_writes_open)" || { info "$(tr_msg cancelled)"; return 0; }

  step "$(tr_msg step_writes)" write_media_writes
  step "$(tr_msg step_restart)" systemctl restart "$SERVICE"

  refused="$(folders_refusing "$folders")"
  if [[ -z "$refused" ]]; then
    success "$(tr_msg writes_opened)"
  else
    settle_refusals "$refused"
  fi
}

action_lines() {
  fetch_source

  if [[ "$BRANCH" == "$COUNTED_AGAINST" ]]; then
    section "$(tr_msg section_lines)"
    info "$(tr_fmt lines_on "$BRANCH" "$(lines_written_on "origin/${BRANCH}")")"
    info "$(tr_msg lines_counted)"
    return 0
  fi

  # The branch is cloned on its own, so the one it is compared with has to be
  # asked for by name and put where a comparison can find it.
  step "$(tr_fmt step_fetch_main "$COUNTED_AGAINST")" \
    git -C "$SOURCE_DIR" fetch --depth 1 origin \
    "+refs/heads/${COUNTED_AGAINST}:refs/remotes/origin/${COUNTED_AGAINST}"

  section "$(tr_msg section_lines)"

  local here there
  there="$(lines_written_on "origin/${COUNTED_AGAINST}")"
  here="$(lines_written_on "origin/${BRANCH}")"

  info "$(tr_fmt lines_on "$COUNTED_AGAINST" "$there")"
  info "$(tr_fmt lines_on "$BRANCH" "$here")"

  if ((here > there)); then
    success "$(tr_fmt lines_ahead "$BRANCH" "$((here - there))" "$COUNTED_AGAINST")"
  elif ((here < there)); then
    success "$(tr_fmt lines_behind "$BRANCH" "$((there - here))" "$COUNTED_AGAINST")"
  else
    success "$(tr_fmt lines_same "$BRANCH" "$COUNTED_AGAINST")"
  fi

  # What moved, which is a different question from how long each side is: a
  # line rewritten counts on both sides and changes neither total.
  local moved=()
  read -r -a moved < <(
    git -C "$SOURCE_DIR" diff --numstat \
      "origin/${COUNTED_AGAINST}" "origin/${BRANCH}" -- "${LINES_LEFT_OUT[@]}" |
      awk '$1 != "-" { added += $1; removed += $2; files += 1 }
           END { printf "%d %d %d\n", added + 0, removed + 0, files + 0 }'
  )
  info "$(tr_fmt lines_changed "${moved[@]}")"
  info "$(tr_msg lines_counted)"
}

action_backup() {
  local quiet="${1:-}"
  [[ "$quiet" != "quiet" ]] && section "$(tr_msg section_backup)"

  local database="${DATA_DIR}/melyxar.db"
  if [[ ! -f "$database" ]]; then
    warn "$(tr_msg backup_none)"
    return 0
  fi

  local stamp destination
  stamp="$(date +%Y%m%d-%H%M%S)"
  destination="${BACKUP_DIR}/melyxar-${stamp}.db"

  # Only the engine's own copy is consistent while the server writes, and a
  # plain copy of a database in write ahead mode leaves the recent changes
  # behind, so a missing tool stops the backup rather than producing one that
  # cannot be trusted.
  need_command sqlite3

  step "$(tr_msg step_backup)" bash -c "
    mkdir -p '$BACKUP_DIR'
    sqlite3 '$database' \".backup '$destination'\"
    chown '$APP_USER:$APP_GROUP' '$destination'
    ls -1t '$BACKUP_DIR'/melyxar-*.db 2>/dev/null | tail -n +$((BACKUP_KEEP + 1)) | xargs -r rm -f
  "
  success "$(tr_fmt backup_done "$destination")"
}

action_restore() {
  is_installed || die "$(tr_msg err_not_installed)"
  section "$(tr_msg section_restore)"

  local backups=() backup
  mapfile -t backups < <(ls -1t "${BACKUP_DIR}"/melyxar-*.db 2>/dev/null || true)
  if [[ "${#backups[@]}" -eq 0 ]]; then
    warn "$(tr_fmt restore_none "$BACKUP_DIR")"
    return 0
  fi

  echo
  local index=1
  for backup in "${backups[@]}"; do
    printf "  %b%2d%b) %s %b(%s)%b\n" "${BOLD}" "$index" "${RESET}" \
      "$(basename "$backup")" "${DIM}" "$(date -r "$backup" '+%Y-%m-%d %H:%M')" "${RESET}"
    index=$((index + 1))
  done
  echo

  local choice
  choice="$(prompt_default "$(tr_msg restore_pick)" "1")"
  # A line that is not a number must not become a position in the list, where
  # a negative one would quietly pick the oldest backup of them all.
  [[ "$choice" =~ ^[1-9][0-9]*$ ]] || die "$(tr_fmt err_bad_choice "$choice")"
  local picked="${backups[$((choice - 1))]:-}"
  [[ -z "$picked" ]] && die "$(tr_fmt err_bad_choice "$choice")"

  confirm_default_no "$(tr_msg restore_confirm)" || { info "$(tr_msg cancelled)"; return 0; }

  local kept
  kept="${BACKUP_DIR}/before-restore-$(date +%Y%m%d-%H%M%S).db"
  step "$(tr_msg step_stop)" systemctl stop "$SERVICE"
  step "$(tr_msg step_restore)" bash -c "
    if [[ -f '${DATA_DIR}/melyxar.db' ]]; then
      mv '${DATA_DIR}/melyxar.db' '$kept'
    fi
    rm -f '${DATA_DIR}/melyxar.db-wal' '${DATA_DIR}/melyxar.db-shm'
    cp -a '$picked' '${DATA_DIR}/melyxar.db'
    chown '$APP_USER:$APP_GROUP' '${DATA_DIR}/melyxar.db'
  "
  step "$(tr_msg step_restart)" systemctl restart "$SERVICE"
  if [[ -f "$kept" ]]; then
    success "$(tr_fmt restore_done "$kept")"
  else
    success "$(tr_msg restore_done_fresh)"
  fi
}

action_uninstall() {
  section "$(tr_msg section_uninstall)"

  confirm_default_no "$(tr_msg uninstall_confirm)" || { info "$(tr_msg cancelled)"; return 0; }

  if systemctl list-unit-files 2>/dev/null | grep -q "^${SERVICE}.service"; then
    step "$(tr_msg step_stop)" bash -c "
      systemctl disable --now '$SERVICE' || true
      rm -f '$UNIT_FILE'
      systemctl daemon-reload
    "
  fi

  rm -f "$BINARY_PATH"
  rm -rf "$SOURCE_DIR" "$INSTALLED_DIR" "$(dirname "$INTERFACE_DIR")"

  if confirm_default_no "$(tr_msg uninstall_keep_data)"; then
    rm -rf "$DATA_DIR" "$CACHE_DIR" "$CONFIG_DIR"
    userdel "$APP_USER" 2>/dev/null || true
  else
    info "$(tr_fmt uninstall_data_kept "$DATA_DIR" "$CONFIG_DIR")"
  fi

  success "$(tr_msg uninstall_done)"
}

# ── Menu ──────────────────────────────────────────────────────────────────────

menu() {
  echo
  printf "%b%s%b\n" "${BOLD}" "$(tr_msg menu_title)" "${RESET}"
  echo
  printf "   %b1%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_install)"
  printf "   %b2%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_update)"
  printf "   %b3%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_status)"
  printf "   %b4%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_backup)"
  printf "   %b5%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_restore)"
  printf "   %b6%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_uninstall)"
  printf "   %b7%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_bench)"
  printf "   %b8%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_accounts)"
  printf "   %b9%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_lines)"
  printf "  %b10%b) %s\n" "${BOLD}${RED_SOFT}" "${RESET}" "$(tr_msg menu_writes)"
  printf "   %b0%b) %s\n" "${BOLD}${GRAY}" "${RESET}" "$(tr_msg menu_quit)"
  echo

  local choice
  choice="$(prompt_default "$(tr_msg prompt_choice)" "2")"

  case "$choice" in
    1) action_install ;;
    2) action_update ;;
    3) action_status ;;
    4) action_backup ;;
    5) action_restore ;;
    6) action_uninstall ;;
    7) action_bench ;;
    8) action_accounts ;;
    9) action_lines ;;
    10) action_writes ;;
    0) exit 0 ;;
    *) die "$(tr_fmt err_bad_choice "$choice")" ;;
  esac
}

main() {
  need_root
  need_command awk
  need_command sed

  # Switches are read first and taken out, so the action stays the first word
  # whether or not one was given.
  local rest=()
  while (($#)); do
    case "$1" in
      -v | --verbose) VERBOSE=1 ;;
      -q | --quiet) VERBOSE=0 ;;
      *) rest+=("$1") ;;
    esac
    shift
  done
  set -- ${rest[@]+"${rest[@]}"}

  banner
  [[ "$VERBOSE" -eq 1 ]] && info "$(tr_msg verbose_on)"

  # A named action runs straight away, which is what the update entry in a
  # scheduled job needs.
  case "${1:-}" in
    install)   action_install ;;
    update)    action_update ;;
    status)    action_status ;;
    backup)    action_backup ;;
    restore)   action_restore ;;
    uninstall) action_uninstall ;;
    bench)     action_bench ;;
    accounts)  action_accounts ;;
    lines)     action_lines ;;
    # Without a terminal there is nobody to answer the menu, and an install is
    # far too heavy a thing to start on a default nobody chose.
    "")        has_terminal || die "$(tr_msg err_no_terminal)"; menu ;;
    *)         die "$(tr_fmt err_bad_choice "$1")" ;;
  esac
}

main "$@"
