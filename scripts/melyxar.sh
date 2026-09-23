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
en|app_name|Melyxar media server install, update and management
fr|app_name|Installation, mise à jour et gestion du serveur multimédia Melyxar
en|err_need_root|Run this script as root.
fr|err_need_root|Exécutez ce script en root.
en|err_command_output|Command output:
fr|err_command_output|Sortie de la commande :
en|verbose_on|Verbose: every command shows its own output as it runs.
fr|verbose_on|Mode détaillé : chaque commande affiche sa sortie au fur et à mesure.
en|err_aborted|Stopped at line %s. Nothing else was changed.
fr|err_aborted|Arrêt à la ligne %s. Rien d'autre n'a été modifié.
en|err_missing_command|Missing command: %s
fr|err_missing_command|Commande manquante : %s
en|hint_yes_default|Enter=yes / no
fr|hint_yes_default|Entrée=oui / non
en|hint_no_default|yes / Enter=no
fr|hint_no_default|oui / Entrée=non
en|menu_title|What do you want to do?
fr|menu_title|Que voulez-vous faire ?
en|menu_install|Install Melyxar
fr|menu_install|Installer Melyxar
en|menu_update|Update to the latest version
fr|menu_update|Mettre à jour vers la dernière version
en|menu_status|Show the server report
fr|menu_status|Afficher le rapport du serveur
en|menu_backup|Back up the database now
fr|menu_backup|Sauvegarder la base maintenant
en|menu_restore|Restore from a backup
fr|menu_restore|Restaurer depuis une sauvegarde
en|menu_uninstall|Uninstall
fr|menu_uninstall|Désinstaller
en|menu_bench|Measure the speed on a large library
fr|menu_bench|Mesurer la vitesse sur une grosse bibliothèque
en|menu_lines|Count the lines of code
fr|menu_lines|Compter les lignes de code
en|menu_accounts|Accounts: see them, or put a password back
fr|menu_accounts|Comptes : les voir, ou remettre un mot de passe
en|section_accounts|Accounts
fr|section_accounts|Comptes
en|accounts_notice|The way back in for somebody locked out of their own server. It asks for no password of its own, because whoever has a terminal here can read the database anyway.
fr|accounts_notice|Le moyen de rentrer pour quelqu'un enfermé dehors de son propre serveur. Il ne demande aucun mot de passe, parce que qui a un terminal ici peut de toute façon lire la base.
en|accounts_none|No account yet: open this server in a browser to set it up.
fr|accounts_none|Aucun compte pour l'instant : ouvrez ce serveur dans un navigateur pour le configurer.
en|prompt_accounts_which|Whose password to put back, or nothing to leave them alone
fr|prompt_accounts_which|De qui remettre le mot de passe, ou rien pour n'y pas toucher
en|prompt_accounts_password|The new password
fr|prompt_accounts_password|Le nouveau mot de passe
en|accounts_changed|The password was changed, and every device of that account was signed out.
fr|accounts_changed|Le mot de passe a été changé, et tous les appareils de ce compte ont été déconnectés.
en|menu_writes|Media folders: let Melyxar write to them, or keep them read only
fr|menu_writes|Dossiers des médias : laisser Melyxar y écrire, ou les garder en lecture seule
en|section_writes|Writing to the media folders
fr|section_writes|Écriture dans les dossiers des médias
en|writes_notice|By default Melyxar may only read your media. Allowing it to write is what lets "Delete from the disk" work. Every other folder of this machine stays closed to it.
fr|writes_notice|Par défaut Melyxar ne peut que lire vos médias. L'autoriser à écrire est ce qui permet à « Supprimer aussi du disque » de fonctionner. Tous les autres dossiers de la machine lui restent fermés.
en|writes_now_open|Now: Melyxar may write to the folders of the libraries.
fr|writes_now_open|Actuellement : Melyxar peut écrire dans les dossiers des bibliothèques.
en|writes_now_shut|Now: the media folders are read only for Melyxar.
fr|writes_now_shut|Actuellement : les dossiers des médias sont en lecture seule pour Melyxar.
en|prompt_writes_open|Let Melyxar write to the media folders?
fr|prompt_writes_open|Laisser Melyxar écrire dans les dossiers des médias ?
en|prompt_writes_shut|Put the media folders back to read only?
fr|prompt_writes_shut|Remettre les dossiers des médias en lecture seule ?
en|writes_no_folder|No library folder yet: declare a library first.
fr|writes_no_folder|Aucun dossier de bibliothèque pour l'instant : déclarez d'abord une bibliothèque.
en|writes_opened|Melyxar may now write to these folders.
fr|writes_opened|Melyxar peut maintenant écrire dans ces dossiers.
en|writes_shut|The media folders are read only again.
fr|writes_shut|Les dossiers des médias sont de nouveau en lecture seule.
en|writes_refused|still refused by the owner of the folder: give the melyxar account the right to write there.
fr|writes_refused|toujours refusé par le propriétaire du dossier : donnez au compte melyxar le droit d'y écrire.
en|step_writes|Opening the media folders for writing
fr|step_writes|Ouverture des dossiers des médias en écriture
en|menu_quit|Quit
fr|menu_quit|Quitter
en|prompt_choice|Your choice
fr|prompt_choice|Votre choix
en|err_bad_choice|Unknown choice: %s
fr|err_bad_choice|Choix inconnu : %s
en|section_checks|Checking the system
fr|section_checks|Vérification du système
en|check_debian_ok|Debian based system detected
fr|check_debian_ok|Système basé sur Debian détecté
en|check_debian_warn|This script targets a Debian based system. Continuing anyway.
fr|check_debian_warn|Ce script vise un système basé sur Debian. On continue quand même.
en|check_container|Running inside a container
fr|check_container|Exécution dans un conteneur
en|check_memory|Memory available: %s MB
fr|check_memory|Mémoire disponible : %s Mo
en|warn_memory_low|Building needs roughly 6 to 8 GB at its peak. With less, the build may be killed.
fr|warn_memory_low|La compilation demande environ 6 à 8 Go au pic. En dessous, elle peut être tuée.
en|check_disk|Free space on the system disk: %s GB
fr|check_disk|Espace libre sur le disque système : %s Go
en|warn_disk_low|Building needs roughly 20 GB for its intermediate files.
fr|warn_disk_low|La compilation demande environ 20 Go pour ses fichiers intermédiaires.
en|check_port_free|Port %s is free
fr|check_port_free|Le port %s est libre
en|check_port_busy|Port %s is already in use by something else
fr|check_port_busy|Le port %s est déjà utilisé par autre chose
en|check_ffmpeg_ok|Media tools found: %s
fr|check_ffmpeg_ok|Outils média trouvés : %s
en|check_ffmpeg_missing|Media tools not found. They will be installed.
fr|check_ffmpeg_missing|Outils média absents. Ils seront installés.
en|check_dri_ok|Graphics device visible, hardware acceleration is possible
fr|check_dri_ok|Périphérique graphique visible, l'accélération matérielle est possible
en|check_dri_missing|No graphics device visible
fr|check_dri_missing|Aucun périphérique graphique visible
en|dri_title|Hardware acceleration needs a device
fr|dri_title|L'accélération matérielle a besoin d'un périphérique
en|dri_line1|Melyxar can use a graphics card to transcode, which costs far less
fr|dri_line1|Melyxar peut utiliser une carte graphique pour transcoder, ce qui coûte bien
en|dri_line2|processor time than doing it in software.
fr|dri_line2|moins de temps processeur que de le faire en logiciel.
en|dri_line3|In an unprivileged container the device has to be passed from the host.
fr|dri_line3|Dans un conteneur non privilégié, le périphérique doit être passé depuis l'hôte.
en|dri_line4|This is done on the host, not inside the container.
fr|dri_line4|Cette action se fait sur l'hôte, pas dans le conteneur.
en|dri_commands|Commands to run on the Proxmox host:
fr|dri_commands|Commandes à lancer sur le host Proxmox :
en|dri_replace|Replace <CTID> with the container id.
fr|dri_replace|Remplacez <CTID> par l'identifiant du conteneur.
en|dri_later|You can install now and pass the device later.
fr|dri_later|Vous pouvez installer maintenant et passer le périphérique plus tard.
en|section_packages|Installing what is needed
fr|section_packages|Installation des dépendances
en|step_apt_update|Refreshing the package lists
fr|step_apt_update|Rafraîchissement de la liste des paquets
en|step_apt_install|Installing build tools and media tools
fr|step_apt_install|Installation des outils de compilation et des outils média
en|step_rust|Installing the Rust toolchain
fr|step_rust|Installation de la chaîne d'outils Rust
en|step_node|Installing Node.js %s
fr|step_node|Installation de Node.js %s
en|step_node_distribution|Installing the distribution's Node.js
fr|step_node_distribution|Installation du Node.js de la distribution
en|node_present|Node.js already current: %s
fr|node_present|Node.js déjà à jour : %s
en|node_arch_unknown|Unknown processor type: %s. Node.js cannot be fetched for it.
fr|node_arch_unknown|Type de processeur inconnu : %s. Node.js ne peut pas être téléchargé pour lui.
en|node_unreachable|nodejs.org could not be reached.
fr|node_unreachable|nodejs.org est injoignable.
en|node_bad_sum|What arrived does not match the published checksum and was thrown away.
fr|node_bad_sum|Ce qui est arrivé ne correspond pas à l'empreinte publiée et a été jeté.
en|node_unpack_failed|The archive could not be unpacked. Nothing was replaced.
fr|node_unpack_failed|L'archive n'a pas pu être décompressée. Rien n'a été remplacé.
en|node_installed|Node.js %s installed
fr|node_installed|Node.js %s installé
en|node_kept|The Node.js already installed is kept: %s
fr|node_kept|Le Node.js déjà installé est conservé : %s
en|node_fallback|Node.js is taken from the distribution instead. It is older than the interface's own tools ask for, which is what the warnings during the build are about.
fr|node_fallback|Node.js est pris dans la distribution à la place. Il est plus ancien que ce que demandent les outils de l'interface, et c'est de là que viennent les avertissements pendant la compilation.
en|step_npm_install|Installing the interface's own dependencies
fr|step_npm_install|Installation des dépendances propres à l'interface
en|step_npm_build|Building the web interface
fr|step_npm_build|Compilation de l'interface web
en|rust_present|Rust toolchain already present: %s
fr|rust_present|Chaîne d'outils Rust déjà présente : %s
en|rust_long|A couple of hundred megabytes over one connection. Several minutes is ordinary; the bar underneath is the installer's own.
fr|rust_long|Deux cents mégaoctets et quelques en une seule connexion. Plusieurs minutes, c'est normal ; la barre en dessous est celle de l'installateur.
en|rust_unreachable|rustup could not be reached. Check this container's network, then choose this entry again.
fr|rust_unreachable|rustup est injoignable. Vérifiez le réseau de ce conteneur, puis relancez cette entrée.
en|rust_too_long|The toolchain did not arrive within %s minutes and was stopped. Choose this entry again: it starts over from nothing and keeps nothing half written.
fr|rust_too_long|La chaîne d'outils n'est pas arrivée en %s minutes et a été arrêtée. Relancez cette entrée : elle repart de zéro et ne garde rien d'écrit à moitié.
en|section_account|Preparing the system account and folders
fr|section_account|Préparation du compte système et des dossiers
en|step_user|Creating the system account
fr|step_user|Création du compte système
en|user_present|System account already present
fr|user_present|Compte système déjà présent
en|step_dirs|Creating the folders
fr|step_dirs|Création des dossiers
en|section_source|Getting the source
fr|section_source|Récupération des sources
en|step_clone|Cloning the repository, with its full history
fr|step_clone|Clonage du dépôt, avec tout son historique
en|step_pull|Fetching the latest changes
fr|step_pull|Récupération des dernières modifications
en|step_unshallow|Completing a partial copy of the repository into a full one
fr|step_unshallow|Complétion d'une copie partielle du dépôt en copie complète
en|section_update_diff|What is about to change
fr|section_update_diff|Ce qui va changer
en|update_diff_up_to_date|Already at the latest version. Nothing to update.
fr|update_diff_up_to_date|Déjà à la dernière version. Rien à mettre à jour.
en|section_lines|Lines of code
fr|section_lines|Lignes de code
en|step_fetch_main|Fetching the %s branch as well
fr|step_fetch_main|Récupération de la branche %s également
en|lines_on|%s: %s lines
fr|lines_on|%s : %s lignes
en|lines_ahead|%s is %s lines longer than %s
fr|lines_ahead|%s compte %s lignes de plus que %s
en|lines_behind|%s is %s lines shorter than %s
fr|lines_behind|%s compte %s lignes de moins que %s
en|lines_same|%s and %s are the same length
fr|lines_same|%s et %s font la même longueur
en|lines_changed|%s lines added, %s removed across %s files
fr|lines_changed|%s lignes ajoutées, %s supprimées sur %s fichiers
en|section_bench|Bench
fr|section_bench|Banc d'essai
en|bench_notice|A library of invented works is written beside your own, the pages somebody browsing would ask for are timed on it, and it is removed again. Your own films are never touched and no file is written to a disk.
fr|bench_notice|Une bibliothèque d'œuvres inventées est écrite à côté de la vôtre, les pages qu'un spectateur ouvrirait sont chronométrées dessus, puis elle est retirée. Vos propres films ne sont jamais touchés et aucun fichier n'est écrit sur un disque.
en|bench_takes|Counting on about five minutes in all, and about a gigabyte in the database while it lasts.
fr|bench_takes|Comptez environ cinq minutes en tout, et à peu près un gigaoctet dans la base le temps que ça dure.
en|prompt_bench_works|How many works to invent
fr|prompt_bench_works|Combien d'œuvres inventer
en|prompt_bench_account|Which account to measure as
fr|prompt_bench_account|Avec quel compte mesurer
en|prompt_bench_password|Its password
fr|prompt_bench_password|Son mot de passe
en|bench_signs_in|This server answers nothing to somebody who is not signed in, so the measuring signs in the way a browser does.
fr|bench_signs_in|Ce serveur ne répond rien à qui n'est pas connecté, donc la mesure se connecte comme le ferait un navigateur.
en|err_bench_account|An account is needed to measure as.
fr|err_bench_account|Il faut un compte pour mesurer.
en|err_bench_works|That is not a number of works: %s
fr|err_bench_works|Ce n'est pas un nombre d'œuvres : %s
en|bench_needs_server|The server has to be running to be measured. Start it first.
fr|bench_needs_server|Le serveur doit tourner pour être mesuré. Démarrez-le d'abord.
en|step_bench_clear|Taking away what an earlier run may have left
fr|step_bench_clear|Retrait de ce qu'un essai précédent aurait laissé
en|step_bench_fill|Inventing the library
fr|step_bench_fill|Invention de la bibliothèque
en|bench_measuring|Measuring. The table below says what each page took and what it was allowed.
fr|bench_measuring|Mesure en cours. Le tableau ci-dessous dit ce qu'a pris chaque page et ce à quoi elle avait droit.
en|step_bench_empty|Removing the invented library
fr|step_bench_empty|Retrait de la bibliothèque inventée
en|bench_held|Every budget is held at this size.
fr|bench_held|Tous les budgets sont tenus à cette taille.
en|bench_missed|A budget was missed. The table above names which page, and by how much.
fr|bench_missed|Un budget est dépassé. Le tableau ci-dessus dit quelle page, et de combien.
en|lines_counted|Counted without the lock files and the images.
fr|lines_counted|Compté sans les fichiers de verrouillage et les images.
en|section_build|Building
fr|section_build|Compilation
en|build_notice|This takes 5 to 10 minutes the first time and 1 to 2 minutes afterwards.
fr|build_notice|Cela prend 5 à 10 minutes la première fois, puis 1 à 2 minutes ensuite.
en|build_nice|The build runs at the highest priority, for the fastest rebuild while this is still being developed.
fr|build_nice|La compilation tourne à la priorité la plus haute, pour aller le plus vite possible tant que c'est en plein développement.
en|build_nice_denied|This container is not allowed to raise a priority, so the build runs at the usual one. Nothing else changes.
fr|build_nice_denied|Ce conteneur n'a pas le droit d'augmenter une priorité, la compilation tourne donc à la priorité normale. Rien d'autre ne change.
en|step_build|Building the server
fr|step_build|Compilation du serveur
en|step_install_binary|Installing the binary
fr|step_install_binary|Installation du binaire
en|step_install_interface|Putting the interface in place
fr|step_install_interface|Mise en place de l'interface
en|build_nothing|Nothing to rebuild: the server and the interface already match the source.
fr|build_nothing|Rien à reconstruire : le serveur et l'interface correspondent déjà aux sources.
en|build_interface_only|Only the interface changed: it is rebuilt and put in place while the server keeps running, with no restart.
fr|build_interface_only|Seule l'interface a changé : elle est reconstruite et mise en place pendant que le serveur continue de tourner, sans redémarrage.
en|build_engine_only|Only the server changed: the interface is left as it is.
fr|build_engine_only|Seul le serveur a changé : l'interface reste telle quelle.
en|section_config|Configuration
fr|section_config|Configuration
en|config_present|A configuration file already exists and was left untouched
fr|config_present|Un fichier de configuration existe déjà et n'a pas été touché
en|step_config|Writing the starting configuration
fr|step_config|Écriture de la configuration de départ
en|prompt_port|Which port should Melyxar listen on
fr|prompt_port|Sur quel port Melyxar doit-il écouter
en|prompt_add_library|Add a library now
fr|prompt_add_library|Ajouter une bibliothèque maintenant
en|prompt_library_name|Library name
fr|prompt_library_name|Nom de la bibliothèque
en|prompt_library_kind|Library kind (movies, series, anime, shows, home_media, music)
fr|prompt_library_kind|Type de bibliothèque (movies, series, anime, shows, home_media, music)
en|prompt_root_path|Folder holding the media (empty to stop adding)
fr|prompt_root_path|Dossier contenant les médias (vide pour arrêter d'ajouter)
en|prompt_root_label|Short label for this folder, shown in logs instead of the path
fr|prompt_root_label|Libellé court pour ce dossier, affiché dans les journaux au lieu du chemin
en|root_added|Folder added: %s
fr|root_added|Dossier ajouté : %s
en|root_missing|That folder does not exist. Add it anyway?
fr|root_missing|Ce dossier n'existe pas. L'ajouter quand même ?
en|root_unreadable|That folder exists but cannot be read by the server account.
fr|root_unreadable|Ce dossier existe mais ne peut pas être lu par le compte du serveur.
en|root_not_absolute|A folder has to be given in full, starting with a slash.
fr|root_not_absolute|Un dossier doit être donné en entier, en commençant par une barre oblique.
en|err_bad_kind|Unknown kind. Pick one of: movies, series, anime, shows, home_media, music.
fr|err_bad_kind|Type inconnu. Choisissez parmi : movies, series, anime, shows, home_media, music.
en|library_without_root|No folder was given, so the library was not written. You can add it later from the interface.
fr|library_without_root|Aucun dossier n'a été donné, la bibliothèque n'a donc pas été écrite. Vous pourrez l'ajouter plus tard depuis l'interface.
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
en|section_done|Done
fr|section_done|Terminé
en|done_title|Melyxar is running
fr|done_title|Melyxar tourne
en|done_open|Open it at:
fr|done_open|Ouvrez-le à l'adresse :
en|done_config|Configuration file:
fr|done_config|Fichier de configuration :
en|done_logs|Follow the logs with:
fr|done_logs|Suivez les journaux avec :
en|done_report|Run the report with:
fr|done_report|Lancez le rapport avec :
en|done_again|Run this script again, always the published version, with:
fr|done_again|Relancez ce script, toujours dans sa version publiée, avec :
en|section_backup|Backup
fr|section_backup|Sauvegarde
en|step_backup|Copying the database
fr|step_backup|Copie de la base
en|backup_done|Backup written to %s
fr|backup_done|Sauvegarde écrite dans %s
en|backup_none|No database to back up yet
fr|backup_none|Aucune base à sauvegarder pour l'instant
en|section_restore|Restore
fr|section_restore|Restauration
en|restore_none|No backup found in %s
fr|restore_none|Aucune sauvegarde trouvée dans %s
en|restore_pick|Which backup should be restored
fr|restore_pick|Quelle sauvegarde faut-il restaurer
en|restore_confirm|This replaces the current database. A copy of it is kept first. Continue?
fr|restore_confirm|Cela remplace la base actuelle. Une copie est gardée avant. Continuer ?
en|step_restore|Putting the backup back in place
fr|step_restore|Remise en place de la sauvegarde
en|restore_done|Restored. The current database was kept as %s
fr|restore_done|Restauré. La base actuelle a été gardée sous %s
en|restore_done_fresh|Restored. There was no database to keep aside.
fr|restore_done_fresh|Restauré. Il n'y avait aucune base à mettre de côté.
en|section_uninstall|Uninstall
fr|section_uninstall|Désinstallation
en|uninstall_confirm|Remove the service, the binary and the sources?
fr|uninstall_confirm|Supprimer le service, le binaire et les sources ?
en|uninstall_keep_data|Your library data and configuration are kept. Remove them too?
fr|uninstall_keep_data|Vos données de bibliothèque et la configuration sont gardées. Les supprimer aussi ?
en|uninstall_data_kept|Data kept in %s and configuration in %s
fr|uninstall_data_kept|Données gardées dans %s et configuration dans %s
en|uninstall_done|Melyxar was removed
fr|uninstall_done|Melyxar a été supprimé
en|err_not_installed|Melyxar is not installed yet. Choose install first.
fr|err_not_installed|Melyxar n'est pas encore installé. Choisissez d'abord l'installation.
en|err_no_terminal|There is no terminal to ask on. Name the action: install, update, status, backup, restore, uninstall.
fr|err_no_terminal|Aucun terminal pour poser les questions. Indiquez l'action : install, update, status, backup, restore, uninstall.
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
    write_media_writes "$(library_folders)"
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

# The folders of the libraries, one a line, as the server knows them.
library_folders() {
  runuser -u "$APP_USER" -- "$BINARY_PATH" --config "$CONFIG_FILE" folders 2>/dev/null
}

# Opens every folder of the libraries for writing, and only them. Written
# again on each update, so a library declared since is opened as well. A
# folder that is not there is skipped rather than stopping the server.
write_media_writes() {
  local folders="$1" line tmp
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

action_writes() {
  is_installed || die "$(tr_msg err_not_installed)"
  section "$(tr_msg section_writes)"
  info "$(tr_msg writes_notice)"
  echo

  if [[ -f "$WRITES_FILE" ]]; then
    info "$(tr_msg writes_now_open)"
    confirm_default_no "$(tr_msg prompt_writes_shut)" || { info "$(tr_msg cancelled)"; return 0; }
    rm -f "$WRITES_FILE"
    systemctl daemon-reload
    step "$(tr_msg step_restart)" systemctl restart "$SERVICE"
    success "$(tr_msg writes_shut)"
    return 0
  fi

  info "$(tr_msg writes_now_shut)"
  local folders
  folders="$(library_folders)"
  if [[ -z "$folders" ]]; then
    warn "$(tr_msg writes_no_folder)"
    return 0
  fi
  printf '%s\n' "$folders" | sed 's/^/  /'
  echo
  confirm_default_no "$(tr_msg prompt_writes_open)" || { info "$(tr_msg cancelled)"; return 0; }

  step "$(tr_msg step_writes)" write_media_writes "$folders"
  step "$(tr_msg step_restart)" systemctl restart "$SERVICE"

  # The service may now write; whether the owner of each folder lets the
  # melyxar account do so is a second, separate question, asked by trying.
  local folder probe
  while IFS= read -r folder; do
    [[ -d "$folder" ]] || continue
    probe="$folder/.melyxar-write-probe"
    if runuser -u "$APP_USER" -- touch "$probe" 2>/dev/null; then
      rm -f "$probe"
    else
      warn "$folder: $(tr_msg writes_refused)"
    fi
  done <<< "$folders"
  success "$(tr_msg writes_opened)"
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
